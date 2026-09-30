use crate::app::state::App;
use crate::ui::form::render_field_form;
use ratatui::prelude::*;
use ratatui::widgets::Clear;

/// Three fields of three rows each, plus the form's border and margin.
const FORM_HEIGHT: u16 = 13;

pub fn render_bank_sync_setup(f: &mut Frame, app: &App, area: Rect) {
    let width = area.width.saturating_sub(4).min(90);
    let height = FORM_HEIGHT.min(area.height);
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    f.render_widget(Clear, popup);
    render_field_form(
        f,
        &app.bank_sync_setup_fields,
        app.bank_sync_setup_cursor,
        popup,
        " Set Up Bank Sync (Pluggy) ",
        Some(" [Enter] Save and sync, [Esc] Skip "),
        |_| None,
    );
}
