use crate::app::state::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle_bank_sync_setup_mode(app: &mut App, key_event: KeyEvent) {
    match (key_event.code, key_event.modifiers) {
        (KeyCode::Esc, KeyModifiers::NONE) => app.cancel_bank_sync_setup(),
        (KeyCode::Enter, KeyModifiers::NONE) => app.save_bank_sync_setup(),
        (KeyCode::Tab, KeyModifiers::NONE) | (KeyCode::Down, KeyModifiers::NONE) => {
            app.next_bank_sync_setup_field()
        }
        (KeyCode::BackTab, KeyModifiers::NONE) | (KeyCode::Up, KeyModifiers::NONE) => {
            app.previous_bank_sync_setup_field()
        }
        (KeyCode::Left, KeyModifiers::NONE) => app.move_cursor_left(),
        (KeyCode::Right, KeyModifiers::NONE) => app.move_cursor_right(),
        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
            app.insert_char_at_cursor(c)
        }
        (KeyCode::Backspace, KeyModifiers::NONE) => app.delete_char_before_cursor(),
        (KeyCode::Delete, KeyModifiers::NONE) => app.delete_char_after_cursor(),
        _ => {}
    }
}
