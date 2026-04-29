use ratatui::crossterm::event::Event;

use crate::{
    error::AppError,
    h5f::HasPath,
    ui::state::{AppState, Focus, LastFocused, Mode},
};

use super::{
    keymap::{search_action, Direction, SearchAction},
    EventResult,
};

pub fn handle_search_event(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            ratatui::crossterm::event::KeyEventKind::Press => match search_action(&key_event) {
                Some(SearchAction::ClearQuery) => {
                    let Some(ref mut searcher) = state.searcher else {
                        return Ok(EventResult::Error("No searcher available".into()));
                    };
                    searcher.query.clear();
                    searcher.line_cursor = 0;
                    Ok(EventResult::Redraw)
                }
                Some(SearchAction::Insert(c)) => {
                    let Some(ref mut searcher) = state.searcher else {
                        return Ok(EventResult::Error("No searcher available".into()));
                    };
                    let current_cursor = searcher.line_cursor;
                    if current_cursor == searcher.query.len() {
                        searcher.query.push(c);
                    } else {
                        searcher.query.insert(current_cursor, c);
                    }
                    searcher.line_cursor += 1;
                    let count_results = searcher.count_results();
                    if count_results == 0 {
                        searcher.select_cursor = count_results;
                    } else if searcher.select_cursor >= count_results {
                        searcher.select_cursor = count_results - 1;
                    }
                    Ok(EventResult::Redraw)
                }
                Some(SearchAction::Backspace) => {
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
                Some(SearchAction::Delete) => {
                    let Some(ref mut searcher) = state.searcher else {
                        return Ok(EventResult::Error("No searcher available".into()));
                    };
                    searcher.query.clear();
                    searcher.line_cursor = 0;
                    Ok(EventResult::Redraw)
                }
                Some(SearchAction::Move(Direction::Left)) => {
                    let Some(ref mut searcher) = state.searcher else {
                        return Ok(EventResult::Error("No searcher available".into()));
                    };
                    if searcher.line_cursor > 0 {
                        searcher.line_cursor -= 1;
                    }
                    Ok(EventResult::Redraw)
                }
                Some(SearchAction::Move(Direction::Right)) => {
                    let Some(ref mut searcher) = state.searcher else {
                        return Ok(EventResult::Error("No searcher available".into()));
                    };
                    if searcher.line_cursor < searcher.query.len() {
                        searcher.line_cursor += 1;
                    }
                    Ok(EventResult::Redraw)
                }
                Some(SearchAction::Move(Direction::Up)) => {
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
                Some(SearchAction::Move(Direction::Down)) => {
                    let Some(ref mut searcher) = state.searcher else {
                        return Ok(EventResult::Error("No searcher available".into()));
                    };
                    let searcher_count = searcher.count_results();
                    if searcher_count > 0 && searcher.select_cursor < searcher_count - 1 {
                        searcher.select_cursor += 1;
                    }

                    Ok(EventResult::Redraw)
                }
                Some(SearchAction::Submit) => {
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
                    let selected_result = selected_result
                        .strip_prefix("/")
                        .unwrap_or(&selected_result);
                    root.expand_path(selected_result)?;
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
            },
            _ => Ok(EventResult::Continue),
        },
        _ => Ok(EventResult::Continue),
    }
}
