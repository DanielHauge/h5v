use ratatui::{
    style::Style,
    widgets::{Block, BorderType, Borders},
};

use crate::configure;

pub(crate) fn rounded_panel(title: impl Into<String>) -> Block<'static> {
    rounded_panel_with_style(title, Style::default())
}

pub(crate) fn rounded_panel_with_style(title: impl Into<String>, style: Style) -> Block<'static> {
    Block::default()
        .title(title.into())
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.surface.panel_title))
                .bold(),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(configure::themed_color(|colors| {
            colors.surface.panel_border
        })))
        .style(style)
}

pub(crate) fn truncate_to_width(message: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let char_count = message.chars().count();
    if char_count <= max_width {
        return message.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }
    let mut truncated = message.chars().take(max_width - 3).collect::<String>();
    truncated.push_str("...");
    truncated
}
