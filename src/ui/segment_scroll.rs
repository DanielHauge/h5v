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
    let scrollbar = Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("⬆"))
        .thumb_symbol("█")
        .end_symbol(Some("⬇"));
    let mut scrollbar_state = ScrollbarState::new(state.segment_state.segment_count as usize)
        .viewport_content_length(2)
        .position(state.segment_state.idx as usize);
    f.render_stateful_widget(scrollbar, *area, &mut scrollbar_state);
    Ok(())
}
