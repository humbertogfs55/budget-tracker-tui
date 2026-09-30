use crate::theme;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render_status_bar(f: &mut Frame, message: &str, area: Rect) {
    let style = if message.starts_with("Error:") {
        Style::default().fg(theme::current().bright_red)
    } else {
        Style::default().fg(theme::current().bright_yellow)
    };
    let status_text = Paragraph::new(message)
        .style(style)
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .alignment(Alignment::Center);
    f.render_widget(status_text, area);
}
