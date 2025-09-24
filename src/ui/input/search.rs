use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};

use crate::{
    error::AppError,
    h5f::HasPath,
    ui::state::{AppState, Focus, LastFocused, Mode},
};

use super::EventResult;

pub fn handle_search_event(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            ratatui::crossterm::event::KeyEventKind::Press => {
                // Only allow A-Z, a-z, 0-9, underscore, dash and dot
                match (key_event.code, key_event.modifiers) {
                    (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                        let Some(ref mut searcher) = state.searcher else {
                            return Ok(EventResult::Error("No searcher available".into()));
                        };
                        searcher.query.clear();
                        searcher.line_cursor = 0;
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Char(c), _) => {
                        let Some(ref mut searcher) = state.searcher else {
                            return Ok(EventResult::Error("No searcher available".into()));
                        };
                        let current_cursor = searcher.line_cursor;
                        if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                            if current_cursor == searcher.query.len() {
                                searcher.query.push(c);
                                searcher.line_cursor += 1;
                            } else {
                                searcher.query.insert(current_cursor, c);
                                searcher.line_cursor += 1;
                            }
                            let count_results = searcher.count_results();
                            if count_results == 0 {
                                searcher.select_cursor = count_results;
                            } else if searcher.select_cursor >= count_results {
                                searcher.select_cursor = count_results - 1;
                            }
                            Ok(EventResult::Redraw)
                        } else {
                            Ok(EventResult::Continue)
                        }
                    }
                    (KeyCode::Backspace, _) => match key_event.modifiers {
                        ratatui::crossterm::event::KeyModifiers::CONTROL => {
                            let Some(ref mut searcher) = state.searcher else {
                                return Ok(EventResult::Error("No searcher available".into()));
                            };
                            searcher.query.clear();
                            searcher.line_cursor = 0;
                            Ok(EventResult::Redraw)
                        }
                        _ => {
                            let Some(ref mut searcher) = state.searcher else {
                                return Ok(EventResult::Error("No searcher available".into()));
                            };
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
                        let Some(ref mut searcher) = state.searcher else {
                            return Ok(EventResult::Error("No searcher available".into()));
                        };
                        searcher.query.clear();
                        searcher.line_cursor = 0;
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Left, _) => {
                        let Some(ref mut searcher) = state.searcher else {
                            return Ok(EventResult::Error("No searcher available".into()));
                        };
                        if searcher.line_cursor > 0 {
                            searcher.line_cursor -= 1;
                        }
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Right, _) => {
                        let Some(ref mut searcher) = state.searcher else {
                            return Ok(EventResult::Error("No searcher available".into()));
                        };
                        if searcher.line_cursor < searcher.query.len() {
                            searcher.line_cursor += 1;
                        }
                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Up, _) => {
                        let Some(ref mut searcher) = state.searcher else {
                            return Ok(EventResult::Error("No searcher available".into()));
                        };
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
                        let Some(ref mut searcher) = state.searcher else {
                            return Ok(EventResult::Error("No searcher available".into()));
                        };
                        let searcher_count = searcher.count_results();
                        if searcher_count > 0 && searcher.select_cursor < searcher_count - 1 {
                            searcher.select_cursor += 1;
                        }

                        Ok(EventResult::Redraw)
                    }
                    (KeyCode::Enter, _) => {
                        let Some(ref mut searcher) = state.searcher else {
                            return Ok(EventResult::Error("No searcher available".into()));
                        };

                        let results = searcher.search(&searcher.query);

                        let selected_node = searcher.select_cursor;
                        if results.is_empty() {
                            state.mode = Mode::Normal;
                            return Ok(EventResult::Error("No results found".into()));
                        }

                        let selected_index_corrected = if selected_node >= results.len() {
                            results.len() - 1
                        } else {
                            selected_node
                        };

                        let selected_result = results[selected_index_corrected].to_string();
                        let mut root = state.root.borrow_mut();
                        root.collapse();
                        root.expand_path(selected_result.strip_prefix("/").unwrap())?;
                        drop(root);

                        state.mode = Mode::Normal;
                        state.focus = Focus::Tree(LastFocused::Attributes);
                        state.compute_tree_view();
                        for (i, tree_item) in state.treeview.iter().enumerate() {
                            if tree_item.node.borrow().node.path() == selected_result {
                                state.tree_view_cursor = i;
                                break;
                            }
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
