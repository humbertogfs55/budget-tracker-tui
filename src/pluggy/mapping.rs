//! Turns Pluggy data into what the app stores: transactions become a regular transaction, a
//! move into or out of an investment account, or nothing; investment balances become daily
//! valuations. Pure, so every rule is unit-tested.

use super::{Category, Investment, Transaction};
use crate::db::investment_store::{Holding, SyncedFlow, SyncedValuation};
use crate::db::transaction_store::SyncedTransaction;
use crate::model::{InvestmentEntryKind, TransactionDraft, TransactionType};
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap};

const CREDIT_CARD_PAYMENT: &str = "05100000";
const INVESTMENTS_PREFIX: &str = "03";

/// Investment accounts (name, kind) the sync files into. Flows and valuations must land in
/// the same account, or growth is computed against the wrong contributions.
const CRYPTO: (&str, &str) = ("Cripto", "Crypto");
const BROKERAGE: (&str, &str) = ("Renda Variável", "Brokerage");
const SAVINGS: (&str, &str) = ("Caixinhas / RDB", "Savings");
const OTHER_INVESTMENTS: (&str, &str) = ("Outros investimentos", "Other");

/// The crypto account is valued as units × market price, so it is only tracked once it has an
/// opening quantity (see `Holding`).
pub const CRYPTO_ACCOUNT: &str = CRYPTO.0;

/// Nubank's buy price sits above market and its sell price below, with no separate fee. 1% is
/// the smallest spread consistent with this account's trade history against its real balance.
const CRYPTO_SPREAD: Decimal = Decimal::from_parts(1, 0, 0, false, 2); // 0.01

#[derive(Debug, PartialEq)]
pub enum Mapped {
    Transaction(SyncedTransaction),
    Flow(SyncedFlow),
    /// A crypto trade still to be priced: `at` is when it happened, to look up the market
    /// price that minute and fill in its quantity.
    CryptoFlow {
        flow: SyncedFlow,
        at: DateTime<Utc>,
    },
    Skipped,
}

/// `crypto_since` is the crypto account's opening date: trades on or before it are already
/// inside the opening quantity, and with no opening at all crypto is not tracked.
pub fn map_transaction(
    tx: &Transaction,
    categories: &HashMap<String, Category>,
    today: NaiveDate,
    crypto_since: Option<NaiveDate>,
) -> Mapped {
    let Some(instant) = DateTime::parse_from_rfc3339(&tx.date).ok() else {
        return Mapped::Skipped;
    };
    let date = local_date(instant);
    let amount = tx.amount.abs().round_dp(2);
    let category_id = tx.category_id.as_deref().unwrap_or("");
    let description = tx.description.trim();
    let lower = description.to_lowercase();
    let is_debit = tx.kind == "DEBIT";

    // Future card installments show up the day they are billed.
    if date > today || amount.is_zero() {
        return Mapped::Skipped;
    }
    // Card purchases are already counted, so paying the bill is not spending: skip both the
    // payment leaving checking and the same payment arriving on the card.
    if category_id == CREDIT_CARD_PAYMENT || lower.starts_with("pagamento de fatura") {
        return Mapped::Skipped;
    }
    // Nubank's shared balance ("Espaço Família") isn't reported by Pluggy and is no longer
    // used, so its moves would only skew the investment it got filed under.
    if lower.contains("saldo compartilhado") {
        return Mapped::Skipped;
    }
    if category_id.starts_with(INVESTMENTS_PREFIX) || lower.contains("renda variável") {
        let (account_name, account_kind) = investment_account_for(&lower);
        let flow = SyncedFlow {
            pluggy_id: tx.id.clone(),
            account_name: account_name.to_string(),
            account_kind: account_kind.to_string(),
            date,
            entry_kind: if is_debit {
                InvestmentEntryKind::Contribution
            } else {
                InvestmentEntryKind::Withdrawal
            },
            amount,
            note: description.to_string(),
            quantity: None,
        };
        if account_name != CRYPTO_ACCOUNT {
            return Mapped::Flow(flow);
        }
        return match crypto_since {
            Some(since) if date > since => Mapped::CryptoFlow {
                flow,
                at: instant.with_timezone(&Utc),
            },
            _ => Mapped::Skipped,
        };
    }

    let (category, subcategory) = match categories.get(category_id) {
        Some(found) => match &found.parent_description {
            Some(parent) => (parent.clone(), found.description.clone()),
            None => (found.description.clone(), String::new()),
        },
        None => ("Uncategorized".to_string(), String::new()),
    };
    Mapped::Transaction(SyncedTransaction {
        pluggy_id: tx.id.clone(),
        draft: TransactionDraft {
            date,
            description: description.to_string(),
            amount,
            transaction_type: if is_debit {
                TransactionType::Expense
            } else {
                TransactionType::Income
            },
            category,
            subcategory,
            is_recurring: false,
            recurrence_frequency: None,
            recurrence_end_date: None,
        },
    })
}

/// Nubank's own wording is the only signal for which investment the money went to.
fn investment_account_for(lower_description: &str) -> (&'static str, &'static str) {
    if lower_description.contains("cripto") {
        CRYPTO
    } else if lower_description.contains("renda variável") {
        BROKERAGE
    } else {
        SAVINGS
    }
}

/// One valuation per account for `today`: the summed balance of every holding filed there.
/// Fully redeemed holdings still count (as zero), so an account emptied out reads as zero
/// rather than keeping its last value.
pub fn map_investments(investments: &[Investment], today: NaiveDate) -> Vec<SyncedValuation> {
    let mut totals: BTreeMap<(&str, &str), Decimal> = BTreeMap::new();
    for investment in investments {
        let account = match investment.kind.as_str() {
            "EQUITY" | "ETF" => BROKERAGE,
            "FIXED_INCOME" => SAVINGS,
            _ => OTHER_INVESTMENTS,
        };
        *totals.entry(account).or_default() += investment.balance.unwrap_or_default();
    }
    totals
        .into_iter()
        .map(|((name, kind), total)| SyncedValuation {
            account_name: name.to_string(),
            account_kind: kind.to_string(),
            date: today,
            amount: total.round_dp(2),
            note: "Synced from bank".to_string(),
        })
        .collect()
}

/// Fill in the units a crypto trade moved, given the market price at that minute. Buying
/// costs the spread above market; selling gets the spread below it. Rounded to satoshis.
pub fn price_crypto_flow(mut flow: SyncedFlow, market_price: Decimal) -> SyncedFlow {
    let effective_price = match flow.entry_kind {
        InvestmentEntryKind::Withdrawal => market_price * (Decimal::ONE - CRYPTO_SPREAD),
        _ => market_price * (Decimal::ONE + CRYPTO_SPREAD),
    };
    let units = (flow.amount / effective_price).round_dp(8);
    flow.note = format!("{} · {:.8} BTC", flow.note, units);
    flow.quantity = Some(units);
    flow
}

/// Today's value of the crypto account: units held × market price.
pub fn crypto_valuation(
    holding: Holding,
    market_price: Decimal,
    today: NaiveDate,
) -> SyncedValuation {
    let quantity = holding.quantity.max(Decimal::ZERO);
    SyncedValuation {
        account_name: CRYPTO.0.to_string(),
        account_kind: CRYPTO.1.to_string(),
        date: today,
        amount: (quantity * market_price).round_dp(2),
        note: format!("{:.8} BTC × R$ {:.2}", quantity, market_price),
    }
}

/// Pluggy dates are UTC instants; Brazil has been a fixed UTC-3 since 2019.
fn local_date(instant: DateTime<FixedOffset>) -> NaiveDate {
    let brt = FixedOffset::west_opt(3 * 3600).expect("UTC-3 is a valid offset");
    instant.with_timezone(&brt).date_naive()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 28).unwrap()
    }

    fn categories() -> HashMap<String, Category> {
        [
            ("11020000", "Food delivery", Some("Food and drinks")),
            ("10000000", "Groceries", None),
            ("04000000", "Same person transfer", None),
            ("05000000", "Transfers", None),
        ]
        .into_iter()
        .map(|(id, description, parent)| {
            (
                id.to_string(),
                Category {
                    id: id.to_string(),
                    description: description.to_string(),
                    parent_description: parent.map(str::to_string),
                },
            )
        })
        .collect()
    }

    fn tx(date: &str, description: &str, amount: &str, kind: &str, category: &str) -> Transaction {
        Transaction {
            id: format!("id-{}", description),
            date: date.to_string(),
            description: description.to_string(),
            amount: Decimal::from_str(amount).unwrap(),
            kind: kind.to_string(),
            category_id: Some(category.to_string()),
        }
    }

    fn map(t: &Transaction) -> Mapped {
        map_transaction(t, &categories(), today(), None)
    }

    fn expect_transaction(mapped: Mapped) -> TransactionDraft {
        match mapped {
            Mapped::Transaction(synced) => synced.draft,
            other => panic!("expected a transaction, got {:?}", other),
        }
    }

    fn expect_flow(mapped: Mapped) -> SyncedFlow {
        match mapped {
            Mapped::Flow(flow) => flow,
            other => panic!("expected a flow, got {:?}", other),
        }
    }

    #[test]
    fn card_purchase_is_an_expense_with_parent_and_child_category() {
        let draft = expect_transaction(map(&tx(
            "2026-01-15T12:00:00.000Z",
            "Ifd*Example Restaurant",
            "40.00",
            "DEBIT",
            "11020000",
        )));
        assert_eq!(draft.transaction_type, TransactionType::Expense);
        assert_eq!(draft.amount, Decimal::from_str("40.00").unwrap());
        assert_eq!(draft.category, "Food and drinks");
        assert_eq!(draft.subcategory, "Food delivery");
    }

    #[test]
    fn checking_debit_sign_is_dropped_and_top_level_category_has_no_subcategory() {
        let draft = expect_transaction(map(&tx(
            "2026-01-15T12:00:00.000Z",
            "Compra no débito|MERCADO EXEMPLO",
            "-18.5",
            "DEBIT",
            "10000000",
        )));
        assert_eq!(draft.amount, Decimal::from_str("18.50").unwrap());
        assert_eq!(draft.category, "Groceries");
        assert_eq!(draft.subcategory, "");
    }

    #[test]
    fn card_credit_with_negative_amount_is_income() {
        let draft = expect_transaction(map(&tx(
            "2026-01-15T12:00:00.000Z",
            "IOF de volta",
            "-1.2",
            "CREDIT",
            "10000000",
        )));
        assert_eq!(draft.transaction_type, TransactionType::Income);
        assert_eq!(draft.amount, Decimal::from_str("1.20").unwrap());
    }

    #[test]
    fn same_person_transfer_stays_income() {
        let draft = expect_transaction(map(&tx(
            "2026-01-15T12:00:00.000Z",
            "Transferência Recebida|FULANO DE TAL",
            "3000.00",
            "CREDIT",
            "04000000",
        )));
        assert_eq!(draft.transaction_type, TransactionType::Income);
        assert_eq!(draft.category, "Same person transfer");
    }

    #[test]
    fn date_is_taken_in_brasilia_time() {
        // 01:30 UTC on the 16th is still the evening of the 15th in Brazil.
        let draft = expect_transaction(map(&tx(
            "2026-01-16T01:30:00.000Z",
            "Late purchase",
            "10",
            "DEBIT",
            "10000000",
        )));
        assert_eq!(draft.date, NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
    }

    #[test]
    fn future_installments_are_skipped() {
        let t = tx(
            "2027-01-15T12:00:00.000Z",
            "Loja Exemplo 12/12",
            "30.00",
            "DEBIT",
            "10000000",
        );
        assert_eq!(map(&t), Mapped::Skipped);
    }

    #[test]
    fn bill_payment_is_skipped_on_both_sides() {
        let from_checking = tx(
            "2026-01-15T12:00:00.000Z",
            "Pagamento de fatura",
            "-2000.00",
            "DEBIT",
            "05000000",
        );
        let on_card = tx(
            "2026-01-15T12:00:00.000Z",
            "Pagamento recebido",
            "-2000.00",
            "CREDIT",
            CREDIT_CARD_PAYMENT,
        );
        assert_eq!(map(&from_checking), Mapped::Skipped);
        assert_eq!(map(&on_card), Mapped::Skipped);
    }

    #[test]
    fn generic_transfer_that_is_really_spending_stays_an_expense() {
        let draft = expect_transaction(map(&tx(
            "2026-01-15T12:00:00.000Z",
            "Plano de celular",
            "35",
            "DEBIT",
            "05000000",
        )));
        assert_eq!(draft.transaction_type, TransactionType::Expense);
    }

    #[test]
    fn rdb_moves_become_savings_flows() {
        let deposit = expect_flow(map(&tx(
            "2026-01-15T12:00:00.000Z",
            "Aplicação RDB",
            "-100",
            "DEBIT",
            "03000000",
        )));
        assert_eq!(deposit.entry_kind, InvestmentEntryKind::Contribution);
        assert_eq!(deposit.account_name, "Caixinhas / RDB");
        assert_eq!(deposit.amount, Decimal::from_str("100").unwrap());

        let redemption = expect_flow(map(&tx(
            "2026-01-15T12:00:00.000Z",
            "Resgate RDB",
            "500.00",
            "CREDIT",
            "03000000",
        )));
        assert_eq!(redemption.entry_kind, InvestmentEntryKind::Withdrawal);
        assert_eq!(redemption.account_name, "Caixinhas / RDB");
    }

    #[test]
    fn shared_balance_moves_are_skipped() {
        let deposit = tx(
            "2026-01-15T12:00:00.000Z",
            "Depósito no saldo compartilhado",
            "-100",
            "DEBIT",
            "05060000",
        );
        let redemption = tx(
            "2026-01-15T12:00:00.000Z",
            "Resgate no saldo compartilhado",
            "80",
            "CREDIT",
            "03000000",
        );
        assert_eq!(map(&deposit), Mapped::Skipped);
        assert_eq!(map(&redemption), Mapped::Skipped);
    }

    #[test]
    fn pix_on_credit_top_up_is_a_transaction_not_an_investment_flow() {
        // Pluggy files it under "Transfer - Internal", which is not an investment.
        let draft = expect_transaction(map(&tx(
            "2026-01-15T12:00:00.000Z",
            "Valor adicionado na conta por cartão de crédito | Valor adicionado para PIX no Crédito",
            "150",
            "CREDIT",
            "05060000",
        )));
        assert_eq!(draft.transaction_type, TransactionType::Income);
    }

    #[test]
    fn stock_purchase_miscategorized_as_shopping_is_a_brokerage_flow() {
        let flow = expect_flow(map(&tx(
            "2026-01-15T12:00:00.000Z",
            "Compra de Renda Variável",
            "-1000.00",
            "DEBIT",
            "08000000",
        )));
        assert_eq!(flow.account_name, "Renda Variável");
        assert_eq!(flow.entry_kind, InvestmentEntryKind::Contribution);
    }

    #[test]
    fn crypto_is_only_tracked_after_the_opening_date() {
        let purchase = |date: &str| tx(date, "Compra de criptomoedas", "-200", "DEBIT", "03040000");
        let opening = Some(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        let map_with = |t: &Transaction, since| map_transaction(t, &categories(), today(), since);

        // No opening quantity: crypto isn't tracked at all.
        assert_eq!(map(&purchase("2026-01-20T12:00:00.000Z")), Mapped::Skipped);
        // On the opening day it's already part of the opening quantity.
        assert_eq!(
            map_with(&purchase("2026-01-15T12:00:00.000Z"), opening),
            Mapped::Skipped
        );

        match map_with(&purchase("2026-01-20T12:34:56.000Z"), opening) {
            Mapped::CryptoFlow { flow, at } => {
                assert_eq!(flow.account_name, "Cripto");
                assert_eq!(flow.entry_kind, InvestmentEntryKind::Contribution);
                assert_eq!(flow.quantity, None);
                assert_eq!(at.to_rfc3339(), "2026-01-20T12:34:56+00:00");
            }
            other => panic!("expected a crypto flow, got {:?}", other),
        }
    }

    fn crypto_flow(kind: InvestmentEntryKind, amount: &str) -> SyncedFlow {
        SyncedFlow {
            pluggy_id: "c".to_string(),
            account_name: CRYPTO_ACCOUNT.to_string(),
            account_kind: "Crypto".to_string(),
            date: today(),
            entry_kind: kind,
            amount: Decimal::from_str(amount).unwrap(),
            note: "Compra de criptomoedas".to_string(),
            quantity: None,
        }
    }

    #[test]
    fn buys_pay_the_spread_above_market_and_sells_get_it_below() {
        let price = Decimal::from_str("400000").unwrap();
        // 404 / (400000 × 1.01) = 0.001 BTC exactly.
        let buy = price_crypto_flow(crypto_flow(InvestmentEntryKind::Contribution, "404"), price);
        assert_eq!(buy.quantity, Some(Decimal::from_str("0.001").unwrap()));
        assert_eq!(buy.note, "Compra de criptomoedas · 0.00100000 BTC");
        // 396 / (400000 × 0.99) = 0.001 BTC exactly.
        let sell = price_crypto_flow(crypto_flow(InvestmentEntryKind::Withdrawal, "396"), price);
        assert_eq!(sell.quantity, Some(Decimal::from_str("0.001").unwrap()));
    }

    #[test]
    fn crypto_valuation_is_units_times_market_price() {
        let holding = |units: &str| Holding {
            as_of: today(),
            quantity: Decimal::from_str(units).unwrap(),
        };
        let price = Decimal::from_str("433966").unwrap();
        let valuation = crypto_valuation(holding("0.00271012"), price, today());
        assert_eq!(valuation.account_name, "Cripto");
        assert_eq!(valuation.amount, Decimal::from_str("1176.10").unwrap());
        // Selling more than the model thought you had reads as empty, not negative.
        assert_eq!(
            crypto_valuation(holding("-0.0001"), price, today()).amount,
            Decimal::ZERO
        );
    }

    #[test]
    fn unknown_category_falls_back_to_uncategorized() {
        let draft = expect_transaction(map(&tx(
            "2026-01-15T12:00:00.000Z",
            "Mystery",
            "1",
            "DEBIT",
            "99999999",
        )));
        assert_eq!(draft.category, "Uncategorized");
    }

    fn holding(kind: &str, balance: Option<&str>) -> Investment {
        Investment {
            kind: kind.to_string(),
            balance: balance.map(|value| Decimal::from_str(value).unwrap()),
        }
    }

    #[test]
    fn holdings_sum_into_one_valuation_per_account() {
        let valuations = map_investments(
            &[
                holding("EQUITY", Some("1000.50")),
                holding("ETF", Some("200.25")),
                // Redeemed RDBs still report, at zero, so the account reads as emptied.
                holding("FIXED_INCOME", Some("0")),
                holding("FIXED_INCOME", None),
                holding("MUTUAL_FUND", Some("50")),
            ],
            today(),
        );
        let by_name: HashMap<&str, (&str, Decimal)> = valuations
            .iter()
            .map(|v| (v.account_name.as_str(), (v.account_kind.as_str(), v.amount)))
            .collect();

        assert_eq!(valuations.len(), 3);
        assert!(valuations.iter().all(|v| v.date == today()));
        assert_eq!(
            by_name["Renda Variável"],
            ("Brokerage", Decimal::from_str("1200.75").unwrap())
        );
        assert_eq!(by_name["Caixinhas / RDB"], ("Savings", Decimal::ZERO));
        assert_eq!(
            by_name["Outros investimentos"],
            ("Other", Decimal::from_str("50").unwrap())
        );
    }

    #[test]
    fn no_holdings_means_no_valuations() {
        assert!(map_investments(&[], today()).is_empty());
    }

    #[test]
    fn pluggy_amounts_parse_from_json_numbers() {
        let parsed: Transaction = serde_json::from_str(
            r#"{"id":"a","date":"2026-01-15T12:00:00.000Z","description":"x","amount":-1234.56,"type":"DEBIT","categoryId":null,"extra":1}"#,
        )
        .unwrap();
        assert_eq!(parsed.amount, Decimal::from_str("-1234.56").unwrap());
    }
}
