use ratatui::crossterm::event::Event;

use crate::{
    error::AppError,
    ui::state::{AppState, ContentShowMode},
};

use super::{
    keymap::{content_action, ContentAction, Direction},
    EventResult,
};

pub fn handle_normal_content_event(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            ratatui::crossterm::event::KeyEventKind::Press => {
                match (content_action(&key_event), state.content_mode) {
                    (
                        Some(ContentAction::Move(Direction::Left, amount)),
                        ContentShowMode::Matrix,
                    ) => {
                        // Get the current tree item and its attributes
                        let max = state
                            .matrix_view_state
                            .cols_currently_available
                            .saturating_sub(1);

                        let move_within_view = state.matrix_view_state.cursor_col.min(amount);
                        let remaining = amount.saturating_sub(move_within_view);
                        if remaining > 0 && state.matrix_view_state.col_offset > 0 {
                            state.left(remaining as isize)?;
                        }
                        let new_cursor = state
                            .matrix_view_state
                            .cursor_col
                            .saturating_sub(move_within_view)
                            .clamp(0, max);
                        state.matrix_view_state.cursor_col = new_cursor;

                        Ok(EventResult::Redraw)
                    }
                    (
                        Some(ContentAction::Move(Direction::Right, amount)),
                        ContentShowMode::Matrix,
                    ) => {
                        let max = state
                            .matrix_view_state
                            .cols_currently_available
                            .saturating_sub(1);

                        let move_within_view = max
                            .saturating_sub(state.matrix_view_state.cursor_col)
                            .min(amount);
                        let remaining = amount.saturating_sub(move_within_view);
                        if remaining > 0 {
                            state.right(remaining as isize)?;
                        }
                        let new_cursor = state
                            .matrix_view_state
                            .cursor_col
                            .saturating_add(move_within_view)
                            .clamp(0, max);
                        state.matrix_view_state.cursor_col = new_cursor;
                        Ok(EventResult::Redraw)
                    }
                    (Some(ContentAction::Move(Direction::Up, amount)), ContentShowMode::Matrix) => {
                        // Get the current tree item and its attributes
                        let max = state
                            .matrix_view_state
                            .rows_currently_available
                            .saturating_sub(1);

                        let move_within_view = state.matrix_view_state.cursor_row.min(amount);
                        let remaining = amount.saturating_sub(move_within_view);
                        if remaining > 0 && state.matrix_view_state.row_offset > 0 {
                            state.up(remaining)?;
                        }
                        let new_cursor = state
                            .matrix_view_state
                            .cursor_row
                            .saturating_sub(move_within_view)
                            .clamp(0, max);
                        state.matrix_view_state.cursor_row = new_cursor;
                        Ok(EventResult::Redraw)
                    }
                    (
                        Some(ContentAction::Move(Direction::Down, amount)),
                        ContentShowMode::Matrix,
                    ) => {
                        let max = state
                            .matrix_view_state
                            .rows_currently_available
                            .saturating_sub(1);
                        let move_within_view = max
                            .saturating_sub(state.matrix_view_state.cursor_row)
                            .min(amount);
                        let remaining = amount.saturating_sub(move_within_view);
                        if remaining > 0 {
                            state.down(remaining)?;
                        }
                        let new_cursor = state
                            .matrix_view_state
                            .cursor_row
                            .saturating_add(move_within_view)
                            .clamp(0, max);
                        state.matrix_view_state.cursor_row = new_cursor;

                        Ok(EventResult::Redraw)
                    }
                    (
                        Some(ContentAction::Move(Direction::Down, amount)),
                        ContentShowMode::Preview,
                    ) => state.down(amount),
                    (
                        Some(ContentAction::Move(Direction::Up, amount)),
                        ContentShowMode::Preview,
                    ) => state.up(amount),
                    (
                        Some(ContentAction::Move(Direction::Right, amount)),
                        ContentShowMode::Preview,
                    ) => state.right(amount as isize),
                    (
                        Some(ContentAction::Move(Direction::Left, amount)),
                        ContentShowMode::Preview,
                    ) => state.left(amount as isize),
                    (Some(ContentAction::Copy), ContentShowMode::Matrix) => {
                        Ok(EventResult::Copying)
                    }
                    _ => Ok(EventResult::Continue),
                }
            }
            ratatui::crossterm::event::KeyEventKind::Repeat => Ok(EventResult::Continue),
            ratatui::crossterm::event::KeyEventKind::Release => Ok(EventResult::Continue),
        },
        Event::Resize(_, _) => Ok(EventResult::Redraw),
        _ => Ok(EventResult::Continue),
    }
}
