use std::{cell::RefCell, rc::Rc};

use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};
use tree::handle_normal_tree_event;

use crate::error::AppError;

use super::app::{AppState, Focus, Mode};

pub mod search;
pub mod tree;

pub enum EventResult {
    Quit,
    Redraw,
    Continue,
    RedrawTreeCompute,
}

pub fn handle_input_event<'a>(
    state: &Rc<RefCell<AppState<'a>>>,
    event: Event,
) -> Result<EventResult, AppError> {
    if let Event::Resize(_, __) = event {
        return Ok(EventResult::Redraw);
    }

    let mut state = state.borrow_mut();
    match state.mode {
        Mode::Normal => {
            if let Event::Key(key_event) = event {
                match (key_event.code, key_event.modifiers) {
                    (KeyCode::Char('/'), _) => {
                        state.searcher.borrow_mut().query.clear();
                        state.searcher.borrow_mut().line_cursor = 0;
                        state.mode = Mode::Search;
                        return Ok(EventResult::Redraw);
                    }
                    (KeyCode::Char('q'), _) => return Ok(EventResult::Quit),
                    (KeyCode::Char('?'), _) => {
                        state.mode = Mode::Help;
                        return Ok(EventResult::Redraw);
                    }
                    (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                        if let Focus::Tree = state.focus {
                            state.focus = Focus::Attributes;
                        } else {
                            state.focus = Focus::Tree;
                        }
                        state.show_tree_view = !state.show_tree_view;

                        return Ok(EventResult::Redraw);
                    }

                    _ => match state.focus {
                        Focus::Tree => handle_normal_tree_event(&mut state, event),
                        Focus::Attributes => Ok(EventResult::Continue),
                    },
                }
            } else {
                return Ok(EventResult::Continue);
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
            search::handle_search_event(&mut state, event)
        }
        Mode::Help => todo!(),
    }
}
