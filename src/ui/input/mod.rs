use attributes::handle_normal_attributes;
use content::handle_normal_content_event;
use keymap::{normal_action, window_action, Direction, NormalAction, WindowAction};
use ratatui::crossterm::event::{Event, KeyCode};
use tree::handle_normal_tree_event;

use crate::{
    error::AppError,
    h5f::Node,
    search::{full_traversal, Searcher},
    ui::state::AppToast,
};

use super::state::{AppState, Mode, PendingChord};

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
        Mode::Normal => {
            if let Event::Key(key_event) = event {
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
                    super::state::Focus::Tree(_) => handle_normal_tree_event(state, event),
                    super::state::Focus::Attributes => handle_normal_attributes(state, event),
                    super::state::Focus::Content => handle_normal_content_event(state, event),
                }
            } else {
                Ok(EventResult::Continue)
            }
        }
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

fn apply_focus_action(state: &mut AppState<'_>, direction: Direction) {
    match direction {
        Direction::Left => state.focus_left(),
        Direction::Right => state.focus_right(),
        Direction::Up => state.focus_up(),
        Direction::Down => state.focus_down(),
    }
}
