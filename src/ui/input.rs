use std::{cell::RefCell, rc::Rc};

use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind};

use crate::ui::app::{AppError, AppState};

pub enum EventResult {
    Quit,
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
                KeyCode::Char('q') => return Ok(EventResult::Quit),
                KeyCode::Char('?') => state.help = !state.help,
                KeyCode::Up => {
                    if state.tree_view_cursor > 0 {
                        state.tree_view_cursor -= 1;
                    }
                }
                KeyCode::Char('j') => {
                    if state.tree_view_cursor < state.treeview.len() - 1 {
                        state.tree_view_cursor += 1;
                    }
                }
                KeyCode::Down => {
                    if state.tree_view_cursor < state.treeview.len() - 1 {
                        state.tree_view_cursor += 1;
                    }
                }
                KeyCode::Char('k') => {
                    if state.tree_view_cursor > 0 {
                        state.tree_view_cursor -= 1;
                    }
                }
                KeyCode::Enter => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    tree_item.node.borrow_mut().expand_toggle().unwrap();
                    state.compute_tree_view();
                }
                KeyCode::Char(' ') => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    tree_item.node.borrow_mut().expand_toggle().unwrap();
                    state.compute_tree_view();
                }
                _ => {}
            },
            KeyEventKind::Repeat => {}
            KeyEventKind::Release => {}
        },
        _ => {} // Handle other events if needed
    }
    Ok(EventResult::Continue)
}
