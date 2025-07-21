use ratatui::{
    layout::Rect,
    widgets::{Paragraph, Scrollbar, ScrollbarState},
    Frame,
};

use crate::error::AppError;

use super::state::AppState;

pub fn render_segment_scroll(
    f: &mut Frame,
    area: &Rect,
    state: &mut AppState,
) -> Result<(), AppError> {
    let scrollbar = Scrollbar::new(ratatui::widgets::ScrollbarOrientation::HorizontalBottom)
        .end_symbol(Some("→"))
        .thumb_symbol("█")
        .begin_symbol(Some("←"));
    let mut scrollbar_state = ScrollbarState::new(state.segment_state.segment_count as usize)
        .viewport_content_length(2)
        .position(state.segment_state.idx as usize);
    f.render_stateful_widget(scrollbar, *area, &mut scrollbar_state);
    let p = format!(
        "{}/{}",
        state.segment_state.idx + 1,
        state.segment_state.segment_count
    );
    let centered_p = Paragraph::new(p)
        .alignment(ratatui::layout::Alignment::Center)
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(centered_p, *area);
    Ok(())
}
