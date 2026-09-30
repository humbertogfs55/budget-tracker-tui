use crate::app::state::{App, AppMode};
use crate::theme;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render_help_bar(f: &mut Frame, app: &App, area: Rect) {
    let help_spans = match app.mode {
        AppMode::Normal => vec![
            Span::styled("↑↓ Nav | ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                "a",
                Style::default()
                    .fg(theme::current().bright_green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Add | "),
            Span::styled(
                "e",
                Style::default()
                    .fg(theme::current().bright_yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Edit | "),
            Span::styled(
                "d",
                Style::default()
                    .fg(theme::current().bright_red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Del | "),
            Span::styled(
                "r",
                Style::default()
                    .fg(theme::current().bright_blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Rcr | "),
            Span::styled(
                "f",
                Style::default()
                    .fg(theme::current().cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Filt | "),
            Span::styled(
                "s",
                Style::default()
                    .fg(theme::current().bright_magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Mth | "),
            Span::styled(
                "c",
                Style::default()
                    .fg(theme::current().bright_cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Cate | "),
            Span::styled(
                "b",
                Style::default()
                    .fg(theme::current().bright_yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Budg | "),
            Span::styled(
                "i",
                Style::default()
                    .fg(theme::current().bright_cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Inv | "),
            Span::styled(
                "1-6",
                Style::default()
                    .fg(theme::current().bright_blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Srt | "),
            Span::styled(
                "q/Esc",
                Style::default()
                    .fg(theme::current().magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Quit | "),
            Span::styled("o", Style::default().fg(theme::current().red))
                .add_modifier(Modifier::BOLD),
            Span::raw(" ⚙"),
        ],
        AppMode::Adding | AppMode::Editing => vec![
            Span::raw("Tab/↑↓ Nav | "),
            Span::raw("←→ Toggle | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(" Save/Select | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(" Cancel"),
        ],
        AppMode::ConfirmDelete => vec![
            Span::styled("y", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Confirm | "),
            Span::styled("n/Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Cancel"),
        ],
        AppMode::Filtering => vec![
            Span::raw("← → Cursor | "),
            Span::raw("Bksp/Del Edit | "),
            Span::styled("Ctrl+F", Style::default().fg(theme::current().red)),
            Span::raw(" Adv Filt | "),
            Span::styled(
                "Ctrl+R",
                Style::default().fg(theme::current().bright_yellow),
            ),
            Span::raw(" Clear | "),
            Span::styled(
                "Enter/Esc",
                Style::default().fg(theme::current().bright_green),
            ),
            Span::raw(" Apply/Exit"),
        ],
        AppMode::AdvancedFiltering => vec![
            Span::raw("Tab/↑↓ Nav | "),
            Span::raw("← → Adjust | "),
            Span::styled(
                "Ctrl+R",
                Style::default().fg(theme::current().bright_yellow),
            ),
            Span::raw(" Clear | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(" Save | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(" Cancel"),
        ],
        AppMode::SelectingFilterCategory | AppMode::SelectingFilterSubcategory => vec![
            Span::raw("↑↓ Nav | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Confirm | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Cancel"),
        ],
        AppMode::SelectingCategory | AppMode::SelectingSubcategory => vec![
            Span::raw("↑↓ Nav | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Confirm | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Cancel"),
        ],
        AppMode::Summary => vec![
            Span::styled(
                "↑↓",
                Style::default()
                    .fg(theme::current().bright_yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Month | "),
            Span::styled(
                "←→",
                Style::default()
                    .fg(theme::current().bright_cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("/"),
            Span::styled(
                "[]",
                Style::default()
                    .fg(theme::current().bright_blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Year | "),
            Span::styled("m", Style::default().fg(theme::current().bright_blue)),
            Span::raw(" Multi | "),
            Span::styled("c", Style::default().fg(theme::current().bright_yellow)),
            Span::raw(" Cumu | "),
            Span::styled("q/Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(" Back"),
        ],
        AppMode::CategorySummary => vec![
            Span::styled(
                "↑↓",
                Style::default()
                    .fg(theme::current().bright_yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Nav | "),
            Span::styled(
                "←→",
                Style::default()
                    .fg(theme::current().bright_cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("/"),
            Span::styled(
                "[]",
                Style::default()
                    .fg(theme::current().bright_blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Year | "),
            Span::styled(
                "PgUp/PgDn",
                Style::default()
                    .fg(theme::current().bright_green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Month Jump | "),
            Span::styled(
                "1-6",
                Style::default()
                    .fg(theme::current().bright_magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Sort | "),
            Span::styled("Enter", Style::default().fg(theme::current().magenta)),
            Span::raw(" Drill Down | "),
            Span::styled("f", Style::default().fg(theme::current().bright_yellow)),
            Span::raw(" Filter List | "),
            Span::styled("q/Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(" Back"),
        ],
        AppMode::Budget => vec![
            Span::styled(
                "↑↓",
                Style::default()
                    .fg(theme::current().bright_yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Rows | "),
            Span::styled(
                "←→",
                Style::default()
                    .fg(theme::current().bright_cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Month | "),
            Span::styled(
                "Shift+←→",
                Style::default()
                    .fg(theme::current().bright_blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Year | "),
            Span::styled("e", Style::default().fg(theme::current().bright_yellow)),
            Span::raw(" Edit Budget | "),
            Span::styled("t", Style::default().fg(theme::current().bright_magenta)),
            Span::raw(" Edit Monthly | "),
            Span::styled("c", Style::default().fg(theme::current().bright_green)),
            Span::raw(" Edit Categories | "),
            Span::styled("q/Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(" Back"),
        ],
        AppMode::BudgetCategoryEditor => vec![
            Span::raw("Type amount | "),
            Span::raw("←→ Cursor | "),
            Span::styled("↑↓", Style::default().fg(theme::current().bright_blue)),
            Span::raw(" Scope | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(" Save | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(" Cancel"),
        ],
        AppMode::Settings => vec![
            Span::styled(
                "Tab/↑↓",
                Style::default()
                    .fg(theme::current().bright_yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Nav | "),
            Span::styled(
                "←/→",
                Style::default()
                    .fg(theme::current().bright_cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Cursor | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Save | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Cancel | "),
            Span::styled(
                "Ctrl+D",
                Style::default().fg(theme::current().bright_magenta),
            ),
            Span::raw(": Reset | "),
            Span::styled(
                "Ctrl+U",
                Style::default().fg(theme::current().bright_magenta),
            ),
            Span::raw(": Clear"),
        ],
        AppMode::CategoryCatalog => vec![
            Span::raw("↑↓ Nav | "),
            Span::styled("f", Style::default().fg(theme::current().cyan)),
            Span::raw(": Filter | "),
            Span::styled("a", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Add | "),
            Span::styled(
                "e/Enter",
                Style::default().fg(theme::current().bright_yellow),
            ),
            Span::raw(": Edit | "),
            Span::styled("d", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Delete | "),
            Span::styled("b", Style::default().fg(theme::current().bright_magenta)),
            Span::raw(": Budget | "),
            Span::styled("1-5", Style::default().fg(theme::current().bright_blue)),
            Span::raw(": Sort | "),
            Span::styled("q/Esc", Style::default().fg(theme::current().bright_cyan)),
            Span::raw(": Back"),
        ],
        AppMode::CategoryCatalogFilter => vec![
            Span::raw("Type to Filter | "),
            Span::raw("↑↓ Nav | "),
            Span::styled(
                "Ctrl+R",
                Style::default().fg(theme::current().bright_yellow),
            ),
            Span::raw(" Clear | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(" Apply | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(" Clear & Back"),
        ],
        AppMode::CategoryEditor => vec![
            Span::raw("Tab/↑↓ Nav | "),
            Span::raw("←→ Type/Cursor | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Toggle/Save | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Cancel"),
        ],
        AppMode::ConfirmCategoryDelete => vec![
            Span::styled("y", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Confirm | "),
            Span::styled("n/Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Cancel"),
        ],
        AppMode::Investments => vec![
            Span::raw("↑↓ Nav | "),
            Span::styled("←→", Style::default().fg(theme::current().magenta)),
            Span::raw(" Range | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_cyan)),
            Span::raw(" Detail | "),
            Span::styled("v", Style::default().fg(theme::current().bright_cyan)),
            Span::raw(" Value | "),
            Span::styled("a", Style::default().fg(theme::current().bright_green)),
            Span::raw(" Add | "),
            Span::styled("e", Style::default().fg(theme::current().bright_yellow)),
            Span::raw(" Edit | "),
            Span::styled("d", Style::default().fg(theme::current().bright_red)),
            Span::raw(" Del | "),
            Span::styled("q/Esc", Style::default().fg(theme::current().magenta)),
            Span::raw(" Back"),
        ],
        AppMode::InvestmentDetail => vec![
            Span::raw("↑↓ Entries | "),
            Span::styled("←→", Style::default().fg(theme::current().magenta)),
            Span::raw(" Range | "),
            Span::styled("v", Style::default().fg(theme::current().bright_cyan)),
            Span::raw(" Value | "),
            Span::styled("a", Style::default().fg(theme::current().bright_green)),
            Span::raw(" Add | "),
            Span::styled(
                "e/Enter",
                Style::default().fg(theme::current().bright_yellow),
            ),
            Span::raw(" Edit | "),
            Span::styled("d", Style::default().fg(theme::current().bright_red)),
            Span::raw(" Del | "),
            Span::styled("q/Esc", Style::default().fg(theme::current().magenta)),
            Span::raw(" Back"),
        ],
        AppMode::InvestmentAccountEditor | AppMode::InvestmentEntryEditor => vec![
            Span::raw("Tab/↑↓ Nav | "),
            Span::raw("←→ Adjust | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(" Toggle/Save | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(" Cancel"),
        ],
        AppMode::ConfirmInvestmentDelete => vec![
            Span::styled("y", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Confirm | "),
            Span::styled("n/Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Cancel"),
        ],
        AppMode::LedgerManager => vec![
            Span::raw("↑↓ Nav | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Switch | "),
            Span::styled("a", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Add | "),
            Span::styled("e", Style::default().fg(theme::current().bright_yellow)),
            Span::raw(": Rename | "),
            Span::styled("Ctrl+C", Style::default().fg(theme::current().bright_blue)),
            Span::raw(": Copy | "),
            Span::styled("d", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Delete | "),
            Span::styled("q/Esc", Style::default().fg(theme::current().bright_cyan)),
            Span::raw(": Back"),
        ],
        AppMode::BankSyncSetup => vec![
            Span::raw("Type or paste | "),
            Span::raw("Tab/↑↓ Fields | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Save and sync | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Skip"),
        ],
        AppMode::LedgerEditor => vec![
            Span::raw("Type a name | "),
            Span::raw("←→ Cursor | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Save | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Cancel"),
        ],
        AppMode::BackupManager => vec![
            Span::raw("↑↓ Nav | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Restore | "),
            Span::styled("b", Style::default().fg(theme::current().bright_blue)),
            Span::raw(": Back up now | "),
            Span::styled("d", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Delete | "),
            Span::styled("q/Esc", Style::default().fg(theme::current().bright_cyan)),
            Span::raw(": Back"),
        ],
        AppMode::ConfirmBackupRestore | AppMode::ConfirmBackupDelete => vec![
            Span::styled("y", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Confirm | "),
            Span::styled("n/Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Cancel"),
        ],
        AppMode::ConfirmLedgerDelete => vec![
            Span::styled("y", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Confirm | "),
            Span::styled("n/Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Cancel"),
        ],
        AppMode::RecurringSettings => vec![
            Span::raw("Tab/↑↓ Nav | "),
            Span::raw("←→ Toggle/Date | "),
            Span::raw("Shift+←→ Month | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(" Select/Save | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(" Cancel"),
        ],
        AppMode::SelectingRecurrenceFrequency => vec![
            Span::raw("↑↓ Nav | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Confirm | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Cancel"),
        ],
        AppMode::KeybindingsInfo | AppMode::KeybindingDetail => vec![
            Span::styled(
                "Esc/q/Ctrl+H",
                Style::default().fg(theme::current().bright_red),
            ),
            Span::raw(": Close Help | "),
            Span::raw("↑↓/PgUp/PgDn: Scroll/Select | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Details"),
        ],
        AppMode::FuzzyFinding => vec![
            Span::raw("Type to Search | "),
            Span::raw("↑↓ Nav | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(": Select | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(": Cancel"),
        ],
        AppMode::ImportTransactions | AppMode::ExportTransactions => vec![
            Span::raw("Type path | "),
            Span::raw("←→ Cursor | "),
            Span::styled(
                "Ctrl+U",
                Style::default().fg(theme::current().bright_magenta),
            ),
            Span::raw(" Clear | "),
            Span::styled(
                "Ctrl+D",
                Style::default().fg(theme::current().bright_magenta),
            ),
            Span::raw(" Default | "),
            Span::styled("Enter", Style::default().fg(theme::current().bright_green)),
            Span::raw(" Confirm | "),
            Span::styled("Esc", Style::default().fg(theme::current().bright_red)),
            Span::raw(" Back"),
        ],
    };

    let help_paragraph = Paragraph::new(Line::from(help_spans))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Line::from(vec![
                    Span::styled("Help - ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(
                        "Ctrl+H",
                        Style::default()
                            .fg(theme::current().red)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " For More Info",
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ])),
        );
    f.render_widget(help_paragraph, area);
}
