use attributes::handle_normal_attributes;
use content::handle_normal_content_event;
use keymap::{normal_action, window_action, Direction, NormalAction, WindowAction};
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind,
};
use tree::handle_normal_tree_event;

use crate::{
    error::AppError,
    h5f::{FixedStringRewrite, Node},
    search::{full_traversal, Searcher},
    ui::state::{AppToast, AttributeCreateField, FixedStringOverflowChoice},
};

use super::state::{AppState, AttributeViewSelection, Focus, Mode, PendingChord};

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

                if state.pending_chord == Some(PendingChord::CtrlW) {
                    state.pending_chord = None;
                    if let Some(action) = window_action(&key_event) {
                        return match action {
                            WindowAction::Focus(direction) => {
                                apply_focus_action(state, direction);
                                Ok(EventResult::Redraw)
                            }
                            WindowAction::ToggleTreeView => {
                                state.toggle_tree_view();
                                Ok(EventResult::Redraw)
                            }
                        };
                    }
                    return Ok(EventResult::Continue);
                }

                if let Some(action) = normal_action(&key_event) {
                    return match action {
                        NormalAction::EnterCommand => {
                            state.command_return_mode = Mode::Normal;
                            state.mode = Mode::Command;
                            state.command_state.begin_new_entry();
                            Ok(EventResult::Redraw)
                        }
                        NormalAction::RepeatCommand => state.reexecute_command(),
                        NormalAction::EnterSearch => {
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
                        NormalAction::Quit => Ok(EventResult::Quit),
                        NormalAction::ToggleContentMode => {
                            let available = state.treeview[state.tree_view_cursor]
                                .node
                                .borrow_mut()
                                .content_show_modes();
                            state.swap_content_show_mode(available);
                            Ok(EventResult::Redraw)
                        }
                        NormalAction::ShowHelp => {
                            state.mode = Mode::Help;
                            Ok(EventResult::Redraw)
                        }
                        NormalAction::ToggleMultiChart => {
                            state.mode = Mode::MultiChart;
                            Ok(EventResult::Redraw)
                        }
                        NormalAction::ToggleTreeView => {
                            state.toggle_tree_view();
                            Ok(EventResult::Redraw)
                        }
                        NormalAction::ReloadFile => Ok(EventResult::ReloadFile {
                            write: !state.readonly,
                        }),
                        NormalAction::Focus(direction) => {
                            apply_focus_action(state, direction);
                            Ok(EventResult::Redraw)
                        }
                        NormalAction::StartWindowChord => {
                            state.pending_chord = Some(PendingChord::CtrlW);
                            Ok(EventResult::Continue)
                        }
                        NormalAction::ChangeX(delta) => state.change_x(delta),
                        NormalAction::ChangeRow(delta) => state.change_row(delta),
                        NormalAction::ChangeCol(delta) => state.change_col(delta),
                        NormalAction::ChangeSelectedIndex(delta) => {
                            state.change_selected_index(delta)
                        }
                        NormalAction::ChangeSelectedDimension(delta) => {
                            state.change_selected_dimension(delta)
                        }
                        NormalAction::Scroll(direction, amount) => match direction {
                            Direction::Left => state.left(amount as isize),
                            Direction::Right => state.right(amount as isize),
                            Direction::Up => state.up(amount),
                            Direction::Down => state.down(amount),
                        },
                    };
                }

                match state.focus {
                    Focus::Tree(_) => handle_normal_tree_event(state, Event::Key(key_event)),
                    Focus::Attributes => handle_normal_attributes(state, Event::Key(key_event)),
                    Focus::Content => handle_normal_content_event(state, Event::Key(key_event)),
                }
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
                if key_event.code == KeyCode::Esc {
                    state.mode = Mode::Normal;
                    return Ok(EventResult::Redraw);
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
        state.content_mode = tab_hitbox.mode;
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
        state.matrix_view_state.cursor_row = matrix_cell.row;
        state.matrix_view_state.cursor_col = matrix_cell.col;
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
