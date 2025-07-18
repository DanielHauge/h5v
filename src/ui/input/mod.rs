use attributes::handle_normal_attributes;
use content::handle_normal_content_event;
use ratatui::crossterm::{
    event::{Event, KeyCode, KeyModifiers},
    style::Attributes,
};
use tree::handle_normal_tree_event;

use crate::{error::AppError, h5f::Node};

use super::state::{AppState, Focus, LastFocused, Mode};

pub mod attributes;
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
        Mode::Normal | Mode::Command => {
            if let Event::Key(key_event) = event {
                match (key_event.code, key_event.modifiers) {
                    (KeyCode::Char(':'), _) => {
                        state.mode = match state.mode {
                            Mode::Command => Mode::Normal,
                            _ => Mode::Command,
                        };
                        state.focus = match state.mode {
                            Mode::Command => Focus::Content,
                            _ => Focus::Tree(LastFocused::Content),
                        };
                        Ok(EventResult::Redraw)
                    }
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
                    (KeyCode::Up, KeyModifiers::CONTROL) => match state.content_mode {
                        super::state::ContentShowMode::Preview => {
                            // if state.img_state.idx_to_load > 0 {
                            //     state.img_state.idx_to_load -= 1;
                            //     Ok(EventResult::Redraw)
                            // } else {
                            //     Ok(EventResult::Continue)
                            // }
                            Ok(EventResult::Continue)
                        }
                        super::state::ContentShowMode::Matrix => {
                            let current_node =
                                &state.treeview[state.tree_view_cursor].node.borrow().node;
                            if state.matrix_view_state.row_offset == 0 {
                                return Ok(EventResult::Continue);
                            }
                            if let Node::Dataset(_, dsattr) = current_node {
                                let row_selected_shape = dsattr.shape[state.selected_x_dim];
                                state.matrix_view_state.row_offset =
                                    (state.matrix_view_state.row_offset - 1).min(
                                        row_selected_shape
                                            - state.matrix_view_state.rows_currently_available,
                                    );
                                Ok(EventResult::Redraw)
                            } else {
                                Ok(EventResult::Continue)
                            }
                        }
                    },
                    (KeyCode::Down, KeyModifiers::CONTROL) => match state.content_mode {
                        super::state::ContentShowMode::Preview => Ok(EventResult::Continue),
                        super::state::ContentShowMode::Matrix => {
                            let current_node =
                                &state.treeview[state.tree_view_cursor].node.borrow().node;
                            if let Node::Dataset(_, dsattr) = current_node {
                                let row_selected_shape = dsattr.shape[state.selected_x_dim];
                                state.matrix_view_state.row_offset =
                                    (state.matrix_view_state.row_offset + 1).min(
                                        row_selected_shape
                                            - state.matrix_view_state.rows_currently_available,
                                    );
                                Ok(EventResult::Redraw)
                            } else {
                                Ok(EventResult::Continue)
                            }
                        }
                    },
                    (KeyCode::Right, KeyModifiers::CONTROL) => match state.content_mode {
                        super::state::ContentShowMode::Preview => {
                            if state.segment_state.segumented {
                                if state.img_state.idx_to_load
                                    < state.segment_state.segment_count - 1
                                {
                                    state.img_state.idx_to_load += 1;
                                    Ok(EventResult::Redraw)
                                } else {
                                    Ok(EventResult::Continue)
                                }
                            } else {
                                state.img_state.idx_to_load = state.segment_state.idx;
                                Ok(EventResult::Redraw)
                            }
                        }
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
                        super::state::ContentShowMode::Preview => {
                            if state.img_state.idx_to_load > 0 && state.segment_state.segumented {
                                state.img_state.idx_to_load -= 1;
                                Ok(EventResult::Redraw)
                            } else {
                                Ok(EventResult::Continue)
                            }
                        }
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
                    (KeyCode::PageDown, _) => {
                        let current_node =
                            &state.treeview[state.tree_view_cursor].node.borrow().node;
                        if let Node::Dataset(_, dsattr) = current_node {
                            let row_selected_shape = dsattr.shape[state.selected_x_dim];
                            state.matrix_view_state.row_offset =
                                (state.matrix_view_state.row_offset + 20).min(
                                    row_selected_shape
                                        - state.matrix_view_state.rows_currently_available,
                                );
                            Ok(EventResult::Redraw)
                        } else {
                            Ok(EventResult::Continue)
                        }
                    }
                    (KeyCode::PageUp, _) => {
                        let current_node =
                            &state.treeview[state.tree_view_cursor].node.borrow().node;
                        if state.matrix_view_state.row_offset == 0 {
                            return Ok(EventResult::Continue);
                        }
                        if let Node::Dataset(_, dsattr) = current_node {
                            let row_selected_shape = dsattr.shape[state.selected_x_dim];
                            if state.matrix_view_state.row_offset < 20 {
                                state.matrix_view_state.row_offset = 0;
                            } else {
                                state.matrix_view_state.row_offset =
                                    (state.matrix_view_state.row_offset - 20).min(
                                        row_selected_shape
                                            - state.matrix_view_state.rows_currently_available,
                                    );
                            }
                            Ok(EventResult::Redraw)
                        } else {
                            Ok(EventResult::Continue)
                        }
                    }
                    _ => match state.focus {
                        Focus::Tree(_) => handle_normal_tree_event(state, event),
                        Focus::Attributes => handle_normal_attributes(state, event),
                        Focus::Content => handle_normal_content_event(state, event),
                        Focus::Command => handle_normal_content_event(state, event),
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
