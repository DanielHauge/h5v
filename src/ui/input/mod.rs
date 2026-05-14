use std::cell::RefCell;

use attributes::handle_normal_attributes;
use content::handle_normal_content_event;
use dialogs::{
    handle_attribute_create_dialog, handle_attribute_delete_dialog,
    handle_fixed_string_overflow_dialog, handle_fixed_string_resize_dialog,
};
use keymap::{
    global_action, normal_action, window_action, BoundAction, Direction, GlobalAction,
    NormalAction, WindowAction,
};
use mouse::handle_normal_mouse_event;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use tree::handle_normal_tree_event;

use crate::{
    configure,
    error::AppError,
    h5f::Node,
    search::{full_traversal, Searcher},
    ui::command::{execute_command, parse_command_text, parse_startup_commands},
    ui::state::AppToast,
};

use super::state::{AppState, Focus, Mode, PendingChord};

pub mod attributes;
pub mod command;
pub mod content;
mod dialogs;
pub mod keymap;
pub mod mchart;
mod mouse;
pub mod search;
pub mod tree;

pub enum EventResult {
    Quit,
    Redraw,
    Copying,
    ReloadFile { write: bool },
    Configure { reset: bool },
    Continue,
    Error(String),
    Toast(AppToast, bool),
}

fn is_handled_key_press(key_event: &KeyEvent) -> bool {
    matches!(key_event.kind, KeyEventKind::Press)
}

pub(crate) fn execute_bound_command(
    state: &mut AppState<'_>,
    command_text: &str,
) -> Result<EventResult, AppError> {
    if state.binding_command_depth >= 8 {
        return Err(AppError::InvalidCommand(
            "Keybinding command recursion limit reached".to_string(),
        ));
    }
    state.binding_command_depth += 1;
    let result = (|| {
        let command = parse_command_text(command_text)?;
        execute_command(state, &command)
    })();
    state.binding_command_depth = state.binding_command_depth.saturating_sub(1);
    result
}

pub(crate) fn execute_bound_script(
    state: &mut AppState<'_>,
    script_text: &str,
    origin: &str,
) -> Result<EventResult, AppError> {
    if state.binding_command_depth >= 8 {
        return Err(AppError::InvalidCommand(
            "Keybinding command recursion limit reached".to_string(),
        ));
    }
    state.binding_command_depth += 1;
    let result = (|| {
        let mut last_result = EventResult::Continue;
        for command in parse_startup_commands(script_text, origin) {
            let invocation = parse_command_text(&command.command_text).map_err(|error| {
                AppError::InvalidCommand(format!("{}: {}", command.origin, error))
            })?;
            let event_result = execute_command(state, &invocation)?;
            match event_result {
                EventResult::Continue | EventResult::Redraw | EventResult::Copying => {
                    last_result = event_result;
                }
                EventResult::Toast(_, _) => {
                    last_result = event_result;
                }
                EventResult::Quit
                | EventResult::ReloadFile { .. }
                | EventResult::Configure { .. }
                | EventResult::Error(_) => return Ok(event_result),
            }
        }
        Ok(last_result)
    })();
    state.binding_command_depth = state.binding_command_depth.saturating_sub(1);
    result
}

pub(crate) fn execute_bound_lua_callback(
    state: &mut AppState<'_>,
    callback_id: &str,
) -> Result<EventResult, AppError> {
    if state.binding_command_depth >= 8 {
        return Err(AppError::InvalidCommand(
            "Keybinding command recursion limit reached".to_string(),
        ));
    }
    state.binding_command_depth += 1;
    let result = configure::with_keymap_lua_callback(callback_id, |lua, callback| {
        let callback_result = RefCell::new(EventResult::Continue);
        let state_cell = RefCell::new(&mut *state);
        lua.scope(|scope| {
            let command_fn = scope.create_function_mut(|_, command: String| {
                let mut state = state_cell.borrow_mut();
                *callback_result.borrow_mut() = execute_bound_command(*state, &command)
                    .map_err(|error| mlua::Error::runtime(error.to_string()))?;
                Ok(())
            })?;
            let commands_fn = scope.create_function_mut(|_, commands: mlua::Table| {
                let mut parts = Vec::new();
                for value in commands.sequence_values::<String>() {
                    parts.push(value?);
                }
                let script = parts.join("\n");
                let mut state = state_cell.borrow_mut();
                *callback_result.borrow_mut() =
                    execute_bound_script(*state, &script, "lua keybinding commands")
                        .map_err(|error| mlua::Error::runtime(error.to_string()))?;
                Ok(())
            })?;
            let script_fn = scope.create_function_mut(|_, script: String| {
                let mut state = state_cell.borrow_mut();
                *callback_result.borrow_mut() =
                    execute_bound_script(*state, &script, "lua keybinding script")
                        .map_err(|error| mlua::Error::runtime(error.to_string()))?;
                Ok(())
            })?;
            let ctx = lua.create_table()?;
            ctx.set("command", command_fn)?;
            ctx.set("commands", commands_fn)?;
            ctx.set("script", script_fn)?;
            callback.call::<()>(ctx)?;
            Ok(())
        })
        .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        Ok(callback_result.into_inner())
    });
    state.binding_command_depth = state.binding_command_depth.saturating_sub(1);
    result
}

fn handle_global_action(
    state: &mut AppState<'_>,
    action: GlobalAction,
) -> Result<EventResult, AppError> {
    match action {
        GlobalAction::EnterCommand => {
            state.command_return_mode = state.mode.clone();
            state.mode = Mode::Command;
            state.command_state.begin_new_entry();
            Ok(EventResult::Redraw)
        }
        GlobalAction::ShowHelp => {
            state.help_return_mode = state.mode.clone();
            state.mode = Mode::Help;
            Ok(EventResult::Redraw)
        }
        GlobalAction::Quit => Ok(EventResult::Quit),
        GlobalAction::ReloadFile => Ok(EventResult::ReloadFile {
            write: !state.readonly,
        }),
        GlobalAction::ToggleMultiChart => {
            state.mode = if matches!(state.mode, Mode::MultiChart) {
                Mode::Normal
            } else {
                Mode::MultiChart
            };
            Ok(EventResult::Redraw)
        }
    }
}

pub fn handle_input_event(state: &mut AppState<'_>, event: Event) -> Result<EventResult, AppError> {
    if let Event::Resize(_, __) = event {
        return Ok(EventResult::Redraw);
    }

    match state.mode {
        Mode::Command => command::handle_command_event(state, event),
        Mode::MultiChart => mchart::handle_mchart_event(state, event),
        Mode::AttributeCreateDialog => handle_attribute_create_dialog(state, event),
        Mode::AttributeDeleteDialog => handle_attribute_delete_dialog(state, event),
        Mode::FixedStringOverflowDialog => handle_fixed_string_overflow_dialog(state, event),
        Mode::FixedStringResizeDialog => handle_fixed_string_resize_dialog(state, event),
        Mode::Normal => match event {
            Event::Key(key_event) => {
                if !is_handled_key_press(&key_event) {
                    return Ok(EventResult::Continue);
                }

                let keymaps = configure::current_keymaps();
                if state.pending_chord == Some(PendingChord::CtrlW) {
                    state.pending_chord = None;
                    if let Some(action) = window_action(&key_event, &keymaps) {
                        return match action {
                            BoundAction::Action(WindowAction::Focus(direction)) => {
                                apply_focus_action(state, direction);
                                Ok(EventResult::Redraw)
                            }
                            BoundAction::Action(WindowAction::ToggleTreeView) => {
                                state.toggle_tree_view();
                                Ok(EventResult::Redraw)
                            }
                            BoundAction::Command(command) => execute_bound_command(state, &command),
                            BoundAction::Script(script) => {
                                execute_bound_script(state, &script, "keybinding script")
                            }
                            BoundAction::LuaCallback(callback_id) => {
                                execute_bound_lua_callback(state, &callback_id)
                            }
                        };
                    }
                    return Ok(EventResult::Continue);
                }

                let focused_result = match state.focus {
                    Focus::Tree(_) => {
                        handle_normal_tree_event(state, Event::Key(key_event), &keymaps)?
                    }
                    Focus::Attributes => {
                        handle_normal_attributes(state, Event::Key(key_event), &keymaps)?
                    }
                    Focus::Content => {
                        handle_normal_content_event(state, Event::Key(key_event), &keymaps)?
                    }
                };
                if !matches!(focused_result, EventResult::Continue) {
                    return Ok(focused_result);
                }

                if let Some(action) = normal_action(&key_event, &keymaps) {
                    return match action {
                        BoundAction::Action(NormalAction::EnterCommand) => {
                            state.command_return_mode = Mode::Normal;
                            state.mode = Mode::Command;
                            state.command_state.begin_new_entry();
                            Ok(EventResult::Redraw)
                        }
                        BoundAction::Action(NormalAction::RepeatCommand) => {
                            state.reexecute_command()
                        }
                        BoundAction::Action(NormalAction::EnterSearch) => {
                            if state.searcher.is_none() {
                                let Node::File(ref file) = state.root.borrow().node else {
                                    return Ok(EventResult::Error(
                                        "Search only available for HDF5 files".to_string(),
                                    ));
                                };
                                let Ok(file_as_group) = file.as_group() else {
                                    return Ok(EventResult::Error(
                                        "Search only available for HDF5 files with roots that can polymorp as group.".to_string(),
                                    ));
                                };
                                let all_h5_paths = full_traversal(&file_as_group);
                                state.searcher = Some(Searcher::new(all_h5_paths));
                            }
                            let Some(ref mut searcher) = state.searcher else {
                                return Ok(EventResult::Error("Search not available".to_string()));
                            };
                            searcher.query.clear();
                            searcher.line_cursor = 0;
                            state.mode = Mode::Search;
                            Ok(EventResult::Redraw)
                        }
                        BoundAction::Action(NormalAction::Quit) => Ok(EventResult::Quit),
                        BoundAction::Action(NormalAction::ToggleContentMode) => {
                            let available = state.treeview[state.tree_view_cursor]
                                .node
                                .borrow_mut()
                                .content_show_modes();
                            state.swap_content_show_mode(
                                state.filter_runtime_content_modes(available),
                            );
                            Ok(EventResult::Redraw)
                        }
                        BoundAction::Action(NormalAction::ShowHelp) => {
                            state.help_return_mode = Mode::Normal;
                            state.mode = Mode::Help;
                            Ok(EventResult::Redraw)
                        }
                        BoundAction::Action(NormalAction::ToggleMultiChart) => {
                            state.mode = Mode::MultiChart;
                            Ok(EventResult::Redraw)
                        }
                        BoundAction::Action(NormalAction::ToggleTreeView) => {
                            state.toggle_tree_view();
                            Ok(EventResult::Redraw)
                        }
                        BoundAction::Action(NormalAction::ReloadFile) => {
                            Ok(EventResult::ReloadFile {
                                write: !state.readonly,
                            })
                        }
                        BoundAction::Action(NormalAction::Focus(direction)) => {
                            apply_focus_action(state, direction);
                            Ok(EventResult::Redraw)
                        }
                        BoundAction::Action(NormalAction::StartWindowChord) => {
                            state.pending_chord = Some(PendingChord::CtrlW);
                            Ok(EventResult::Continue)
                        }
                        BoundAction::Action(NormalAction::ChangeX(delta)) => state.change_x(delta),
                        BoundAction::Action(NormalAction::ChangeRow(delta)) => {
                            state.change_row(delta)
                        }
                        BoundAction::Action(NormalAction::ChangeCol(delta)) => {
                            state.change_col(delta)
                        }
                        BoundAction::Action(NormalAction::ChangeSelectedIndex(delta)) => {
                            state.change_selected_index(delta)
                        }
                        BoundAction::Action(NormalAction::ChangeSelectedDimension(delta)) => {
                            state.change_selected_dimension(delta)
                        }
                        BoundAction::Action(NormalAction::Scroll(direction, amount)) => {
                            match direction {
                                Direction::Left => state.left(amount as isize),
                                Direction::Right => state.right(amount as isize),
                                Direction::Up => state.up(amount),
                                Direction::Down => state.down(amount),
                            }
                        }
                        BoundAction::Command(command) => execute_bound_command(state, &command),
                        BoundAction::Script(script) => {
                            execute_bound_script(state, &script, "keybinding script")
                        }
                        BoundAction::LuaCallback(callback_id) => {
                            execute_bound_lua_callback(state, &callback_id)
                        }
                    };
                }

                if let Some(action) = global_action(&key_event, &keymaps) {
                    return match action {
                        BoundAction::Action(action) => handle_global_action(state, action),
                        BoundAction::Command(command) => execute_bound_command(state, &command),
                        BoundAction::Script(script) => {
                            execute_bound_script(state, &script, "keybinding script")
                        }
                        BoundAction::LuaCallback(callback_id) => {
                            execute_bound_lua_callback(state, &callback_id)
                        }
                    };
                }

                Ok(EventResult::Continue)
            }
            Event::Mouse(mouse_event) => handle_normal_mouse_event(state, mouse_event),
            _ => Ok(EventResult::Continue),
        },
        Mode::Search => {
            if let Event::Key(key_event) = event {
                if !is_handled_key_press(&key_event) {
                    return Ok(EventResult::Continue);
                }
                match key_event.code {
                    KeyCode::Char('q') => return Ok(EventResult::Quit),
                    KeyCode::Esc => {
                        state.mode = state.help_return_mode.clone();
                        return Ok(EventResult::Redraw);
                    }
                    _ => {}
                }
            }
            search::handle_search_event(state, event)
        }
        Mode::Help => {
            if let Event::Key(key_event) = event {
                if !is_handled_key_press(&key_event) {
                    return Ok(EventResult::Continue);
                }
                match key_event.code {
                    KeyCode::Esc => {
                        state.mode = state.help_return_mode.clone();
                        return Ok(EventResult::Redraw);
                    }
                    KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                        if state.help_next_tab() {
                            return Ok(EventResult::Redraw);
                        }
                    }
                    KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                        if state.help_prev_tab() {
                            return Ok(EventResult::Redraw);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if state.help_next_section() {
                            return Ok(EventResult::Redraw);
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if state.help_prev_section() {
                            return Ok(EventResult::Redraw);
                        }
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        if state.help_first_section() {
                            return Ok(EventResult::Redraw);
                        }
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        if state.help_last_section() {
                            return Ok(EventResult::Redraw);
                        }
                    }
                    _ => {}
                }
                let keymaps = configure::current_keymaps();
                if let Some(action) = global_action(&key_event, &keymaps) {
                    return match action {
                        BoundAction::Action(action) => handle_global_action(state, action),
                        BoundAction::Command(command) => execute_bound_command(state, &command),
                        BoundAction::Script(script) => {
                            execute_bound_script(state, &script, "keybinding script")
                        }
                        BoundAction::LuaCallback(callback_id) => {
                            execute_bound_lua_callback(state, &callback_id)
                        }
                    };
                }
            }
            Ok(EventResult::Continue)
        }
    }
}

fn apply_focus_action(state: &mut AppState<'_>, direction: Direction) {
    match direction {
        Direction::Left => state.focus_left(),
        Direction::Right => state.focus_right(),
        Direction::Up => state.focus_up(),
        Direction::Down => state.focus_down(),
    }
}
