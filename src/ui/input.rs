use std::{
    cell::RefCell,
    cmp::{max, min},
    rc::Rc,
};

use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind};

use crate::ui::app::{AppError, AppState};

pub enum EventResult {
    Quit,
    Redraw,
    Continue,
}

pub fn handle_event<'a>(
    state: &Rc<RefCell<AppState<'a>>>,
    event: Event,
) -> Result<EventResult, AppError> {
    let mut state = state.borrow_mut();

    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match key_event.code {
                KeyCode::Char('q') => Ok(EventResult::Quit),
                KeyCode::Char('?') => {
                    state.help = !state.help;
                    Ok(EventResult::Redraw)
                }
                KeyCode::Up => {
                    if state.tree_view_cursor > 0 {
                        state.tree_view_cursor -= 1;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                KeyCode::PageUp => {
                    state.tree_view_cursor = max(state.tree_view_cursor as isize - 10, 0) as usize;
                    Ok(EventResult::Redraw)
                }
                KeyCode::Char('j') => {
                    if state.tree_view_cursor < state.treeview.len() - 1 {
                        state.tree_view_cursor += 1;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                KeyCode::Down => {
                    if state.tree_view_cursor < state.treeview.len() - 1 {
                        state.tree_view_cursor += 1;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                KeyCode::Left => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    if tree_item.node.borrow().expanded {
                        tree_item.node.borrow_mut().collapse().unwrap();
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                KeyCode::Right => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    if !tree_item.node.borrow().expanded {
                        tree_item.node.borrow_mut().expand().unwrap();
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                KeyCode::PageDown => {
                    state.tree_view_cursor =
                        min(state.tree_view_cursor + 10, state.treeview.len() - 1);
                    Ok(EventResult::Redraw)
                }
                KeyCode::Char('k') => {
                    if state.tree_view_cursor > 0 {
                        state.tree_view_cursor -= 1;
                    }
                    Ok(EventResult::Redraw)
                }
                KeyCode::Enter => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    tree_item.node.borrow_mut().expand_toggle().unwrap();
                    state.compute_tree_view();
                    Ok(EventResult::Redraw)
                }
                KeyCode::Char(' ') => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    tree_item.node.borrow_mut().expand_toggle().unwrap();
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
