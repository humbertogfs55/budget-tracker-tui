use super::state::App;
use crate::db::category_store::CategoryStore;
use crate::db::database::SqliteDatabase;
use crate::db::investment_store::{
    InvestmentStore, SqliteInvestmentStore, SyncedFlow, SyncedValuation,
};
use crate::db::transaction_store::{SqliteTransactionStore, SyncedTransaction, TransactionStore};
use crate::market_price::BtcPrices;
use crate::model::CategoryDraft;
use crate::pluggy::mapping::{
    CRYPTO_ACCOUNT, Mapped, crypto_valuation, map_investments, map_transaction, price_crypto_flow,
};
use crate::pluggy::{self, Credentials};
use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use std::io::Error;
use std::sync::mpsc;
use std::thread;

/// How far back the first sync of a ledger asks for. Pluggy only returns what the bank shares
/// (about 14 months for Nubank), so asking far back means "everything available", which
/// keeps investment contributions complete.
const FIRST_SYNC_DAYS: i64 = 3650;
/// Later syncs re-read this much before the last one, so pending card purchases that moved
/// or posted since are refreshed.
const RESYNC_OVERLAP_DAYS: i64 = 30;

/// A fetch running on a background thread, bound to the ledger that was active when it began.
pub(crate) struct BankSyncJob {
    ledger_id: i64,
    rx: mpsc::Receiver<Result<SyncOutcome, String>>,
}

/// Everything the network side produced, ready to write.
struct SyncOutcome {
    transactions: Vec<SyncedTransaction>,
    flows: Vec<SyncedFlow>,
    valuations: Vec<SyncedValuation>,
    skipped: usize,
    /// Live BTC price, fetched only when the crypto account is being tracked.
    btc_price: Option<Decimal>,
}

/// Fetch from Pluggy, map it, and price crypto trades. Runs off the UI thread.
fn collect(
    credentials: &Credentials,
    since: NaiveDate,
    today: NaiveDate,
    crypto_since: Option<NaiveDate>,
) -> Result<SyncOutcome, String> {
    let batch = pluggy::fetch(credentials, since)?;
    let prices = crypto_since.map(|_| BtcPrices::new());
    let mut outcome = SyncOutcome {
        transactions: Vec::new(),
        flows: Vec::new(),
        valuations: map_investments(&batch.investments, today),
        skipped: 0,
        btc_price: None,
    };
    for tx in &batch.transactions {
        match map_transaction(tx, &batch.categories, today, crypto_since) {
            Mapped::Transaction(row) => outcome.transactions.push(row),
            Mapped::Flow(flow) => outcome.flows.push(flow),
            Mapped::CryptoFlow { flow, at } => {
                let prices = prices
                    .as_ref()
                    .expect("crypto flows only map with a cutoff");
                outcome.flows.push(price_crypto_flow(flow, prices.at(at)?));
            }
            Mapped::Skipped => outcome.skipped += 1,
        }
    }
    if let Some(prices) = &prices {
        outcome.btc_price = Some(prices.now()?);
    }
    Ok(outcome)
}

impl App {
    /// Start fetching from Pluggy in the background. Silent when sync isn't configured unless
    /// the user asked for it (`manual`).
    pub(crate) fn start_bank_sync(&mut self, manual: bool) {
        if self.bank_sync_job.is_some() {
            if manual {
                self.set_status_message("Bank sync already running.", Some(Duration::seconds(3)));
            }
            return;
        }
        let credentials = match pluggy::load_credentials() {
            Ok(Some(credentials)) => credentials,
            Ok(None) => {
                if manual {
                    let path = pluggy::env_file_path()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "pluggy.env".to_string());
                    self.set_status_message(
                        format!(
                            "Bank sync is not set up: add Pluggy credentials to {}",
                            path
                        ),
                        None,
                    );
                }
                return;
            }
            Err(err) => {
                self.set_status_message(format!("Bank sync: {}", err), None);
                return;
            }
        };

        let today = self.today();
        let window = self.last_bank_sync().and_then(|last| {
            let crypto_since = self
                .investment_store()
                .holding(CRYPTO_ACCOUNT)?
                .map(|holding| holding.as_of);
            Ok((last, crypto_since))
        });
        let (since, crypto_since) = match window {
            Ok((last, crypto_since)) => {
                let since = match last {
                    Some(last) => last - Duration::days(RESYNC_OVERLAP_DAYS),
                    None => today - Duration::days(FIRST_SYNC_DAYS),
                };
                // Reach back far enough to see every trade after a newly set opening.
                (since.min(crypto_since.unwrap_or(since)), crypto_since)
            }
            Err(err) => {
                self.set_status_message(format!("Bank sync: {}", err), None);
                return;
            }
        };

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(collect(&credentials, since, today, crypto_since));
        });
        self.bank_sync_job = Some(BankSyncJob {
            ledger_id: self.active_ledger_id,
            rx,
        });
        // Don't bury a startup warning under the progress note.
        if manual || self.status_message.is_none() {
            self.set_status_message("Syncing bank…", None);
        }
    }

    /// Apply a finished fetch, if there is one. Returns whether anything changed on screen.
    pub(crate) fn poll_bank_sync(&mut self) -> bool {
        let Some(job) = &self.bank_sync_job else {
            return false;
        };
        let result = match job.rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => Err("sync thread stopped".to_string()),
        };
        let ledger_id = job.ledger_id;
        self.bank_sync_job = None;

        let message = match result {
            Ok(outcome) => match self.apply_bank_sync(ledger_id, outcome) {
                Ok(message) => message,
                Err(err) => format!("Bank sync failed to save: {}", err),
            },
            Err(err) => format!("Bank sync failed: {}", err),
        };
        self.set_status_message(message, None);
        true
    }

    fn apply_bank_sync(&mut self, ledger_id: i64, outcome: SyncOutcome) -> Result<String, Error> {
        let today = self.today();
        let SyncOutcome {
            transactions,
            flows,
            mut valuations,
            skipped,
            btc_price,
        } = outcome;

        // Pluggy's categories join the shared catalog so they can be picked and budgeted.
        let category_store = self.category_store();
        for row in &transactions {
            let draft = &row.draft;
            let known = self.category_records.iter().any(|record| {
                record.transaction_type == draft.transaction_type
                    && record.category == draft.category
                    && record.subcategory == draft.subcategory
            });
            if !known {
                let record = category_store.insert(&CategoryDraft {
                    transaction_type: draft.transaction_type,
                    category: draft.category.clone(),
                    subcategory: draft.subcategory.clone(),
                    tag: None,
                })?;
                self.category_records.push(record);
            }
        }

        let database = SqliteDatabase::new(&self.database_path);
        let summary =
            SqliteTransactionStore::new(database.clone(), ledger_id).sync_merge(&transactions)?;
        let investments = SqliteInvestmentStore::new(database.clone(), ledger_id);
        let flows_added = investments.sync_flows(&flows)?;
        // Valued after the flows are in, so today's trades count toward the holding.
        if let Some(price) = btc_price
            && let Some(holding) = investments.holding(CRYPTO_ACCOUNT)?
        {
            valuations.push(crypto_valuation(holding, price, today));
        }
        let valuations_written = investments.sync_valuations(&valuations)?;
        let conn = database.ready_connection("bank sync")?;
        database.set_metadata_value(
            &conn,
            &last_sync_key(ledger_id),
            &today.format("%Y-%m-%d").to_string(),
        )?;

        if ledger_id == self.active_ledger_id {
            self.reload_working_set()?;
        } else {
            self.reload_categories_from_store()?;
        }

        Ok(format!(
            "Bank sync: {} new, {} updated, {} investment moves, {} valuations, {} skipped.",
            summary.added, summary.updated, flows_added, valuations_written, skipped
        ))
    }

    fn last_bank_sync(&self) -> Result<Option<NaiveDate>, Error> {
        let database = SqliteDatabase::new(&self.database_path);
        let conn = database.ready_connection("bank sync")?;
        Ok(database
            .metadata_value(&conn, &last_sync_key(self.active_ledger_id))?
            .and_then(|value| NaiveDate::parse_from_str(&value, "%Y-%m-%d").ok()))
    }
}

fn last_sync_key(ledger_id: i64) -> String {
    format!("pluggy_last_sync_ledger_{}", ledger_id)
}
