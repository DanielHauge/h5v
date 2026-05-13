use ratatui::{
    layout::Rect,
    widgets::{Scrollbar, ScrollbarState},
    Frame,
};

use crate::error::AppError;

pub struct SegmentDisplayInfo<'a> {
    pub title: &'a str,
    pub current: usize,
    pub total: usize,
    pub range_start: usize,
    pub range_end: usize,
    pub total_items: usize,
    pub unit: &'a str,
}

pub fn render_position_scroll(
    f: &mut Frame,
    area: &Rect,
    total: usize,
    position: usize,
    viewport_len: usize,
) -> Result<(), AppError> {
    let scrollbar = Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("⬆"))
        .thumb_symbol("█")
        .end_symbol(Some("⬇"));
    let mut scrollbar_state = ScrollbarState::new(total.max(1))
        .viewport_content_length(viewport_len.max(1))
        .position(position.min(total.saturating_sub(1)));
    f.render_stateful_widget(scrollbar, *area, &mut scrollbar_state);
    Ok(())
}

pub(crate) fn compact_count(value: usize) -> String {
    match value {
        0..=999 => value.to_string(),
        1_000..=999_999 => format!("{:.1}k", value as f64 / 1_000.0),
        1_000_000..=999_999_999 => format!("{:.1}M", value as f64 / 1_000_000.0),
        _ => format!("{:.1}B", value as f64 / 1_000_000_000.0),
    }
}
