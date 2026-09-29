//! Turns Pluggy transactions into what the app stores: a regular transaction, a move into or
//! out of an investment account, or nothing. Pure, so every rule is unit-tested.

use super::{Category, Transaction};
use crate::db::investment_store::SyncedFlow;
use crate::db::transaction_store::SyncedTransaction;
use crate::model::{InvestmentEntryKind, TransactionDraft, TransactionType};
use chrono::{DateTime, FixedOffset, NaiveDate};
use std::collections::HashMap;

const CREDIT_CARD_PAYMENT: &str = "05100000";
const TRANSFER_INTERNAL: &str = "05060000";
const INVESTMENTS_PREFIX: &str = "03";

#[derive(Debug, PartialEq)]
pub enum Mapped {
    Transaction(SyncedTransaction),
    Flow(SyncedFlow),
    Skipped,
}

pub fn map_transaction(
    tx: &Transaction,
    categories: &HashMap<String, Category>,
    today: NaiveDate,
) -> Mapped {
    let Some(date) = local_date(&tx.date) else {
        return Mapped::Skipped;
    };
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
    if category_id.starts_with(INVESTMENTS_PREFIX)
        || category_id == TRANSFER_INTERNAL
        || lower.contains("renda variável")
    {
        let (account_name, account_kind) = investment_account_for(&lower);
        return Mapped::Flow(SyncedFlow {
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
        });
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
        ("Cripto", "Crypto")
    } else if lower_description.contains("renda variável") {
        ("Renda Variável", "Brokerage")
    } else {
        ("Caixinhas / RDB", "Savings")
    }
}

/// Pluggy dates are UTC instants; Brazil has been a fixed UTC-3 since 2019.
fn local_date(timestamp: &str) -> Option<NaiveDate> {
    let brt = FixedOffset::west_opt(3 * 3600)?;
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|instant| instant.with_timezone(&brt).date_naive())
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
        map_transaction(t, &categories(), today())
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
    fn caixinha_moves_become_savings_flows() {
        let deposit = expect_flow(map(&tx(
            "2026-01-15T12:00:00.000Z",
            "Depósito no saldo compartilhado",
            "-100",
            "DEBIT",
            TRANSFER_INTERNAL,
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
    fn crypto_purchase_is_a_crypto_flow() {
        let flow = expect_flow(map(&tx(
            "2026-01-15T12:00:00.000Z",
            "Compra de criptomoedas",
            "-200",
            "DEBIT",
            "03040000",
        )));
        assert_eq!(flow.account_name, "Cripto");
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

    #[test]
    fn pluggy_amounts_parse_from_json_numbers() {
        let parsed: Transaction = serde_json::from_str(
            r#"{"id":"a","date":"2026-01-15T12:00:00.000Z","description":"x","amount":-1234.56,"type":"DEBIT","categoryId":null,"extra":1}"#,
        )
        .unwrap();
        assert_eq!(parsed.amount, Decimal::from_str("-1234.56").unwrap());
    }
}
