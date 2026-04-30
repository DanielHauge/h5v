use attributes::handle_normal_attributes;
use content::handle_normal_content_event;
use keymap::{normal_action, window_action, Direction, NormalAction, WindowAction};
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEvent, MouseEventKind};
use tree::handle_normal_tree_event;

use crate::{
    error::AppError,
    h5f::{FixedStringRewrite, Node},
    search::{full_traversal, Searcher},
    ui::state::{AppToast, FixedStringOverflowChoice},
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
    Continue,
    Error(String),
    Toast(AppToast, bool),
}

pub fn handle_input_event(state: &mut AppState<'_>, event: Event) -> Result<EventResult, AppError> {
    if let Event::Resize(_, __) = event {
        return Ok(EventResult::Redraw);
    }

    match state.mode {
        Mode::Command => command::handle_command_event(state, event),
        Mode::MultiChart => mchart::handle_mchart_event(state, event),
        Mode::FixedStringOverflowDialog => handle_fixed_string_overflow_dialog(state, event),
        Mode::FixedStringResizeDialog => handle_fixed_string_resize_dialog(state, event),
        Mode::Normal => match event {
            Event::Key(key_event) => {
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
                            state.mode = Mode::Command;
                            state.command_state.command_buffer.clear();
                            state.command_state.cursor = 0;
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
            handle_left_click(state, mouse_event.column, mouse_event.row)
        }
        _ => Ok(EventResult::Continue),
    }
}

fn handle_left_click(
    state: &mut AppState<'_>,
    column: u16,
    row: u16,
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
                    state.tree_view_cursor = clicked_index;
                }
            }
            return Ok(EventResult::Redraw);
        }
    }

    if let Some(attributes) = state.ui_layout.attributes {
        if point_in_rect(attributes.outer, column, row) {
            state.focus = Focus::Attributes;
            if point_in_rect(attributes.inner, column, row) {
                let clicked_row = row.saturating_sub(attributes.inner.y) as usize;
                let clicked_index = attributes.row_offset.saturating_add(clicked_row);
                if clicked_row < attributes.visible_rows && clicked_index < attributes.total_rows {
                    let selection = if point_in_rect(attributes.name_area, column, row) {
                        AttributeViewSelection::Name
                    } else if point_in_rect(attributes.value_area, column, row) {
                        AttributeViewSelection::Value
                    } else {
                        AttributeViewSelection::Value
                    };
                    if let Some(tree_item) = state.treeview.get(state.tree_view_cursor) {
                        let mut node = tree_item.node.borrow_mut();
                        node.attributes_view_cursor.attribute_index = clicked_index;
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

fn handle_fixed_string_resize_dialog(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    let Event::Key(key_event) = event else {
        return Ok(EventResult::Continue);
    };
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
