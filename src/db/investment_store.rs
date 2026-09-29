use crate::db::database::SqliteDatabase;
use crate::model::{
    DATE_FORMAT, InvestmentAccount, InvestmentAccountDraft, InvestmentEntry, InvestmentEntryDraft,
    InvestmentEntryKind,
};
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, Row, params};
use rust_decimal::Decimal;
use std::io::{Error, ErrorKind, Result};
use std::str::FromStr;

/// A move between the bank account and an investment, pulled from the bank. It lands in the
/// named account (created on first use) as a contribution or withdrawal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedFlow {
    pub pluggy_id: String,
    pub account_name: String,
    pub account_kind: String,
    pub date: NaiveDate,
    pub entry_kind: InvestmentEntryKind,
    pub amount: Decimal,
    pub note: String,
    /// Units the trade moved (e.g. BTC), for accounts valued as units × price.
    pub quantity: Option<Decimal>,
}

/// An investment account's value on a day, as the sync computed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedValuation {
    pub account_name: String,
    pub account_kind: String,
    pub date: NaiveDate,
    pub amount: Decimal,
    pub note: String,
}

/// Units held in an account: the quantity on its latest opening valuation plus every synced
/// trade after that day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Holding {
    pub as_of: NaiveDate,
    pub quantity: Decimal,
}

pub trait InvestmentStore {
    fn list_accounts(&self) -> Result<Vec<InvestmentAccount>>;
    fn list_entries(&self) -> Result<Vec<InvestmentEntry>>;
    fn create_account(&self, draft: &InvestmentAccountDraft) -> Result<i64>;
    fn update_account(&self, id: i64, draft: &InvestmentAccountDraft) -> Result<()>;
    fn delete_account(&self, id: i64) -> Result<()>;
    /// Replaces any valuation the account already has on that date.
    fn save_entry(&self, draft: &InvestmentEntryDraft) -> Result<i64>;
    fn update_entry(&self, id: i64, draft: &InvestmentEntryDraft) -> Result<()>;
    fn delete_entry(&self, id: i64) -> Result<()>;
    /// Record bank flows not already recorded in this ledger. Returns how many were added.
    fn sync_flows(&self, flows: &[SyncedFlow]) -> Result<usize>;
    /// Record bank-reported values, replacing any valuation already on that day.
    /// Record computed values. One per account per day, replacing an earlier synced one but
    /// never a valuation entered by hand, which the user may have typed to correct the sync.
    /// Returns how many were written.
    fn sync_valuations(&self, valuations: &[SyncedValuation]) -> Result<usize>;
    /// Mark a valuation as the quantity-held anchor for its account.
    fn set_valuation_quantity(&self, entry_id: i64, quantity: Decimal) -> Result<()>;
    /// Units held in the named account, or None until it has a valuation with a quantity.
    fn holding(&self, account_name: &str) -> Result<Option<Holding>>;
}

pub struct SqliteInvestmentStore {
    database: SqliteDatabase,
    ledger_id: i64,
}

impl SqliteInvestmentStore {
    pub fn new(database: SqliteDatabase, ledger_id: i64) -> Self {
        Self {
            database,
            ledger_id,
        }
    }

    fn ready_connection(&self) -> Result<Connection> {
        self.database.ready_connection("investment")
    }

    fn row_to_account(row: &Row<'_>) -> rusqlite::Result<InvestmentAccount> {
        Ok(InvestmentAccount {
            id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            position: row.get(3)?,
            archived: row.get::<_, i64>(4)? != 0,
        })
    }

    fn row_to_entry(row: &Row<'_>) -> rusqlite::Result<InvestmentEntry> {
        Ok(InvestmentEntry {
            id: row.get(0)?,
            account_id: row.get(1)?,
            date: parse_date(2, &row.get::<_, String>(2)?)?,
            entry_kind: parse_kind(3, &row.get::<_, String>(3)?)?,
            amount: parse_decimal(4, &row.get::<_, String>(4)?)?,
            note: row.get(5)?,
        })
    }

    /// The id of the named account, creating it (last in order) if this ledger has none.
    /// Names are unique per ledger, case-insensitively.
    fn synced_account_id(&self, conn: &Connection, name: &str, kind: &str) -> Result<i64> {
        conn.execute(
            "
            INSERT INTO investment_accounts (ledger_id, name, kind, position)
            SELECT ?1, ?2, ?3, COALESCE(MAX(position), -1) + 1
            FROM investment_accounts WHERE ledger_id = ?1
            ON CONFLICT(ledger_id, name) DO NOTHING
            ",
            params![self.ledger_id, name, kind],
        )
        .map_err(|err| Error::other(format!("Failed to create investment account: {}", err)))?;
        conn.query_row(
            "SELECT id FROM investment_accounts WHERE ledger_id = ?1 AND name = ?2",
            params![self.ledger_id, name],
            |row| row.get(0),
        )
        .map_err(|err| Error::other(format!("Failed to find investment account: {}", err)))
    }

    fn assert_owns_account(&self, conn: &Connection, account_id: i64) -> Result<()> {
        let owned: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM investment_accounts WHERE id = ?1 AND ledger_id = ?2",
                params![account_id, self.ledger_id],
                |row| row.get(0),
            )
            .map_err(|err| Error::other(format!("Failed to check investment account: {}", err)))?;

        if owned == 0 {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Investment account {} was not found.", account_id),
            ));
        }
        Ok(())
    }
}

impl InvestmentStore for SqliteInvestmentStore {
    fn list_accounts(&self) -> Result<Vec<InvestmentAccount>> {
        let conn = self.ready_connection()?;
        let mut stmt = conn
            .prepare(
                "
                SELECT id, name, kind, position, archived
                FROM investment_accounts
                WHERE ledger_id = ?1
                ORDER BY position, id
                ",
            )
            .map_err(|err| Error::other(format!("Failed to prepare account query: {}", err)))?;

        stmt.query_map([self.ledger_id], Self::row_to_account)
            .map_err(|err| Error::other(format!("Failed to load investment accounts: {}", err)))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|err| Error::other(format!("Failed to read investment accounts: {}", err)))
    }

    fn list_entries(&self) -> Result<Vec<InvestmentEntry>> {
        let conn = self.ready_connection()?;
        let mut stmt = conn
            .prepare(
                "
                SELECT e.id, e.account_id, e.date, e.entry_kind, e.amount, e.note
                FROM investment_entries e
                JOIN investment_accounts a ON a.id = e.account_id
                WHERE a.ledger_id = ?1
                ORDER BY e.account_id, e.date, e.id
                ",
            )
            .map_err(|err| Error::other(format!("Failed to prepare entry query: {}", err)))?;

        stmt.query_map([self.ledger_id], Self::row_to_entry)
            .map_err(|err| Error::other(format!("Failed to load investment entries: {}", err)))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|err| Error::other(format!("Failed to read investment entries: {}", err)))
    }

    fn create_account(&self, draft: &InvestmentAccountDraft) -> Result<i64> {
        let name = validate_name(&draft.name)?;
        let conn = self.ready_connection()?;
        let position: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) + 1 FROM investment_accounts WHERE ledger_id = ?1",
                [self.ledger_id],
                |row| row.get(0),
            )
            .map_err(|err| Error::other(format!("Failed to position new account: {}", err)))?;

        conn.execute(
            "
            INSERT INTO investment_accounts (ledger_id, name, kind, position, archived)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                self.ledger_id,
                &name,
                draft.kind.trim(),
                position,
                draft.archived as i64
            ],
        )
        .map_err(|err| name_conflict(err, &name, "create"))?;

        Ok(conn.last_insert_rowid())
    }

    fn update_account(&self, id: i64, draft: &InvestmentAccountDraft) -> Result<()> {
        let name = validate_name(&draft.name)?;
        let conn = self.ready_connection()?;
        let updated = conn
            .execute(
                "
                UPDATE investment_accounts
                SET name = ?1, kind = ?2, archived = ?3
                WHERE id = ?4 AND ledger_id = ?5
                ",
                params![
                    &name,
                    draft.kind.trim(),
                    draft.archived as i64,
                    id,
                    self.ledger_id
                ],
            )
            .map_err(|err| name_conflict(err, &name, "rename"))?;

        if updated == 0 {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Investment account {} was not found.", id),
            ));
        }
        Ok(())
    }

    fn delete_account(&self, id: i64) -> Result<()> {
        let conn = self.ready_connection()?;
        let deleted = conn
            .execute(
                "DELETE FROM investment_accounts WHERE id = ?1 AND ledger_id = ?2",
                params![id, self.ledger_id],
            )
            .map_err(|err| Error::other(format!("Failed to delete investment account: {}", err)))?;

        if deleted == 0 {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Investment account {} was not found.", id),
            ));
        }
        Ok(())
    }

    fn save_entry(&self, draft: &InvestmentEntryDraft) -> Result<i64> {
        let amount = validate_amount(draft.amount, draft.entry_kind)?;
        let mut conn = self.ready_connection()?;
        self.assert_owns_account(&conn, draft.account_id)?;

        let tx = conn
            .transaction()
            .map_err(|err| Error::other(format!("Failed to begin entry write: {}", err)))?;

        // A partial unique index can't be an ON CONFLICT target, so clear then insert.
        if draft.entry_kind == InvestmentEntryKind::Valuation {
            tx.execute(
                "
                DELETE FROM investment_entries
                WHERE account_id = ?1 AND date = ?2 AND entry_kind = 'Valuation'
                ",
                params![draft.account_id, draft.date.format(DATE_FORMAT).to_string()],
            )
            .map_err(|err| Error::other(format!("Failed to replace valuation: {}", err)))?;
        }

        tx.execute(
            "
            INSERT INTO investment_entries (account_id, date, entry_kind, amount, note)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                draft.account_id,
                draft.date.format(DATE_FORMAT).to_string(),
                draft.entry_kind.as_str(),
                amount.to_string(),
                draft.note.trim()
            ],
        )
        .map_err(|err| Error::other(format!("Failed to save investment entry: {}", err)))?;

        let id = tx.last_insert_rowid();
        tx.commit()
            .map_err(|err| Error::other(format!("Failed to commit entry write: {}", err)))?;
        Ok(id)
    }

    fn update_entry(&self, id: i64, draft: &InvestmentEntryDraft) -> Result<()> {
        let amount = validate_amount(draft.amount, draft.entry_kind)?;
        let mut conn = self.ready_connection()?;
        self.assert_owns_account(&conn, draft.account_id)?;

        let tx = conn
            .transaction()
            .map_err(|err| Error::other(format!("Failed to begin entry update: {}", err)))?;

        if draft.entry_kind == InvestmentEntryKind::Valuation {
            tx.execute(
                "
                DELETE FROM investment_entries
                WHERE account_id = ?1 AND date = ?2 AND entry_kind = 'Valuation' AND id != ?3
                ",
                params![
                    draft.account_id,
                    draft.date.format(DATE_FORMAT).to_string(),
                    id
                ],
            )
            .map_err(|err| Error::other(format!("Failed to replace valuation: {}", err)))?;
        }

        let updated = tx
            .execute(
                "
                UPDATE investment_entries
                SET account_id = ?1, date = ?2, entry_kind = ?3, amount = ?4, note = ?5
                WHERE id = ?6 AND account_id IN (
                    SELECT id FROM investment_accounts WHERE ledger_id = ?7
                )
                ",
                params![
                    draft.account_id,
                    draft.date.format(DATE_FORMAT).to_string(),
                    draft.entry_kind.as_str(),
                    amount.to_string(),
                    draft.note.trim(),
                    id,
                    self.ledger_id
                ],
            )
            .map_err(|err| Error::other(format!("Failed to update investment entry: {}", err)))?;

        if updated == 0 {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Investment entry {} was not found.", id),
            ));
        }

        tx.commit()
            .map_err(|err| Error::other(format!("Failed to commit entry update: {}", err)))
    }

    fn delete_entry(&self, id: i64) -> Result<()> {
        let conn = self.ready_connection()?;
        let deleted = conn
            .execute(
                "
                DELETE FROM investment_entries
                WHERE id = ?1 AND account_id IN (
                    SELECT id FROM investment_accounts WHERE ledger_id = ?2
                )
                ",
                params![id, self.ledger_id],
            )
            .map_err(|err| Error::other(format!("Failed to delete investment entry: {}", err)))?;

        if deleted == 0 {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Investment entry {} was not found.", id),
            ));
        }
        Ok(())
    }

    fn sync_flows(&self, flows: &[SyncedFlow]) -> Result<usize> {
        let mut conn = self.ready_connection()?;
        let tx = conn
            .transaction()
            .map_err(|err| Error::other(format!("Failed to begin investment sync: {}", err)))?;

        let mut added = 0;
        for flow in flows {
            let amount = validate_amount(flow.amount, flow.entry_kind)?;
            let already_synced = tx
                .query_row(
                    "
                    SELECT 1 FROM investment_entries e
                    JOIN investment_accounts a ON a.id = e.account_id
                    WHERE a.ledger_id = ?1 AND e.pluggy_id = ?2
                    ",
                    params![self.ledger_id, &flow.pluggy_id],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|err| Error::other(format!("Failed to look up synced entry: {}", err)))?
                .is_some();
            if already_synced {
                continue;
            }

            let account_id = self.synced_account_id(&tx, &flow.account_name, &flow.account_kind)?;

            tx.execute(
                "
                INSERT INTO investment_entries
                    (account_id, date, entry_kind, amount, note, pluggy_id, quantity)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ",
                params![
                    account_id,
                    flow.date.format(DATE_FORMAT).to_string(),
                    flow.entry_kind.as_str(),
                    amount.to_string(),
                    flow.note.trim(),
                    &flow.pluggy_id,
                    flow.quantity
                        .map(|quantity| quantity.normalize().to_string())
                ],
            )
            .map_err(|err| Error::other(format!("Failed to save synced entry: {}", err)))?;
            added += 1;
        }

        tx.commit()
            .map_err(|err| Error::other(format!("Failed to commit investment sync: {}", err)))?;
        Ok(added)
    }

    fn sync_valuations(&self, valuations: &[SyncedValuation]) -> Result<usize> {
        let mut conn = self.ready_connection()?;
        let tx = conn
            .transaction()
            .map_err(|err| Error::other(format!("Failed to begin valuation sync: {}", err)))?;

        let mut written = 0;

        for valuation in valuations {
            let amount = validate_amount(valuation.amount, InvestmentEntryKind::Valuation)?;
            let account_id =
                self.synced_account_id(&tx, &valuation.account_name, &valuation.account_kind)?;
            let date = valuation.date.format(DATE_FORMAT).to_string();
            let hand_entered = tx
                .query_row(
                    "
                    SELECT 1 FROM investment_entries
                    WHERE account_id = ?1 AND date = ?2 AND entry_kind = 'Valuation'
                      AND pluggy_id IS NULL
                    ",
                    params![account_id, &date],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|err| Error::other(format!("Failed to check valuation: {}", err)))?
                .is_some();
            if hand_entered {
                continue;
            }

            // A partial unique index can't be an ON CONFLICT target, so clear then insert.
            tx.execute(
                "
                DELETE FROM investment_entries
                WHERE account_id = ?1 AND date = ?2 AND entry_kind = 'Valuation'
                ",
                params![account_id, &date],
            )
            .map_err(|err| Error::other(format!("Failed to replace valuation: {}", err)))?;
            tx.execute(
                "
                INSERT INTO investment_entries (account_id, date, entry_kind, amount, note, pluggy_id)
                VALUES (?1, ?2, 'Valuation', ?3, ?4, ?5)
                ",
                params![
                    account_id,
                    &date,
                    amount.to_string(),
                    valuation.note.trim(),
                    format!("valuation:{}", date)
                ],
            )
            .map_err(|err| Error::other(format!("Failed to save synced valuation: {}", err)))?;
            written += 1;
        }

        tx.commit()
            .map_err(|err| Error::other(format!("Failed to commit valuation sync: {}", err)))?;
        Ok(written)
    }

    fn set_valuation_quantity(&self, entry_id: i64, quantity: Decimal) -> Result<()> {
        if quantity < Decimal::ZERO {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Quantity cannot be negative.",
            ));
        }
        let conn = self.ready_connection()?;
        let updated = conn
            .execute(
                "
                UPDATE investment_entries SET quantity = ?1
                WHERE id = ?2 AND entry_kind = 'Valuation' AND account_id IN (
                    SELECT id FROM investment_accounts WHERE ledger_id = ?3
                )
                ",
                params![quantity.normalize().to_string(), entry_id, self.ledger_id],
            )
            .map_err(|err| Error::other(format!("Failed to save quantity: {}", err)))?;
        if updated == 0 {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Valuation {} was not found.", entry_id),
            ));
        }
        Ok(())
    }

    fn holding(&self, account_name: &str) -> Result<Option<Holding>> {
        let conn = self.ready_connection()?;
        let anchor = conn
            .query_row(
                "
                SELECT e.account_id, e.date, e.quantity
                FROM investment_entries e
                JOIN investment_accounts a ON a.id = e.account_id
                WHERE a.ledger_id = ?1 AND a.name = ?2
                  AND e.entry_kind = 'Valuation' AND e.quantity IS NOT NULL
                ORDER BY e.date DESC, e.id DESC
                LIMIT 1
                ",
                params![self.ledger_id, account_name],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        parse_date(1, &row.get::<_, String>(1)?)?,
                        parse_decimal(2, &row.get::<_, String>(2)?)?,
                    ))
                },
            )
            .optional()
            .map_err(|err| Error::other(format!("Failed to load holding: {}", err)))?;
        let Some((account_id, as_of, mut quantity)) = anchor else {
            return Ok(None);
        };

        let mut stmt = conn
            .prepare(
                "
                SELECT entry_kind, quantity FROM investment_entries
                WHERE account_id = ?1 AND date > ?2 AND quantity IS NOT NULL
                  AND entry_kind IN ('Contribution', 'Withdrawal')
                ",
            )
            .map_err(|err| Error::other(format!("Failed to prepare holding query: {}", err)))?;
        let trades = stmt
            .query_map(
                params![account_id, as_of.format(DATE_FORMAT).to_string()],
                |row| {
                    Ok((
                        parse_kind(0, &row.get::<_, String>(0)?)?,
                        parse_decimal(1, &row.get::<_, String>(1)?)?,
                    ))
                },
            )
            .map_err(|err| Error::other(format!("Failed to load trades: {}", err)))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|err| Error::other(format!("Failed to read trades: {}", err)))?;
        for (kind, units) in trades {
            quantity += units * kind.flow_sign();
        }
        Ok(Some(Holding { as_of, quantity }))
    }
}

fn validate_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Account name cannot be empty.",
        ));
    }
    Ok(trimmed.to_string())
}

/// Zero is a real valuation (you sold out) but never a real flow.
fn validate_amount(amount: Decimal, kind: InvestmentEntryKind) -> Result<Decimal> {
    if amount < Decimal::ZERO {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Amount cannot be negative. Use a withdrawal to take money out.",
        ));
    }
    if amount.is_zero() && kind != InvestmentEntryKind::Valuation {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("A {} needs an amount.", kind.as_str().to_lowercase()),
        ));
    }
    Ok(amount)
}

fn name_conflict(err: rusqlite::Error, name: &str, action: &str) -> Error {
    match err {
        rusqlite::Error::SqliteFailure(inner, _)
            if inner.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            Error::new(
                ErrorKind::AlreadyExists,
                format!("An investment account named '{}' already exists.", name),
            )
        }
        other => Error::other(format!(
            "Failed to {} investment account: {}",
            action, other
        )),
    }
}

fn parse_date(index: usize, value: &str) -> rusqlite::Result<NaiveDate> {
    NaiveDate::parse_from_str(value, DATE_FORMAT).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(Error::new(
                ErrorKind::InvalidData,
                format!("Invalid date '{}' in investment database: {}", value, err),
            )),
        )
    })
}

fn parse_decimal(index: usize, value: &str) -> rusqlite::Result<Decimal> {
    Decimal::from_str(value.trim()).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(Error::new(
                ErrorKind::InvalidData,
                format!("Invalid amount '{}' in investment database: {}", value, err),
            )),
        )
    })
}

fn parse_kind(index: usize, value: &str) -> rusqlite::Result<InvestmentEntryKind> {
    InvestmentEntryKind::from_label(value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(Error::new(
                ErrorKind::InvalidData,
                format!("Invalid entry kind '{}' in investment database.", value),
            )),
        )
    })
}
