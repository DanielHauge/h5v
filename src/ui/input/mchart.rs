use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind};

use crate::{
    error::AppError,
    ui::state::{AppState, Mode},
};

use super::EventResult;

pub(crate) fn handle_mchart_event(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match (key_event.code, key_event.modifiers) {
                (KeyCode::Esc, _) => {
                    state.mode = Mode::Normal;
                    Ok(EventResult::Redraw)
                }

                (KeyCode::Char('q'), _) => Ok(EventResult::Quit),

                (KeyCode::Delete, _) => {
                    state.multi_chart.clear_selected();
                    state.compute_tree_view();
                    Ok(EventResult::Redraw)
                }

                (KeyCode::Backspace, _) => {
                    state.multi_chart.clear_selected();
                    state.compute_tree_view();
                    Ok(EventResult::Redraw)
                }

                (KeyCode::Char('d'), _) => {
                    state.multi_chart.clear_selected();
                    state.compute_tree_view();
                    Ok(EventResult::Redraw)
                }

                (KeyCode::Down, _) => {
                    state.multi_chart.idx = state
                        .multi_chart
                        .idx
                        .saturating_add(1)
                        .clamp(0, state.multi_chart.line_series.len().saturating_sub(1));
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Up, _) => {
                    state.multi_chart.idx = state.multi_chart.idx.saturating_sub(1);
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char('M'), _) => {
                    state.mode = Mode::Normal;
                    Ok(EventResult::Redraw)
                }
                _ => Ok(EventResult::Continue),
            },
            KeyEventKind::Repeat => Ok(EventResult::Continue),
            KeyEventKind::Release => Ok(EventResult::Continue),
        },
        Event::Resize(_, _) => Ok(EventResult::Redraw),
        _ => Ok(EventResult::Continue),
    }
}
