use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::ui::state::Mode;
use crate::{error::AppError, ui::state::AppState};

use super::EventResult;

pub fn handle_command_event(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match (key_event.code, key_event.modifiers) {
                (KeyCode::Enter, _) => {
                    state.mode = Mode::Normal;
                    match state.command_state.parse_command() {
                        Ok(cmd) => state.execute_command(&cmd),
                        Err(_) => Ok(EventResult::Redraw),
                    }
                }
                (KeyCode::Char('+'), _) => {
                    if state.command_state.cursor != 0 {
                        return Ok(EventResult::Continue);
                    }
                    if state.command_state.command_buffer.is_empty()
                        || (!state.command_state.command_buffer.starts_with('+')
                            && !state.command_state.command_buffer.starts_with('-'))
                    {
                        state
                            .command_state
                            .command_buffer
                            .insert(state.command_state.cursor, '+');
                        state.command_state.cursor += 1;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Char('q'), _) => {
                    state.mode = Mode::Normal;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Esc, _) => {
                    state.mode = Mode::Normal;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                    state.command_state.command_buffer.clear();
                    state.command_state.cursor = 0;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                    state.command_state.cursor = 0;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                    state.command_state.cursor = state.command_state.command_buffer.len();
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                    state.command_state.command_buffer.clear();
                    state.command_state.cursor = 0;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char('-'), _) => {
                    if state.command_state.cursor != 0 {
                        return Ok(EventResult::Continue);
                    }
                    if state.command_state.command_buffer.is_empty()
                        || (!state.command_state.command_buffer.starts_with('+')
                            && !state.command_state.command_buffer.starts_with('-'))
                    {
                        state
                            .command_state
                            .command_buffer
                            .insert(state.command_state.cursor, '-');
                        state.command_state.cursor += 1;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Backspace, _) => {
                    if state.command_state.cursor > 0 {
                        state.command_state.cursor -= 1;
                        state
                            .command_state
                            .command_buffer
                            .remove(state.command_state.cursor);
                    }
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Delete, _) => {
                    if state.command_state.cursor < state.command_state.command_buffer.len() {
                        state
                            .command_state
                            .command_buffer
                            .remove(state.command_state.cursor);
                    }
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Left, _) => {
                    if state.command_state.cursor > 0 {
                        state.command_state.cursor -= 1;
                    }
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Right, _) => {
                    if state.command_state.cursor < state.command_state.command_buffer.len() {
                        state.command_state.cursor += 1;
                    }
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char(c), _) if c.is_ascii_digit() => {
                    state
                        .command_state
                        .command_buffer
                        .insert(state.command_state.cursor, c);
                    state.command_state.cursor += 1;
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
