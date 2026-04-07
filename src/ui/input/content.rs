use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind};

use crate::{error::AppError, ui::state::AppState};

use super::EventResult;

pub fn handle_normal_content_event(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match (key_event.code, key_event.modifiers) {
                (KeyCode::Left, _) => {
                    // Get the current tree item and its attributes
                    let max = state.matrix_view_state.cols_currently_available;
                    let new_cursor = state
                        .matrix_view_state
                        .cursor_col
                        .saturating_sub(1)
                        .clamp(0, max.saturating_sub(1));
                    state.matrix_view_state.cursor_col = new_cursor;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Right, _) => {
                    let max = state.matrix_view_state.cols_currently_available;
                    let new_cursor = state
                        .matrix_view_state
                        .cursor_col
                        .saturating_add(1)
                        .clamp(0, max.saturating_sub(1));
                    state.matrix_view_state.cursor_col = new_cursor;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Up, _) => {
                    // Get the current tree item and its attributes
                    let max = state.matrix_view_state.rows_currently_available;
                    let new_cursor = state
                        .matrix_view_state
                        .cursor_row
                        .saturating_sub(1)
                        .clamp(0, max.saturating_sub(1));
                    state.matrix_view_state.cursor_row = new_cursor;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Down, _) => {
                    let max = state.matrix_view_state.rows_currently_available;
                    let new_cursor = state
                        .matrix_view_state
                        .cursor_row
                        .saturating_add(1)
                        .clamp(0, max.saturating_sub(1));
                    state.matrix_view_state.cursor_row = new_cursor;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char('y'), _) => Ok(EventResult::Copying),
                _ => Ok(EventResult::Continue),
            },
            KeyEventKind::Repeat => Ok(EventResult::Continue),
            KeyEventKind::Release => Ok(EventResult::Continue),
        },
        Event::Resize(_, _) => Ok(EventResult::Redraw),
        _ => Ok(EventResult::Continue),
    }
}
