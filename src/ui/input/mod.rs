use attributes::handle_normal_attributes;
use content::handle_normal_content_event;
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};
use tree::handle_normal_tree_event;

use crate::{
    error::AppError,
    h5f::Node,
    search::{full_traversal, Searcher},
};

use super::state::{AppState, Focus, LastFocused, Mode};

pub mod attributes;
pub mod command;
pub mod content;
pub mod mchart;
pub mod search;
pub mod tree;

pub enum EventResult {
    Quit,
    Redraw,
    Copying,
    Continue,
    Error(String),
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
                match (key_event.code, key_event.modifiers) {
                    (KeyCode::Char(':'), _) => {
                        state.mode = Mode::Command;
                        state.command_state.command_buffer.clear();
                        state.command_state.cursor = 0;
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Char('.'), _) => state.reexecute_command(),
                    (KeyCode::Char('/'), _) => {
                        if state.searcher.is_none() {
                            let Node::File(ref file) = state.root.borrow().node else {
                                panic!("Root node is not a file");
                            };
                            let all_h5_paths = full_traversal(&file.as_group().unwrap());
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
                    (KeyCode::Char('q'), _) => Ok(EventResult::Quit),
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => Ok(EventResult::Quit),
                    (KeyCode::Tab, _) => {
                        let available = state.treeview[state.tree_view_cursor]
                            .node
                            .borrow_mut()
                            .content_show_modes();
                        state.swap_content_show_mode(available);
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Char('?'), _) => {
                        state.mode = Mode::Help;
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Char('M'), _) => {
                        state.mode = Mode::MultiChart;
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
                        state.show_tree_view = !state.show_tree_view;
                        if state.show_tree_view {
                            state.focus = Focus::Tree(LastFocused::Content);
                        } else {
                            state.focus = Focus::Content;
                        }

                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Char('x'), _) => state.change_x(1),
                    (KeyCode::Char('X'), _) => state.change_x(-1),
                    (KeyCode::Char('r'), _) => state.change_row(1),
                    (KeyCode::Char('R'), _) => state.change_row(-1),
                    (KeyCode::Char('c'), _) => state.change_col(1),
                    (KeyCode::Char('C'), _) => state.change_col(-1),
                    (KeyCode::Up, KeyModifiers::ALT) => state.change_selected_index(-1),
                    (KeyCode::Down, KeyModifiers::ALT) => state.change_selected_index(1),
                    (KeyCode::PageUp, KeyModifiers::ALT) => state.change_selected_index(-10),
                    (KeyCode::PageDown, KeyModifiers::ALT) => state.change_selected_index(10),
                    (KeyCode::Left, KeyModifiers::ALT) => state.change_selected_dimension(-1),
                    (KeyCode::Right, KeyModifiers::ALT) => state.change_selected_dimension(1),
                    (KeyCode::Up, KeyModifiers::CONTROL) => state.up(1),
                    (KeyCode::Down, KeyModifiers::CONTROL) => state.down(1),
                    (KeyCode::Right, KeyModifiers::CONTROL) => state.right(1),
                    (KeyCode::Left, KeyModifiers::CONTROL) => state.left(1),
                    (KeyCode::PageDown, _) => state.down(20),
                    (KeyCode::PageUp, _) => state.up(20),
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
