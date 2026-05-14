use std::cell::RefCell;

use attributes::handle_normal_attributes;
use content::handle_normal_content_event;
use keymap::{
    global_action, normal_action, window_action, BoundAction, Direction, GlobalAction,
    NormalAction, WindowAction,
};
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind,
};
use tree::handle_normal_tree_event;

use crate::{
    configure,
    error::AppError,
    h5f::{FixedStringRewrite, Node},
    search::{full_traversal, Searcher},
    ui::command::{execute_command, parse_command_text, parse_startup_commands},
    ui::state::{AppToast, AttributeCreateField, FixedStringOverflowChoice},
};

use super::state::{AppState, AttributeViewSelection, ContentShowMode, Focus, Mode, PendingChord};

pub mod attributes;
pub mod command;
pub mod content;
pub mod keymap;
pub mod mchart;
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
                        state.mode = Mode::Normal;
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
                        state.mode = Mode::Normal;
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

fn handle_normal_mouse_event(
    state: &mut AppState<'_>,
    mouse_event: MouseEvent,
) -> Result<EventResult, AppError> {
    state.pending_chord = None;

    match mouse_event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            handle_left_click(state, mouse_event.column, mouse_event.row, false)
        }
        MouseEventKind::Down(MouseButton::Right) => {
            handle_right_mouse_down(state, mouse_event.column, mouse_event.row)
        }
        MouseEventKind::Drag(MouseButton::Right) => {
            handle_right_mouse_drag(state, mouse_event.column, mouse_event.row)
        }
        MouseEventKind::Up(MouseButton::Right) => {
            handle_right_mouse_up(state, mouse_event.column, mouse_event.row)
        }
        MouseEventKind::ScrollUp => {
            handle_heatmap_scroll(state, mouse_event.column, mouse_event.row, true)
        }
        MouseEventKind::ScrollDown => {
            handle_heatmap_scroll(state, mouse_event.column, mouse_event.row, false)
        }
        _ => Ok(EventResult::Continue),
    }
}

fn handle_left_click(
    state: &mut AppState<'_>,
    column: u16,
    row: u16,
    toggle_if_selected: bool,
) -> Result<EventResult, AppError> {
    if let Some(tab_hitbox) = state
        .ui_layout
        .content_tabs
        .iter()
        .find(|tab| point_in_rect(tab.area, column, row))
        .copied()
    {
        state.set_content_mode(tab_hitbox.mode);
        state.focus = Focus::Content;
        return Ok(EventResult::Redraw);
    }

    if let Some(matrix_cell) = state
        .ui_layout
        .matrix_cells
        .iter()
        .find(|cell| point_in_rect(cell.area, column, row))
        .copied()
    {
        state.focus = Focus::Content;
        if state.active_content_mode() == ContentShowMode::Heatmap {
            state.heatmap_select_cell(matrix_cell.row, matrix_cell.col);
        } else {
            state.matrix_view_state.cursor_row = matrix_cell.row;
            state.matrix_view_state.cursor_col = matrix_cell.col;
        }
        return Ok(EventResult::Redraw);
    }

    if let Some(setting_hitbox) = state
        .ui_layout
        .heatmap_settings
        .iter()
        .find(|hitbox| point_in_rect(hitbox.area, column, row))
        .copied()
    {
        state.focus = Focus::Content;
        state.heatmap_render.selected_setting = setting_hitbox.setting;
        return Ok(EventResult::Redraw);
    }

    if let Some(matrix_row) = state
        .ui_layout
        .matrix_rows
        .iter()
        .find(|row_hitbox| point_in_rect(row_hitbox.area, column, row))
        .copied()
    {
        state.focus = Focus::Content;
        state.matrix_view_state.cursor_row = matrix_row.row;
        return Ok(EventResult::Redraw);
    }

    if let Some(tree) = state.ui_layout.tree {
        if point_in_rect(tree.outer, column, row) {
            state.focus_tree_from_current();
            if point_in_rect(tree.inner, column, row) {
                let clicked_row = row.saturating_sub(tree.inner.y) as usize;
                let clicked_index = tree.row_offset.saturating_add(clicked_row);
                if clicked_row < tree.visible_rows && clicked_index < state.treeview.len() {
                    let was_selected = state.tree_view_cursor == clicked_index;
                    state.tree_view_cursor = clicked_index;
                    if was_selected || toggle_if_selected {
                        let tree_item = &state.treeview[clicked_index];
                        if tree_item.load_more {
                            tree_item.node.borrow_mut().view_loaded += 50;
                            state.compute_tree_view();
                            return Ok(EventResult::Redraw);
                        }
                        if tree_item.node.borrow().is_expandable() {
                            tree_item.node.borrow_mut().expand_toggle()?;
                            state.compute_tree_view();
                            return Ok(EventResult::Redraw);
                        }
                    }
                }
            }
            return Ok(EventResult::Redraw);
        }
    }

    if let Some(attributes) = state.ui_layout.attributes.clone() {
        if point_in_rect(attributes.outer, column, row) {
            state.focus = Focus::Attributes;
            if point_in_rect(attributes.inner, column, row) {
                if let Some(cell) = attributes.cells.iter().find(|cell| {
                    point_in_rect(cell.name_area, column, row)
                        || point_in_rect(cell.value_area, column, row)
                }) {
                    let selection = if point_in_rect(cell.name_area, column, row) {
                        AttributeViewSelection::Name
                    } else {
                        AttributeViewSelection::Value
                    };
                    if let Some(tree_item) = state.treeview.get(state.tree_view_cursor) {
                        let mut node = tree_item.node.borrow_mut();
                        node.attributes_view_cursor.attribute_index = cell.row_index;
                        node.attributes_view_cursor.attribute_view_selection = selection;
                    }
                }
            }
            return Ok(EventResult::Redraw);
        }
    }

    if let Some(content) = state.ui_layout.content {
        if point_in_rect(content, column, row) {
            state.focus = Focus::Content;
            return Ok(EventResult::Redraw);
        }
    }

    Ok(EventResult::Continue)
}

fn handle_heatmap_scroll(
    state: &mut AppState<'_>,
    column: u16,
    row: u16,
    zoom_in: bool,
) -> Result<EventResult, AppError> {
    if state.active_content_mode() != ContentShowMode::Heatmap {
        return Ok(EventResult::Continue);
    }
    let Some(matrix_cell) = state
        .ui_layout
        .matrix_cells
        .iter()
        .find(|cell| point_in_rect(cell.area, column, row))
        .copied()
    else {
        return Ok(EventResult::Continue);
    };
    state.focus = Focus::Content;
    if state.zoom_heatmap_step(Some((matrix_cell.row, matrix_cell.col)), zoom_in) {
        Ok(EventResult::Redraw)
    } else {
        Ok(EventResult::Continue)
    }
}

fn handle_right_mouse_down(
    state: &mut AppState<'_>,
    column: u16,
    row: u16,
) -> Result<EventResult, AppError> {
    if state.active_content_mode() != ContentShowMode::Heatmap {
        return Ok(EventResult::Continue);
    }
    let Some(matrix_cell) = state
        .ui_layout
        .matrix_cells
        .iter()
        .find(|cell| point_in_rect(cell.area, column, row))
        .copied()
    else {
        return Ok(EventResult::Continue);
    };
    state.focus = Focus::Content;
    if state.start_heatmap_drag(matrix_cell.row, matrix_cell.col) {
        Ok(EventResult::Redraw)
    } else {
        Ok(EventResult::Continue)
    }
}

fn handle_right_mouse_drag(
    state: &mut AppState<'_>,
    _column: u16,
    _row: u16,
) -> Result<EventResult, AppError> {
    if state.active_content_mode() == ContentShowMode::Heatmap
        && state.heatmap_render.drag_state.is_some()
    {
        return Ok(EventResult::Continue);
    }
    Ok(EventResult::Continue)
}

fn handle_right_mouse_up(
    state: &mut AppState<'_>,
    column: u16,
    row: u16,
) -> Result<EventResult, AppError> {
    if state.active_content_mode() != ContentShowMode::Heatmap {
        return Ok(EventResult::Continue);
    }
    let Some(drag_state) = state.heatmap_render.drag_state else {
        return Ok(EventResult::Continue);
    };
    let release_cell = state
        .ui_layout
        .matrix_cells
        .iter()
        .find(|cell| point_in_rect(cell.area, column, row))
        .map(|cell| (cell.row, cell.col))
        .unwrap_or((drag_state.anchor_row, drag_state.anchor_col));
    state.focus = Focus::Content;
    if release_cell == (drag_state.anchor_row, drag_state.anchor_col) {
        state.end_heatmap_drag();
        if state.heatmap_render.selected_cells.is_some() && state.zoom_heatmap(None, true) {
            return Ok(EventResult::Redraw);
        }
        return Ok(EventResult::Continue);
    }
    if state.finish_heatmap_drag(release_cell.0, release_cell.1) {
        Ok(EventResult::Redraw)
    } else {
        Ok(EventResult::Continue)
    }
}

fn point_in_rect(rect: ratatui::layout::Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn handle_fixed_string_overflow_dialog(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    let Event::Key(key_event) = event else {
        return Ok(EventResult::Continue);
    };
    if !is_handled_key_press(&key_event) {
        return Ok(EventResult::Continue);
    }
    let Some(dialog) = state.fixed_string_overflow_dialog.as_mut() else {
        state.mode = Mode::Normal;
        return Ok(EventResult::Redraw);
    };

    match key_event.code {
        KeyCode::Esc => {
            state.fixed_string_overflow_dialog = None;
            state.mode = Mode::Normal;
            Ok(EventResult::Redraw)
        }
        KeyCode::Left | KeyCode::Up | KeyCode::Char('h') | KeyCode::Char('k') => {
            dialog.selected_choice = match dialog.selected_choice {
                FixedStringOverflowChoice::Cancel => FixedStringOverflowChoice::ChangeSize,
                FixedStringOverflowChoice::ChangeToVarLen => FixedStringOverflowChoice::Cancel,
                FixedStringOverflowChoice::ChangeSize => FixedStringOverflowChoice::ChangeToVarLen,
            };
            Ok(EventResult::Redraw)
        }
        KeyCode::Right | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('l') | KeyCode::Tab => {
            dialog.selected_choice = match dialog.selected_choice {
                FixedStringOverflowChoice::Cancel => FixedStringOverflowChoice::ChangeToVarLen,
                FixedStringOverflowChoice::ChangeToVarLen => FixedStringOverflowChoice::ChangeSize,
                FixedStringOverflowChoice::ChangeSize => FixedStringOverflowChoice::Cancel,
            };
            Ok(EventResult::Redraw)
        }
        KeyCode::Enter => match dialog.selected_choice {
            FixedStringOverflowChoice::Cancel => {
                state.fixed_string_overflow_dialog = None;
                state.mode = Mode::Normal;
                Ok(EventResult::Redraw)
            }
            FixedStringOverflowChoice::ChangeSize => {
                state.mode = Mode::FixedStringResizeDialog;
                Ok(EventResult::Redraw)
            }
            FixedStringOverflowChoice::ChangeToVarLen => {
                let (attr_name, new_value) =
                    (dialog.request.attr_name.clone(), dialog.new_value.clone());
                let mut selected_node = state.treeview[state.tree_view_cursor].node.borrow_mut();
                match selected_node.rewrite_fixed_string_attribute(
                    &attr_name,
                    &new_value,
                    FixedStringRewrite::ToVarLen,
                ) {
                    Ok(()) => {
                        drop(selected_node);
                        state.fixed_string_overflow_dialog = None;
                        state.mode = Mode::Normal;
                        state.acknowledge_file_write();
                        Ok(EventResult::ReloadFile {
                            write: !state.readonly,
                        })
                    }
                    Err(err) => Ok(EventResult::Toast(AppToast::Error(err.to_string()), false)),
                }
            }
        },
        _ => Ok(EventResult::Continue),
    }
}

fn handle_attribute_create_dialog(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    let Event::Key(key_event) = event else {
        return Ok(EventResult::Continue);
    };
    if !is_handled_key_press(&key_event) {
        return Ok(EventResult::Continue);
    }
    let Some(dialog) = state.attribute_create_dialog.as_mut() else {
        state.mode = Mode::Normal;
        return Ok(EventResult::Toast(AppToast::Empty, true));
    };

    match key_event.code {
        KeyCode::Esc => {
            state.attribute_create_dialog = None;
            state.mode = Mode::Normal;
            Ok(EventResult::Toast(AppToast::Empty, true))
        }
        KeyCode::BackTab | KeyCode::Up => {
            dialog.active_field = match dialog.active_field {
                AttributeCreateField::Name => AttributeCreateField::Value,
                AttributeCreateField::Type => AttributeCreateField::Name,
                AttributeCreateField::Value => AttributeCreateField::Type,
            };
            Ok(EventResult::Redraw)
        }
        KeyCode::Tab | KeyCode::Down => {
            dialog.active_field = match dialog.active_field {
                AttributeCreateField::Name => AttributeCreateField::Type,
                AttributeCreateField::Type => AttributeCreateField::Value,
                AttributeCreateField::Value => AttributeCreateField::Name,
            };
            Ok(EventResult::Redraw)
        }
        KeyCode::Enter => match dialog.active_field {
            AttributeCreateField::Name => {
                dialog.active_field = AttributeCreateField::Type;
                Ok(EventResult::Redraw)
            }
            AttributeCreateField::Type => {
                dialog.active_field = AttributeCreateField::Value;
                Ok(EventResult::Redraw)
            }
            AttributeCreateField::Value => {
                let (name, attr_type, value) =
                    (dialog.name.clone(), dialog.attr_type, dialog.value.clone());
                let mut selected_node = state.treeview[state.tree_view_cursor].node.borrow_mut();
                let created_type = selected_node.create_attribute(&name, attr_type, &value)?;
                drop(selected_node);
                state.attribute_create_dialog = None;
                state.mode = Mode::Normal;
                state.acknowledge_file_write();
                Ok(EventResult::Toast(
                    AppToast::Info(format!("Created attribute '{}' ({})", name, created_type)),
                    true,
                ))
            }
        },
        KeyCode::Left | KeyCode::Char('h') if dialog.active_field == AttributeCreateField::Type => {
            let idx = crate::h5f::AttributeCreateType::ALL
                .iter()
                .position(|kind| *kind == dialog.attr_type)
                .unwrap_or(0);
            let prev = if idx == 0 {
                crate::h5f::AttributeCreateType::ALL.len() - 1
            } else {
                idx - 1
            };
            dialog.attr_type = crate::h5f::AttributeCreateType::ALL[prev];
            Ok(EventResult::Redraw)
        }
        KeyCode::Right | KeyCode::Char('l')
            if dialog.active_field == AttributeCreateField::Type =>
        {
            let idx = crate::h5f::AttributeCreateType::ALL
                .iter()
                .position(|kind| *kind == dialog.attr_type)
                .unwrap_or(0);
            dialog.attr_type = crate::h5f::AttributeCreateType::ALL
                [(idx + 1) % crate::h5f::AttributeCreateType::ALL.len()];
            Ok(EventResult::Redraw)
        }
        KeyCode::Backspace => {
            let (buffer, cursor) = match dialog.active_field {
                AttributeCreateField::Name => (&mut dialog.name, &mut dialog.name_cursor),
                AttributeCreateField::Type => return Ok(EventResult::Continue),
                AttributeCreateField::Value => (&mut dialog.value, &mut dialog.value_cursor),
            };
            if *cursor > 0 {
                *cursor -= 1;
                buffer.remove(*cursor);
            }
            Ok(EventResult::Redraw)
        }
        KeyCode::Delete => {
            let (buffer, cursor) = match dialog.active_field {
                AttributeCreateField::Name => (&mut dialog.name, &mut dialog.name_cursor),
                AttributeCreateField::Type => return Ok(EventResult::Continue),
                AttributeCreateField::Value => (&mut dialog.value, &mut dialog.value_cursor),
            };
            if *cursor < buffer.len() {
                buffer.remove(*cursor);
            }
            Ok(EventResult::Redraw)
        }
        KeyCode::Home => {
            match dialog.active_field {
                AttributeCreateField::Name => dialog.name_cursor = 0,
                AttributeCreateField::Type => {}
                AttributeCreateField::Value => dialog.value_cursor = 0,
            }
            Ok(EventResult::Redraw)
        }
        KeyCode::End => {
            match dialog.active_field {
                AttributeCreateField::Name => dialog.name_cursor = dialog.name.len(),
                AttributeCreateField::Type => {}
                AttributeCreateField::Value => dialog.value_cursor = dialog.value.len(),
            }
            Ok(EventResult::Redraw)
        }
        KeyCode::Left if dialog.active_field != AttributeCreateField::Type => {
            match dialog.active_field {
                AttributeCreateField::Name => {
                    dialog.name_cursor = dialog.name_cursor.saturating_sub(1)
                }
                AttributeCreateField::Type => {}
                AttributeCreateField::Value => {
                    dialog.value_cursor = dialog.value_cursor.saturating_sub(1)
                }
            }
            Ok(EventResult::Redraw)
        }
        KeyCode::Right if dialog.active_field != AttributeCreateField::Type => {
            match dialog.active_field {
                AttributeCreateField::Name => {
                    dialog.name_cursor = (dialog.name_cursor + 1).min(dialog.name.len())
                }
                AttributeCreateField::Type => {}
                AttributeCreateField::Value => {
                    dialog.value_cursor = (dialog.value_cursor + 1).min(dialog.value.len())
                }
            }
            Ok(EventResult::Redraw)
        }
        KeyCode::Char(c) if c.is_ascii() && !c.is_ascii_control() => {
            let (buffer, cursor) = match dialog.active_field {
                AttributeCreateField::Name => (&mut dialog.name, &mut dialog.name_cursor),
                AttributeCreateField::Type => return Ok(EventResult::Continue),
                AttributeCreateField::Value => (&mut dialog.value, &mut dialog.value_cursor),
            };
            buffer.insert(*cursor, c);
            *cursor += 1;
            Ok(EventResult::Redraw)
        }
        _ => Ok(EventResult::Continue),
    }
}

fn handle_attribute_delete_dialog(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    let Event::Key(key_event) = event else {
        return Ok(EventResult::Continue);
    };
    if !is_handled_key_press(&key_event) {
        return Ok(EventResult::Continue);
    }
    let Some(dialog) = state.attribute_delete_dialog.as_ref() else {
        state.mode = Mode::Normal;
        return Ok(EventResult::Toast(AppToast::Empty, true));
    };

    match key_event.code {
        KeyCode::Esc => {
            state.attribute_delete_dialog = None;
            state.mode = Mode::Normal;
            Ok(EventResult::Toast(AppToast::Empty, true))
        }
        KeyCode::Enter => {
            let attr_name = dialog.attr_name.clone();
            let mut selected_node = state.treeview[state.tree_view_cursor].node.borrow_mut();
            selected_node.delete_attribute(&attr_name)?;
            drop(selected_node);
            state.attribute_delete_dialog = None;
            state.mode = Mode::Normal;
            state.acknowledge_file_write();
            Ok(EventResult::Toast(
                AppToast::Info(format!("Deleted attribute '{}'", attr_name)),
                true,
            ))
        }
        _ => Ok(EventResult::Continue),
    }
}

fn handle_fixed_string_resize_dialog(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    let Event::Key(key_event) = event else {
        return Ok(EventResult::Continue);
    };
    if !is_handled_key_press(&key_event) {
        return Ok(EventResult::Continue);
    }
    let Some(dialog) = state.fixed_string_overflow_dialog.as_mut() else {
        state.mode = Mode::Normal;
        return Ok(EventResult::Redraw);
    };

    match key_event.code {
        KeyCode::Esc => {
            state.mode = Mode::FixedStringOverflowDialog;
            Ok(EventResult::Redraw)
        }
        KeyCode::Backspace => {
            dialog.size_input.pop();
            Ok(EventResult::Redraw)
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            dialog.size_input.push(c);
            Ok(EventResult::Redraw)
        }
        KeyCode::Enter => {
            let new_size: usize = match dialog.size_input.parse() {
                Ok(size) => size,
                Err(_) => {
                    return Ok(EventResult::Toast(
                        AppToast::Error("Invalid fixed string size".to_string()),
                        false,
                    ))
                }
            };
            if new_size < dialog.overflow.required_size {
                return Ok(EventResult::Toast(
                    AppToast::Error(format!(
                        "New size must be at least {} bytes",
                        dialog.overflow.required_size
                    )),
                    false,
                ));
            }

            let (attr_name, new_value) =
                (dialog.request.attr_name.clone(), dialog.new_value.clone());
            let mut selected_node = state.treeview[state.tree_view_cursor].node.borrow_mut();
            match selected_node.rewrite_fixed_string_attribute(
                &attr_name,
                &new_value,
                FixedStringRewrite::Resize(new_size),
            ) {
                Ok(()) => {
                    drop(selected_node);
                    state.fixed_string_overflow_dialog = None;
                    state.mode = Mode::Normal;
                    state.acknowledge_file_write();
                    Ok(EventResult::ReloadFile {
                        write: !state.readonly,
                    })
                }
                Err(err) => Ok(EventResult::Toast(AppToast::Error(err.to_string()), false)),
            }
        }
        _ => Ok(EventResult::Continue),
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
