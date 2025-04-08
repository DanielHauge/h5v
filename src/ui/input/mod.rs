use std::{cell::RefCell, rc::Rc};

use ratatui::crossterm::event::{Event, KeyCode};
use tree::handle_normal_tree_event;

use super::app::{AppError, AppState, Focus, Mode};

pub mod search;
pub mod tree;

pub enum EventResult {
    Quit,
    Redraw,
    Continue,
}

pub fn handle_input_event<'a>(
    state: &Rc<RefCell<AppState<'a>>>,
    event: Event,
) -> Result<EventResult, AppError> {
    let mut state = state.borrow_mut();
    match state.mode {
        Mode::Normal => {
            if let Event::Key(key_event) = event {
                match key_event.code {
                    KeyCode::Char('/') => {
                        state.mode = Mode::Search;
                        return Ok(EventResult::Redraw);
                    }
                    KeyCode::Char('q') => return Ok(EventResult::Quit),
                    KeyCode::Char('?') => {
                        state.mode = Mode::Help;
                        return Ok(EventResult::Redraw);
                    }

                    _ => match state.focus {
                        Focus::Tree => handle_normal_tree_event(&mut state, event),
                        Focus::Attributes => todo!(),
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
