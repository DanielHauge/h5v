use ratatui::{
    layout::Rect,
    widgets::{Scrollbar, ScrollbarState},
    Frame,
};

use crate::error::AppError;

use super::state::AppState;

pub fn render_segment_scroll(
    f: &mut Frame,
    area: &Rect,
    state: &mut AppState,
) -> Result<(), AppError> {
    render_position_scroll(
        f,
        area,
        state.segment_state.segment_count as usize,
        state.segment_state.idx as usize,
        1,
    )
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
