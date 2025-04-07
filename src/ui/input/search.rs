use std::{cell::RefCell, rc::Rc};

use ratatui::crossterm::event::Event;

use crate::ui::app::{AppError, AppState};

use super::EventResult;

pub fn handle_search_event<'a>(
    state: &mut AppState<'a>,
    event: Event,
) -> Result<EventResult, AppError> {
    Ok(EventResult::Continue)
}
