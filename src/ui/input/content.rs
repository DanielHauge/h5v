use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind};

use crate::{error::AppError, ui::state::AppState};

use super::EventResult;

pub fn handle_normal_content_event(
    _state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match (key_event.code, key_event.modifiers) {
                (KeyCode::Up, _) => {
                    // Get the current tree item and its attributes
                    Ok(EventResult::Continue)
                }
                (KeyCode::Down, _) => {
                    // Get the current tree item and its attributes
                    Ok(EventResult::Continue)
                }
                _ => Ok(EventResult::Continue),
            },
            KeyEventKind::Repeat => Ok(EventResult::Continue),
            KeyEventKind::Release => Ok(EventResult::Continue),
        },
        Event::Resize(_, _) => Ok(EventResult::Redraw),
        _ => Ok(EventResult::Continue),
    }
}
