use std::cmp::{max, min};
use std::rc::Rc;

use ratatui::crossterm::event::{Event, KeyEventKind};

use crate::{error::AppError, ui::state::AppState};

use super::{
    execute_bound_command, execute_bound_lua_callback, execute_bound_script,
    keymap::{tree_action, BoundAction, EffectiveKeymaps, TreeAction},
    EventResult,
};

pub fn handle_normal_tree_event(
    state: &mut AppState<'_>,
    event: Event,
    keymaps: &EffectiveKeymaps,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match tree_action(&key_event, keymaps) {
                Some(BoundAction::Action(TreeAction::MoveUp(step))) => {
                    state.tree_view_cursor =
                        max(state.tree_view_cursor as isize - step as isize, 0) as usize;
                    Ok(EventResult::Redraw)
                }
                Some(BoundAction::Action(TreeAction::MoveDown(step))) => {
                    state.tree_view_cursor =
                        min(state.tree_view_cursor + step, state.treeview.len() - 1);
                    Ok(EventResult::Redraw)
                }
                Some(BoundAction::Action(TreeAction::MoveTop)) => {
                    state.tree_view_cursor = 0;
                    Ok(EventResult::Redraw)
                }
                Some(BoundAction::Action(TreeAction::MoveBottom)) => {
                    state.tree_view_cursor = state.treeview.len() - 1;
                    Ok(EventResult::Redraw)
                }
                Some(BoundAction::Action(TreeAction::Collapse)) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    if tree_item.load_more || tree_item.node.borrow().expanded {
                        let node = tree_item.node.clone();
                        node.borrow_mut().collapse();
                        state.tree_view_cursor = state
                            .treeview
                            .iter()
                            .position(|item| !item.load_more && Rc::ptr_eq(&item.node, &node))
                            .unwrap_or(state.tree_view_cursor);
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    } else if let Some(parent) = tree_item.parent.clone() {
                        state.tree_view_cursor = state
                            .treeview
                            .iter()
                            .position(|item| !item.load_more && Rc::ptr_eq(&item.node, &parent))
                            .unwrap_or(state.tree_view_cursor);
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                Some(BoundAction::Action(TreeAction::Expand)) => {
                    if state.treeview[state.tree_view_cursor].load_more {
                        let tree_item = &state.treeview[state.tree_view_cursor];
                        tree_item.node.borrow_mut().view_loaded += 50;
                        state.compute_tree_view();
                        return Ok(EventResult::Redraw);
                    }

                    let tree_item = &state.treeview[state.tree_view_cursor];
                    if !tree_item.node.borrow().is_expandable() {
                        return Ok(EventResult::Continue);
                    }
                    let node = tree_item.node.clone();
                    if !node.borrow().expanded {
                        let expand_result = node.borrow_mut().expand();
                        if let Err(error) = expand_result {
                            state.compute_tree_view();
                            return Err(error.into());
                        }
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    } else {
                        let node = tree_item.node.clone();
                        if let Some(child) = state.treeview.get(state.tree_view_cursor + 1) {
                            if child
                                .parent
                                .as_ref()
                                .is_some_and(|parent| Rc::ptr_eq(parent, &node))
                            {
                                state.tree_view_cursor += 1;
                                return Ok(EventResult::Redraw);
                            }
                        }
                        Ok(EventResult::Continue)
                    }
                }
                Some(BoundAction::Action(TreeAction::Toggle)) => {
                    if state.treeview[state.tree_view_cursor].load_more {
                        let tree_item = &state.treeview[state.tree_view_cursor];
                        tree_item.node.borrow_mut().view_loaded += 50;
                        state.compute_tree_view();
                        return Ok(EventResult::Redraw);
                    }

                    let tree_item = &state.treeview[state.tree_view_cursor];
                    if tree_item.node.borrow().is_expandable() {
                        let node = tree_item.node.clone();
                        let expand_result = node.borrow_mut().expand_toggle();
                        if let Err(error) = expand_result {
                            state.compute_tree_view();
                            return Err(error.into());
                        }
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                Some(BoundAction::Action(TreeAction::AddToMultiChart)) => {
                    let Some(captured) = state.capture_multichart_item()? else {
                        return Ok(EventResult::Continue);
                    };
                    state
                        .multi_chart
                        .queue_loaded_item(captured)
                        .map_err(crate::error::AppError::InvalidCommand)?;
                    state.compute_tree_view();
                    Ok(EventResult::Redraw)
                }
                Some(BoundAction::Command(command)) => execute_bound_command(state, &command),
                Some(BoundAction::Script(script)) => {
                    execute_bound_script(state, &script, "keybinding script")
                }
                Some(BoundAction::LuaCallback(callback_id)) => {
                    execute_bound_lua_callback(state, &callback_id)
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
