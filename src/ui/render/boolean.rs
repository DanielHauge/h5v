use ratatui::style::Style;

use crate::configure;

pub(crate) fn boolean_text(value: u64) -> Option<&'static str> {
    match value {
        0 => Some("false"),
        1 => Some("true"),
        _ => None,
    }
}

pub(crate) fn boolean_style() -> Style {
    let style = Style::default().fg(configure::themed_color(|colors| colors.text.bool_value));
    if configure::prefers_strong_text() {
        style.bold()
    } else {
        style
    }
}
