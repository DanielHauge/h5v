use std::{cell::RefCell, rc::Rc};

use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};

use crate::ui::app::{AppError, AppState};

use super::EventResult;

pub fn handle_search_event<'a>(
    state: &mut AppState<'a>,
    event: Event,
) -> Result<EventResult, AppError> {
    let root = state.root.borrow();
    let mut searcher = root.searcher.borrow_mut();
    let current_cursor = searcher.line_cursor;

    match event {
        Event::Key(key_event) => match key_event.kind {
            ratatui::crossterm::event::KeyEventKind::Press => {
                // Only allow A-Z, a-z, 0-9, underscore, dash and dot
                match (key_event.code, key_event.modifiers) {
                    (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                        searcher.query.clear();
                        searcher.line_cursor = 0;
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Char(c), _) => {
                        if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                            if current_cursor == searcher.query.len() {
                                searcher.query.push(c);
                                searcher.line_cursor += 1;
                            } else {
                                searcher.query.insert(current_cursor, c);
                                searcher.line_cursor += 1;
                            }
                            Ok(EventResult::Redraw)
                        } else {
                            Ok(EventResult::Continue)
                        }
                    }
                    (KeyCode::Backspace, _) => match key_event.modifiers {
                        ratatui::crossterm::event::KeyModifiers::CONTROL => {
                            searcher.query.clear();
                            searcher.line_cursor = 0;
                            Ok(EventResult::Redraw)
                        }
                        _ => {
                            if searcher.line_cursor > 0 {
                                searcher.query.pop();
                                searcher.line_cursor -= 1;
                                Ok(EventResult::Redraw)
                            } else {
                                Ok(EventResult::Continue)
                            }
                        }
                    },
                    (KeyCode::Delete, _) => {
                        searcher.query.clear();
                        searcher.line_cursor = 0;
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Left, _) => {
                        if searcher.line_cursor > 0 {
                            searcher.line_cursor -= 1;
                        }
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Right, _) => {
                        if searcher.line_cursor < searcher.query.len() {
                            searcher.line_cursor += 1;
                        }
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Up, _) => {
                        if searcher.select_cursor > 0 {
                            searcher.select_cursor -= 1;
                        }
                        let result_count = searcher.count_results();
                        if searcher.select_cursor > result_count {
                            searcher.line_cursor = result_count;
                        }
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Down, _) => {
                        let searcher_count = searcher.count_results();
                        if searcher_count > 0 && searcher.select_cursor < searcher_count - 1 {
                            searcher.select_cursor += 1;
                        }

                        Ok(EventResult::Redraw)
                    }

                    _ => Ok(EventResult::Continue),
                }
            }
            _ => Ok(EventResult::Continue),
        },
        _ => Ok(EventResult::Continue),
    }
}
