use std::cmp::{max, min};

use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::{error::AppError, ui::state::AppState};

use super::EventResult;

pub fn handle_normal_tree_event(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match (key_event.code, key_event.modifiers) {
                (KeyCode::Up, _) => {
                    if state.tree_view_cursor > 0 {
                        state.tree_view_cursor -= 1;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Char('u'), _) => {
                    state.tree_view_cursor = max(state.tree_view_cursor as isize - 10, 0) as usize;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char('g'), _) => {
                    state.tree_view_cursor = 0;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char('G'), _) => {
                    state.tree_view_cursor = state.treeview.len() - 1;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char('h'), _) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    if tree_item.node.borrow().expanded {
                        tree_item.node.borrow_mut().collapse();
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Char('l'), _) => {
                    if state.treeview[state.tree_view_cursor].load_more {
                        return Ok(EventResult::Continue);
                    }

                    let tree_item = &state.treeview[state.tree_view_cursor];
                    if !tree_item.node.borrow().expanded {
                        tree_item.node.borrow_mut().expand()?;
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Char('H'), _) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    if tree_item.node.borrow().expanded {
                        tree_item.node.borrow_mut().collapse();
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Char('L'), _) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    if !tree_item.node.borrow().expanded {
                        tree_item.node.borrow_mut().expand()?;
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Home, _) => {
                    state.tree_view_cursor = 0;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::End, _) => {
                    state.tree_view_cursor = state.treeview.len() - 1;
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char('j'), _) => {
                    if state.tree_view_cursor < state.treeview.len() - 1 {
                        state.tree_view_cursor += 1;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Down, _) => {
                    if state.tree_view_cursor < state.treeview.len() - 1 {
                        state.tree_view_cursor += 1;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Char('J'), _) => {
                    if state.tree_view_cursor < state.treeview.len() - 1 {
                        state.tree_view_cursor += 1;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Char('K'), _) => {
                    if state.tree_view_cursor > 0 {
                        state.tree_view_cursor -= 1;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Left, _) => {
                    if state.treeview[state.tree_view_cursor].load_more {
                        return Ok(EventResult::Continue);
                    }
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    if tree_item.node.borrow().expanded {
                        tree_item.node.borrow_mut().collapse();
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Right, _) => {
                    if state.treeview[state.tree_view_cursor].load_more {
                        return Ok(EventResult::Continue);
                    }
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    if !tree_item.node.borrow().expanded {
                        tree_item.node.borrow_mut().expand()?;
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                    state.tree_view_cursor =
                        min(state.tree_view_cursor + 10, state.treeview.len() - 1);
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char('k'), _) => {
                    if state.tree_view_cursor > 0 {
                        state.tree_view_cursor -= 1;
                    }
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Enter, _) => {
                    if state.treeview[state.tree_view_cursor].load_more {
                        let tree_item = &state.treeview[state.tree_view_cursor];
                        tree_item.node.borrow_mut().view_loaded += 50;
                        state.compute_tree_view();
                        return Ok(EventResult::Redraw);
                    }

                    let tree_item = &state.treeview[state.tree_view_cursor];
                    tree_item.node.borrow_mut().expand_toggle()?;
                    state.compute_tree_view();
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char('m'), _) => {
                    let Some((ds, sel)) = state.get_1d_selection() else {
                        return Ok(EventResult::Continue);
                    };
                    state.multi_chart.add_linspace_series(ds, sel);
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Char(' '), _) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    tree_item.node.borrow_mut().expand_toggle()?;
                    state.compute_tree_view();
                    Ok(EventResult::Redraw)
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
