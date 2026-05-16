use ratatui::crossterm::event::{Event, KeyEventKind};

use crate::ui::state::AppToast;
use crate::{error::AppError, ui::state::AppState};

use super::{
    super::command::execute_command,
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
                    state.mode = state.command_return_mode.clone();
                    match state.command_state.parse_command() {
                        Ok(cmd) => match execute_command(state, &cmd) {
                            Ok(result) => {
                                state.command_state.record_successful_command(&cmd);
                                Ok(result)
                            }
                            Err(error) => Ok(EventResult::Toast(
                                AppToast::Error(error.to_string()),
                                false,
                            )),
                        },
                        Err(error) => Ok(EventResult::Toast(
                            AppToast::Error(error.to_string()),
                            false,
                        )),
                    }
                }
                Some(CommandAction::CompleteSuggestion) => {
                    if state.command_state.apply_selected_suggestion() {
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                Some(CommandAction::SelectPrevSuggestion) => {
                    state.command_state.select_previous_suggestion();
                    Ok(EventResult::Redraw)
                }
                Some(CommandAction::SelectNextSuggestion) => {
                    state.command_state.select_next_suggestion();
                    Ok(EventResult::Redraw)
                }
                Some(CommandAction::SelectPrevHistory) => {
                    if state.command_state.select_previous_history() {
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                Some(CommandAction::SelectNextHistory) => {
                    if state.command_state.select_next_history() {
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                Some(CommandAction::Cancel) => {
                    state.mode = state.command_return_mode.clone();
                    Ok(EventResult::Redraw)
                }
                Some(CommandAction::ClearWord) | Some(CommandAction::Clear) => {
                    state.command_state.begin_new_entry();
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
                Some(CommandAction::Backspace) => {
                    if state.command_state.cursor > 0 {
                        state.command_state.cursor -= 1;
                        state
                            .command_state
                            .command_buffer
                            .remove(state.command_state.cursor);
                        state.command_state.note_buffer_edited();
                    }
                    Ok(EventResult::Redraw)
                }
                Some(CommandAction::Delete) => {
                    if state.command_state.cursor < state.command_state.command_buffer.len() {
                        state
                            .command_state
                            .command_buffer
                            .remove(state.command_state.cursor);
                        state.command_state.note_buffer_edited();
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
                Some(CommandAction::InsertChar(c)) => {
                    state
                        .command_state
                        .command_buffer
                        .insert(state.command_state.cursor, c);
                    state.command_state.cursor += 1;
                    state.command_state.note_buffer_edited();
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
