use attributes::handle_normal_attributes;
use content::handle_normal_content_event;
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};
use tree::handle_normal_tree_event;

use crate::{error::AppError, h5f::Node};

use super::state::{AppState, Focus, LastFocused, Mode};

pub mod attributes;
pub mod command;
pub mod content;
pub mod search;
pub mod tree;

pub enum EventResult {
    Quit,
    Redraw,
    Copying,
    Continue,
}

pub fn handle_input_event(state: &mut AppState<'_>, event: Event) -> Result<EventResult, AppError> {
    if let Event::Resize(_, __) = event {
        return Ok(EventResult::Redraw);
    }

    match state.mode {
        Mode::Command => command::handle_command_event(state, event),
        Mode::Normal => {
            if let Event::Key(key_event) = event {
                match (key_event.code, key_event.modifiers) {
                    (KeyCode::Char(':'), _) => {
                        state.mode = Mode::Command;
                        state.command_state.command_buffer.clear();
                        state.command_state.cursor = 0;
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Char('.'), _) => state.reexecute_command(),
                    (KeyCode::Char('/'), _) => {
                        state.searcher.borrow_mut().query.clear();
                        state.searcher.borrow_mut().line_cursor = 0;
                        state.mode = Mode::Search;
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Char('q'), _) => Ok(EventResult::Quit),
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => Ok(EventResult::Quit),
                    (KeyCode::Tab, _) => {
                        state.swap_content_show_mode();
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Char('?'), _) => {
                        state.mode = Mode::Help;
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Right, KeyModifiers::SHIFT) => {
                        if let Focus::Tree(LastFocused::Attributes) = state.focus {
                            state.focus = Focus::Attributes;
                        }
                        if let Focus::Tree(LastFocused::Content) = state.focus {
                            state.focus = Focus::Content;
                        }
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Left, KeyModifiers::SHIFT) => {
                        if let Focus::Attributes = state.focus {
                            state.focus = Focus::Tree(LastFocused::Attributes);
                        }
                        if let Focus::Content = state.focus {
                            state.focus = Focus::Tree(LastFocused::Content);
                        }
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Down, KeyModifiers::SHIFT) => {
                        if let Focus::Attributes = state.focus {
                            state.focus = Focus::Content;
                        }
                        Ok(EventResult::Redraw)
                    }

                    (KeyCode::Up, KeyModifiers::SHIFT) => {
                        if let Focus::Content = state.focus {
                            state.focus = Focus::Attributes;
                        }
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                        state.focus = Focus::Content;
                        state.show_tree_view = !state.show_tree_view;

                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Up, KeyModifiers::CONTROL) => state.dec(1),
                    (KeyCode::Down, KeyModifiers::CONTROL) => state.inc(1),
                    (KeyCode::Right, KeyModifiers::CONTROL) => match state.content_mode {
                        super::state::ContentShowMode::Preview => state.inc(1),
                        super::state::ContentShowMode::Matrix => {
                            let current_node =
                                &state.treeview[state.tree_view_cursor].node.borrow().node;
                            if let Node::Dataset(_, dsattr) = current_node {
                                if dsattr.shape.len() > state.selected_y_dim {
                                    let col_selected_shape = dsattr.shape[state.selected_y_dim];
                                    state.matrix_view_state.col_offset =
                                        (state.matrix_view_state.col_offset + 1).min(
                                            col_selected_shape
                                                - state.matrix_view_state.cols_currently_available,
                                        );
                                    Ok(EventResult::Redraw)
                                } else {
                                    Ok(EventResult::Continue)
                                }
                            } else {
                                Ok(EventResult::Continue)
                            }
                        }
                    },
                    (KeyCode::Left, KeyModifiers::CONTROL) => match state.content_mode {
                        super::state::ContentShowMode::Preview => state.dec(1),
                        super::state::ContentShowMode::Matrix => {
                            // If we are at the first column, do nothing
                            let current_node =
                                &state.treeview[state.tree_view_cursor].node.borrow().node;
                            if state.matrix_view_state.col_offset == 0 {
                                return Ok(EventResult::Continue);
                            }
                            if let Node::Dataset(_, dsattr) = current_node {
                                if dsattr.shape.len() > state.selected_y_dim {
                                    let col_selected_shape = dsattr.shape[state.selected_y_dim];
                                    state.matrix_view_state.col_offset =
                                        (state.matrix_view_state.col_offset - 1).min(
                                            col_selected_shape
                                                - state.matrix_view_state.cols_currently_available,
                                        );
                                    Ok(EventResult::Redraw)
                                } else {
                                    Ok(EventResult::Continue)
                                }
                            } else {
                                Ok(EventResult::Continue)
                            }
                        }
                    },
                    (KeyCode::PageDown, _) => state.inc(20),
                    (KeyCode::PageUp, _) => state.dec(20),
                    _ => match state.focus {
                        Focus::Tree(_) => handle_normal_tree_event(state, event),
                        Focus::Attributes => handle_normal_attributes(state, event),
                        Focus::Content => handle_normal_content_event(state, event),
                    },
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
