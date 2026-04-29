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
                    (Some(ContentAction::Move(Direction::Left)), ContentShowMode::Matrix) => {
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
                    (Some(ContentAction::Move(Direction::Right)), ContentShowMode::Matrix) => {
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
                    (Some(ContentAction::Move(Direction::Up)), ContentShowMode::Matrix) => {
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
                    (Some(ContentAction::Move(Direction::Down)), ContentShowMode::Matrix) => {
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
                    (Some(ContentAction::Move(Direction::Down)), ContentShowMode::Preview) => {
                        state.down(1)
                    }
                    (Some(ContentAction::Move(Direction::Up)), ContentShowMode::Preview) => {
                        state.up(1)
                    }
                    (Some(ContentAction::Move(Direction::Right)), ContentShowMode::Preview) => {
                        state.right(1)
                    }
                    (Some(ContentAction::Move(Direction::Left)), ContentShowMode::Preview) => {
                        state.left(1)
                    }
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
