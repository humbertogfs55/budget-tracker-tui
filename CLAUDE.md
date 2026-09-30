# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

A Rust + Ratatui terminal budget tracker (binary `budget-tracker`), forked from `Feromond/budget-tracker-tui` (`upstream` remote; `origin` is the fork). The fork adds automatic bank sync through Pluggy (Open Finance Brasil aggregator) for a Nubank checking account, credit card, investments and bitcoin. Everything under "Bank sync" below is fork-only.

## Commands

```bash
cargo build
cargo run                               # starts the TUI (and a background bank sync if configured)
cargo test                              # all tests (single binary crate, no lib)
cargo test mapping::tests               # one module
cargo test sync_merge_updates_by_pluggy_id_and_keeps_user_edits   # one test
cargo fmt --check && cargo clippy --all-targets -- -D warnings    # what CI enforces
```

CI (`.github/workflows/rust.yml`) runs fmt check, clippy with `-D warnings`, build and tests. A release bumps `version` in both `[package]` and `[package.metadata.bundle]` of `Cargo.toml` (see `.github/RELEASING.md`).

Runtime locations: config `~/.config/BudgetTracker/config.json`, database `~/.local/share/BudgetTracker/budget.db` (default; overridable in settings), backups next to it in `backups/`. Deleting `budget.db` is the way to reset; the app recreates it with seed categories from `budget_categories.csv`.

## Architecture

**One state struct, split by feature.** `App` in `src/app/state.rs` holds all UI and data state. Each feature adds an `impl App` block in its own file under `src/app/` (budget, investments, filter, bank_sync, ...). `AppMode` decides both which key handler runs (`src/events/<mode>_mode.rs`, dispatched from `src/events/runner.rs`) and what `src/ui/mod.rs` draws.

**Key filtering gotcha.** `runner.rs` drops key events with modifiers unless the mode is explicitly allow-listed in a long condition there. A new Shift/Ctrl binding does nothing until it is added to that list. New keys also need an entry in `src/app/help.rs` (Ctrl+H menu) and possibly the footer in `src/ui/help.rs`.

**Forms.** Form fields are declared with the `form_fields!` macro in `src/app/fields.rs`; the matching `FieldSet<Field, N>` in `state.rs` must have `N` equal to the field count. `src/ui/form.rs` renders any field set (it scrolls when fields don't fit).

**Persistence.** SQLite via `rusqlite`, one trait + `Sqlite*Store` per entity in `src/db/`. Stores are cheap values built on demand (`app.transaction_store()`, `app.investment_store()`), each opening its own connection through `ready_connection`, which runs pending migrations. Transactions, budgets and investments are scoped to a ledger; the category catalog is shared by all ledgers. Money is always `rust_decimal::Decimal`, stored as text.

**Migrations.** `src/db/database.rs`: `SCHEMA_VERSION` plus `apply_migration(version)`, tracked with `PRAGMA user_version`. Migrations must be idempotent and tolerate hand-built legacy databases (tests create v3 databases missing tables), hence `ensure_column` / `table_exists` guards. Copying a ledger (`src/db/ledger_store.rs`) lists columns explicitly, so new columns must be added there too.

**Derived data is never stored.** Only real transactions are persisted; recurring occurrences are generated in memory (`generate_recurring_transactions`). Investments store valuations and cash flows separately (`investment_entries.entry_kind`); value = last valuation + flows after it, invested = sum of flows, gain is derived (`Portfolio` in `src/model.rs`). Only one valuation per account per day (partial unique index).

**Colors.** UI code never uses `Color::*` directly; it reads `theme::current()` (`src/theme.rs`), a process-wide palette with ANSI-named hues (`bright_green` = old `LightGreen`) plus roles (`text`, `muted`, `accent`, `header_bg`, `panel_bg`, ...). `ThemeWatcher`, polled from the runner loop, rereads `$XDG_STATE_HOME/omarchy/current/theme/colors.toml` once a second, so an `omarchy-theme-set` applies live. Without that file the defaults are the original named colors. Keys that are missing or aren't a valid hex color keep their default.

**Tests.** Most store/integration tests live in the `tests` module of `src/db/transaction_store.rs` using `TempDb` (a throwaway on-disk DB). Pure mapping rules are tested in `src/pluggy/mapping.rs`. Test fixtures use invented names and amounts, never real bank data.

## Bank sync (fork only)

- **Credentials**: env vars `PLUGGY_CLIENT_ID`, `PLUGGY_CLIENT_SECRET`, `PLUGGY_ITEM_IDS` (comma-separated), falling back to `~/.config/BudgetTracker/pluggy.env`. Never read or print that file. Missing credentials silently disable sync. On a first run (no `pluggy.env`, `bank_sync_setup_skipped` not set in config.json), and on `y` without credentials, a `BankSyncSetup` form opens. It writes the file through `pluggy::save_credentials` (0600, temp file then rename). Esc sets the skip flag. The Settings save rebuilds `AppSettings`, so it must carry that flag over.
- **Flow**: runs on startup and on `y` in normal mode. `src/app/bank_sync.rs` spawns a thread (same `mpsc` pattern as `update_checker`) that calls `pluggy::fetch`, maps everything and prices crypto; the runner polls `poll_bank_sync` and the main thread only writes. First sync asks 10 years back (Pluggy returns ~14 months for Nubank); later syncs re-read 30 days before the last sync (stored per ledger in `database_meta`).
- **Pluggy API** (`src/pluggy/mod.rs`): must use `GET /v2/transactions` with the `next` cursor; v1 `/transactions` returns HTTP 410.
- **Mapping rules** (`src/pluggy/mapping.rs`, pure): future-dated card installments, card bill payments ("Pagamento de fatura" / category `05100000`) and "saldo compartilhado" rows are skipped. Category prefix `03` or "Renda Variável" in the description become investment flows into "Caixinhas / RDB", "Renda Variável" or "Cripto". `05060000` (Transfer - Internal) is **not** an investment. Everything else is a transaction with Pluggy's category tree as category/subcategory (auto-added to the catalog). Dates convert from UTC to fixed UTC-3.
- **Idempotency**: synced rows carry `pluggy_id` (schema v6, unique per ledger/account). Re-sync updates only date, amount and type, so user edits to category/description survive. A deleted synced row comes back if still inside the sync window.
- **Valuations**: `/investments` balances summed per account become one daily valuation. Synced valuations have `pluggy_id = "valuation:<date>"`; `sync_valuations` never replaces a hand-entered one (`pluggy_id IS NULL`) on the same day.
- **Bitcoin**: Pluggy has no crypto holdings, only "Compra/Venda de criptomoedas" checking rows. Tracking starts only once an investment account named `Cripto` exists with a "Quantity Held" opening (stored in `investment_entries.quantity`, schema v7, on the opening valuation). Trades after that date are priced at the Binance BTCBRL 1-minute candle for their Pluggy timestamp (`src/market_price.rs`) with `CRYPTO_SPREAD` (1%, a lower bound estimated from history); the account is valued at holding × live price.
- **UI**: investment accounts show a Nerd Font icon chosen from their Type (`kind_icon` in `src/ui/investments.rs`).
