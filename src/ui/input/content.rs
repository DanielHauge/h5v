use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind};

use crate::{
    error::AppError,
    ui::state::{AppState, ContentShowMode},
};

use super::EventResult;

pub fn handle_normal_content_event(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => {
                match (key_event.code, key_event.modifiers, state.content_mode) {
                    (KeyCode::Left, _, ContentShowMode::Matrix) => {
                        // Get the current tree item and its attributes
                        let max = state
                            .matrix_view_state
                            .cols_currently_available
                            .saturating_sub(1);

                        if state.matrix_view_state.cursor_col == 0
                            && state.matrix_view_state.col_offset > 0
                        {
                            state.left(1)?;
                        }
                        let new_cursor = state
                            .matrix_view_state
                            .cursor_col
                            .saturating_sub(1)
                            .clamp(0, max);
                        state.matrix_view_state.cursor_col = new_cursor;

                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Right, _, ContentShowMode::Matrix) => {
                        let max = state
                            .matrix_view_state
                            .cols_currently_available
                            .saturating_sub(1);

                        if state.matrix_view_state.cursor_col == max {
                            state.right(1)?;
                        }
                        let new_cursor = state
                            .matrix_view_state
                            .cursor_col
                            .saturating_add(1)
                            .clamp(0, max);
                        state.matrix_view_state.cursor_col = new_cursor;
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Up, _, ContentShowMode::Matrix) => {
                        // Get the current tree item and its attributes
                        let max = state
                            .matrix_view_state
                            .rows_currently_available
                            .saturating_sub(1);

                        if state.matrix_view_state.cursor_row == 0
                            && state.matrix_view_state.row_offset > 0
                        {
                            state.up(1)?;
                        }
                        let new_cursor = state
                            .matrix_view_state
                            .cursor_row
                            .saturating_sub(1)
                            .clamp(0, max);
                        state.matrix_view_state.cursor_row = new_cursor;
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Down, _, ContentShowMode::Matrix) => {
                        let max = state
                            .matrix_view_state
                            .rows_currently_available
                            .saturating_sub(1);
                        if state.matrix_view_state.cursor_row == max {
                            state.down(1)?;
                        }
                        let new_cursor = state
                            .matrix_view_state
                            .cursor_row
                            .saturating_add(1)
                            .clamp(0, max);
                        state.matrix_view_state.cursor_row = new_cursor;

                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Down, _, ContentShowMode::Preview) => state.down(1),
                    (KeyCode::Up, _, ContentShowMode::Preview) => state.up(1),
                    (KeyCode::Right, _, ContentShowMode::Preview) => state.right(1),
                    (KeyCode::Left, _, ContentShowMode::Preview) => state.left(1),
                    (KeyCode::Char('y'), _, ContentShowMode::Matrix) => Ok(EventResult::Copying),
                    _ => Ok(EventResult::Continue),
                }
            }
            KeyEventKind::Repeat => Ok(EventResult::Continue),
            KeyEventKind::Release => Ok(EventResult::Continue),
        },
        Event::Resize(_, _) => Ok(EventResult::Redraw),
        _ => Ok(EventResult::Continue),
    }
}
