# Budget Tracker TUI

<p align="center">
  <img src="budget_tracker_icon.png" alt="Budget Tracker Logo" width="150"/>
</p>

<p align="center">
  <a href="https://ratatui.rs/"><img src="https://ratatui.rs/built-with-ratatui/badge.svg" alt="Built With Ratatui"></a>
</p>

A terminal app for tracking your personal budget, built with [Rust](https://www.rust-lang.org) and [Ratatui](https://ratatui.rs).

> **Credits.** This is a personal, independently maintained version of
> [**Budget Tracker TUI**](https://github.com/Feromond/budget-tracker-tui) by
> [**Jacob Mish** (Feromond)](https://github.com/Feromond), who designed and wrote the app: the
> transaction views, recurring transactions, categories, summaries and charts, budgets,
> investments, ledgers, CSV import/export, backups, the SQLite storage and the release tooling.
> Almost everything described in this README is Jacob's work. This repository started as a fork
> of the original and was split off in September 2026 because it had drifted too far to contribute back. It
> keeps the original code, its history and its license (GPL-3.0). If you want the original app, install
> it from [the original repository](https://github.com/Feromond/budget-tracker-tui),
> [crates.io](https://crates.io/crates/budget-tracker-tui) or
> [Homebrew](https://formulae.brew.sh/formula/budget-tracker) instead; those install the original
> version, not this one.

## What this version adds

Changes made in this repository (September 2026), on top of the original:

- **Automatic bank sync through [Pluggy](https://pluggy.ai)** (Open Finance Brasil), built for a
  Nubank checking account and credit card. It runs on startup and on `y`. Transactions arrive
  with Pluggy's categories, moves between checking and investments are recorded as investment
  contributions and withdrawals, and investment balances become one valuation per day. A re-sync
  never overwrites the category or description you edited.
- **Bitcoin tracking** for an investment account named `Cripto`: trades are priced at the
  Binance BTC/BRL price of the minute they happened, and the account is valued at the live price.
- **A first-run setup form** for the Pluggy credentials, which writes them to `pluggy.env`
  (readable only by you).
- **[Omarchy](https://omarchy.org) theme sync**: the app uses the active Omarchy theme's colors and
  follows a theme switch live.
- **Vim-style navigation** (`h`/`j`/`k`/`l`) on list and chart pages, a reworked monthly view,
  and investment account icons (a [Nerd Font](https://www.nerdfonts.com) is needed for the icons).

<p align="center">
  <img width="2000" height="1226" alt="Budget Tracker tour" src="https://github.com/user-attachments/assets/10387ade-007f-4ba0-b62b-a261a6acf46a" />
  <br><i>Mini App Tour gif</i><br><br>

</p>


## Screenshots

Screenshots are from the original project.

<p align="center">
  <img width="1000" height="614" alt="main-transaction-view" src="https://github.com/user-attachments/assets/96b58c49-10ff-4f7e-bdd7-b7c927aa9ba8" />
  <br><i>Main transaction view</i><br><br>

  <img width="1000" height="614" alt="budget-view" src="https://github.com/user-attachments/assets/89de97b1-03aa-465b-9e90-815c2361c524" />
  <br><i>Budget view</i><br><br>
</p>

<details><summary>More screenshots</summary>
<p align="center">
  <img width="1000" height="614" alt="category-summary-view" src="https://github.com/user-attachments/assets/bbb050b2-38d8-4936-8363-2b27b015bafc" />
  <br><i>Category summary</i><br><br>
  <img width="1000" height="614" alt="monthly-summary-view" src="https://github.com/user-attachments/assets/c8b8901d-07bb-46b1-a8ce-b1c8d92daba7" />
  <br><i>Monthly summary</i><br><br>
  <img width="1000" height="614" alt="multi-month-summary" src="https://github.com/user-attachments/assets/cf4ff51c-03f7-426c-afdf-ad61a60e9779" />
  <br><i>Multi-month line chart</i><br><br>
  <img width="1000" height="614" alt="cumulative-with-budget-line" src="https://github.com/user-attachments/assets/10ad584f-cd39-4e49-abea-63a4f358b3ff" />
  <br><i>Cumulative chart with budget line</i><br><br>
  <img width="1000" height="614" alt="cumulative-multi-summary" src="https://github.com/user-attachments/assets/acee89ae-22a7-4841-910b-03836f305175" />
  <br><i>Cumulative multi-month chart</i><br><br>
  <img width="1000" height="614" alt="investment-account-view" src="https://github.com/user-attachments/assets/e0e20782-97e6-492a-8ab8-e7ceab191f60" />
  <br><i>Investments</i><br><br>
  <img width="1000" height="614" alt="settings-with-help-menu-open" src="https://github.com/user-attachments/assets/c318d858-2de4-4d4c-b7c8-9d0e0da3c5d0" />
  <br><i>Settings with help menu open</i><br><br>
  <img width="1000" height="614" alt="fuzzy-search-categories" src="https://github.com/user-attachments/assets/ab985691-ca2f-4a07-bbc1-4c9ec3393b39" />
  <br><i>Fuzzy search for categories</i><br><br>
  <img width="1000" height="614" alt="category-catalog-with-budget-target" src="https://github.com/user-attachments/assets/d573a59f-0e53-4d1e-b67c-aa7baed59d77" />
  <br><i>Category catalog</i><br><br>
</p>
</details>

## Features

- Add, edit, delete, filter, and sort income and expense transactions
- Recurring transactions from daily to yearly, with optional forecasting
- Hierarchical categories and subcategories, editable in-app, with optional fuzzy search
- Monthly and category summaries with interactive charts
- Monthly and per-category budgets without changing past months
- Manual investment tracking for valuations, contributions, and growth
- Multiple ledgers for separate accounts or forecasts
- CSV import/export (duplicates skipped on import)
- Local SQLite storage with decimal arithmetic (no floating-point rounding errors)
- Fully keyboard-driven, with a built-in help menu
- Runs on Windows, macOS, and Linux; checks for new versions on startup

## Installation

This version is not published to crates.io or Homebrew yet. Build it from source with Rust
installed ([rustup.rs](https://rustup.rs)):

```bash
git clone https://github.com/humbertogfs55/budget-tracker-tui
cd budget-tracker-tui
cargo install --path .
```

This puts the `budget-tracker` command on your PATH.

## Usage

Launch with `budget-tracker`. The help bar at the bottom shows the keys for the current view, and `Ctrl+H` opens the full keybindings menu. Settings (`o`) is where you configure the database path, categories, CSV import/export, and display preferences. Budgets are set in the budget view (`b`).

For a more detailed walkthrough of every view and setting, see the [User Guide](docs/user-guide.md) (written for the original app, so it doesn't cover the additions above).

### Bank sync (Pluggy)

You need a Pluggy application (Client ID and Client Secret, from
[dashboard.pluggy.ai](https://dashboard.pluggy.ai)) and one Item ID per bank connection. On the
first launch without credentials a form asks for them and saves them to `pluggy.env` next to
`config.json`. Press `Esc` to skip it, and `y` later to open it again. The environment variables
`PLUGGY_CLIENT_ID`, `PLUGGY_CLIENT_SECRET` and `PLUGGY_ITEM_IDS` (comma-separated) take precedence
over the file. Without credentials the app works as before, just without sync.

### Omarchy themes

On [Omarchy](https://omarchy.org) the colors come from the active theme
(`~/.local/state/omarchy/current/theme/colors.toml`) and update within a second of
`omarchy-theme-set`. Elsewhere the app keeps its original terminal colors.

## Data & configuration

Transactions, categories, and investments live in a local SQLite database (`budget.db`), and app preferences in a `config.json`:

| OS      | Database                                       | Config                     |
| ------- | ---------------------------------------------- | -------------------------- |
| Linux   | `~/.local/share/BudgetTracker/`                | `~/.config/BudgetTracker/` |
| macOS   | `~/Library/Application Support/BudgetTracker/` | same                       |
| Windows | `%APPDATA%\BudgetTracker\`                     | same                       |

Bank sync credentials, when set up, are in `pluggy.env` in the config folder.

The database path is configurable in settings; point it at a cloud-synced folder (iCloud, Dropbox, etc.) to share your budget across devices. Changes are saved to the database immediately.

Older versions stored transactions in a `transactions.csv` file. On first launch, it is imported into the database automatically and renamed to `transactions.csv.migrated-backup`.

## CSV format

Import/export uses the columns `date, description, amount, transaction_type, category, subcategory`, with flexible date parsing. Import skips exact duplicates, so re-importing the same file is safe. Full details are in the [User Guide](docs/user-guide.md#csv-format).

## License and credits

Budget Tracker TUI was created by [Jacob Mish](https://github.com/Feromond) and is copyright its
original authors; the changes in this repository are copyright their authors. Both are licensed
under the GNU General Public License v3.0 only, the same license as the original. See
[LICENSE](LICENSE) for details. The commit history, including every commit from the original
project, shows who wrote what.
