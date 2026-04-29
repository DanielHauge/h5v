use ratatui::crossterm::event::{Event, KeyEventKind};

use crate::ui::state::Mode;
use crate::{error::AppError, ui::state::AppState};

use super::{
    keymap::{command_action, CommandAction},
    EventResult,
};

pub fn handle_command_event(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match command_action(&key_event) {
                Some(CommandAction::Submit) => {
                    state.mode = Mode::Normal;
                    match state.command_state.parse_command() {
                        Ok(cmd) => state.execute_command(&cmd),
                        Err(_) => Ok(EventResult::Redraw),
                    }
                }
                Some(CommandAction::PrefixPlus) => {
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
                Some(CommandAction::Cancel) => {
                    state.mode = Mode::Normal;
                    Ok(EventResult::Redraw)
                }
                Some(CommandAction::ClearWord) | Some(CommandAction::Clear) => {
                    state.command_state.command_buffer.clear();
                    state.command_state.cursor = 0;
                    Ok(EventResult::Redraw)
                }
                Some(CommandAction::MoveToStart) => {
                    state.command_state.cursor = 0;
                    Ok(EventResult::Redraw)
                }
                Some(CommandAction::MoveToEnd) => {
                    state.command_state.cursor = state.command_state.command_buffer.len();
                    Ok(EventResult::Redraw)
                }
                Some(CommandAction::PrefixMinus) => {
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
                Some(CommandAction::Backspace) => {
                    if state.command_state.cursor > 0 {
                        state.command_state.cursor -= 1;
                        state
                            .command_state
                            .command_buffer
                            .remove(state.command_state.cursor);
                    }
                    Ok(EventResult::Redraw)
                }
                Some(CommandAction::Delete) => {
                    if state.command_state.cursor < state.command_state.command_buffer.len() {
                        state
                            .command_state
                            .command_buffer
                            .remove(state.command_state.cursor);
                    }
                    Ok(EventResult::Redraw)
                }
                Some(CommandAction::MoveLeft) => {
                    if state.command_state.cursor > 0 {
                        state.command_state.cursor -= 1;
                    }
                    Ok(EventResult::Redraw)
                }
                Some(CommandAction::MoveRight) => {
                    if state.command_state.cursor < state.command_state.command_buffer.len() {
                        state.command_state.cursor += 1;
                    }
                    Ok(EventResult::Redraw)
                }
                Some(CommandAction::InsertDigit(c)) => {
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
