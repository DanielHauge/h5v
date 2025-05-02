use attributes::handle_normal_attributes;
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};
use tree::handle_normal_tree_event;

use crate::error::AppError;

use super::state::{AppState, Focus, Mode};

pub mod attributes;
pub mod search;
pub mod tree;

pub enum EventResult {
    Quit,
    Redraw,
    Continue,
}

pub fn handle_input_event(state: &mut AppState<'_>, event: Event) -> Result<EventResult, AppError> {
    if let Event::Resize(_, __) = event {
        return Ok(EventResult::Redraw);
    }

    match state.mode {
        Mode::Normal => {
            if let Event::Key(key_event) = event {
                match (key_event.code, key_event.modifiers) {
                    (KeyCode::Char('/'), _) => {
                        state.searcher.borrow_mut().query.clear();
                        state.searcher.borrow_mut().line_cursor = 0;
                        state.mode = Mode::Search;
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Char('q'), _) => Ok(EventResult::Quit),
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => Ok(EventResult::Quit),
                    (KeyCode::Char('?'), _) => {
                        state.mode = Mode::Help;
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Right, KeyModifiers::SHIFT) => {
                        if let Focus::Tree = state.focus {
                            state.focus = Focus::Attributes;
                        }
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Left, KeyModifiers::SHIFT) => {
                        if let Focus::Attributes = state.focus {
                            state.focus = Focus::Tree;
                        }
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                        if let Focus::Tree = state.focus {
                            state.focus = Focus::Attributes;
                        } else {
                            state.focus = Focus::Tree;
                        }
                        state.show_tree_view = !state.show_tree_view;

                        Ok(EventResult::Redraw)
                    }

                    _ => match state.focus {
                        Focus::Tree => handle_normal_tree_event(state, event),
                        Focus::Attributes => handle_normal_attributes(state, event),
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
