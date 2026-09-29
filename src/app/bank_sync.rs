use super::state::App;
use crate::db::category_store::CategoryStore;
use crate::db::database::SqliteDatabase;
use crate::db::investment_store::{InvestmentStore, SqliteInvestmentStore};
use crate::db::transaction_store::{SqliteTransactionStore, TransactionStore};
use crate::model::CategoryDraft;
use crate::pluggy::mapping::{Mapped, map_transaction};
use crate::pluggy::{self, SyncBatch};
use chrono::{Duration, NaiveDate};
use std::io::Error;
use std::sync::mpsc;
use std::thread;

/// How far back the first sync of a ledger reaches.
const FIRST_SYNC_DAYS: i64 = 365;
/// Later syncs re-read this much before the last one, so pending card purchases that moved
/// or posted since are refreshed.
const RESYNC_OVERLAP_DAYS: i64 = 30;

/// A fetch running on a background thread, bound to the ledger that was active when it began.
pub(crate) struct BankSyncJob {
    ledger_id: i64,
    rx: mpsc::Receiver<Result<SyncBatch, String>>,
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
        let since = match self.last_bank_sync() {
            Ok(Some(last)) => last - Duration::days(RESYNC_OVERLAP_DAYS),
            Ok(None) => today - Duration::days(FIRST_SYNC_DAYS),
            Err(err) => {
                self.set_status_message(format!("Bank sync: {}", err), None);
                return;
            }
        };

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(pluggy::fetch(&credentials, since));
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
            Ok(batch) => match self.apply_bank_sync(ledger_id, batch) {
                Ok(message) => message,
                Err(err) => format!("Bank sync failed to save: {}", err),
            },
            Err(err) => format!("Bank sync failed: {}", err),
        };
        self.set_status_message(message, None);
        true
    }

    fn apply_bank_sync(&mut self, ledger_id: i64, batch: SyncBatch) -> Result<String, Error> {
        let today = self.today();
        let mut transactions = Vec::new();
        let mut flows = Vec::new();
        let mut skipped = 0;
        for tx in &batch.transactions {
            match map_transaction(tx, &batch.categories, today) {
                Mapped::Transaction(row) => transactions.push(row),
                Mapped::Flow(flow) => flows.push(flow),
                Mapped::Skipped => skipped += 1,
            }
        }

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
        let flows_added =
            SqliteInvestmentStore::new(database.clone(), ledger_id).sync_flows(&flows)?;
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
            "Bank sync: {} new, {} updated, {} investment moves, {} skipped.",
            summary.added, summary.updated, flows_added, skipped
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
