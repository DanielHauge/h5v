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

    // if let (KeyEventKind::Press, KeyCode::Char('q')) = (key.kind, key.code) {
    //     return Ok(EventResult::Quit);
    // }
    // if let (KeyEventKind::Press, KeyCode::Char('?')) = (key.kind, key.code) {
    //     state.help = !state.help;
    // }
    // if let (KeyEventKind::Press, KeyCode::Up) = (key.kind, key.code) {
    //     if state.tree_view_cursor > 0 {
    //         state.tree_view_cursor -= 1;
    //     }
    // }
    // if let (KeyEventKind::Press, KeyCode::Char('j')) = (key.kind, key.code) {
    //     if state.tree_view_cursor < state.treeview.len() - 1 {
    //         state.tree_view_cursor += 1;
    //     }
    // }
    // if let (KeyEventKind::Press, KeyCode::Down) = (key.kind, key.code) {
    //     if state.tree_view_cursor < state.treeview.len() - 1 {
    //         state.tree_view_cursor += 1;
    //     }
    // }
    // if let (KeyEventKind::Press, KeyCode::Char('k')) = (key.kind, key.code) {
    //     if state.tree_view_cursor > 0 {
    //         state.tree_view_cursor -= 1;
    //     }
    // }
    // if let (KeyEventKind::Press, KeyCode::Enter) = (key.kind, key.code) {
    //     let tree_item = &mut state.treeview[state.tree_view_cursor];
    //     tree_item.node.borrow_mut().expand_toggle().unwrap();
    //     state.compute_tree_view();
    // }
    // if let (KeyEventKind::Press, KeyCode::Char(' ')) = (key.kind, key.code) {
    //     let tree_item = &mut state.treeview[state.tree_view_cursor];
    //     tree_item.node.borrow_mut().expand_toggle().unwrap();
    //     state.compute_tree_view();
    // }
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
