use std::cmp::{max, min};

use ratatui::crossterm::event::{Event, KeyEventKind};

use crate::{error::AppError, h5f::read_projected_values_1d, ui::state::AppState};

use super::{
    keymap::{tree_action, TreeAction},
    EventResult,
};

pub fn handle_normal_tree_event(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match tree_action(&key_event) {
                Some(TreeAction::MoveUp(step)) => {
                    state.tree_view_cursor =
                        max(state.tree_view_cursor as isize - step as isize, 0) as usize;
                    Ok(EventResult::Redraw)
                }
                Some(TreeAction::MoveDown(step)) => {
                    state.tree_view_cursor =
                        min(state.tree_view_cursor + step, state.treeview.len() - 1);
                    Ok(EventResult::Redraw)
                }
                Some(TreeAction::MoveTop) => {
                    state.tree_view_cursor = 0;
                    Ok(EventResult::Redraw)
                }
                Some(TreeAction::MoveBottom) => {
                    state.tree_view_cursor = state.treeview.len() - 1;
                    Ok(EventResult::Redraw)
                }
                Some(TreeAction::Collapse) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    if tree_item.node.borrow().expanded {
                        tree_item.node.borrow_mut().collapse();
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                Some(TreeAction::Expand) => {
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
                Some(TreeAction::Toggle) => {
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
                Some(TreeAction::AddToMultiChart) => {
                    let Some((ds, meta, sel)) = state.get_1d_selection() else {
                        return Ok(EventResult::Continue);
                    };
                    if meta.is_compound_container() {
                        return Ok(EventResult::Continue);
                    }
                    if meta.is_compound_leaf() {
                        let Ok(data) = read_projected_values_1d::<f64>(&ds, &meta, sel) else {
                            return Ok(EventResult::Continue);
                        };
                        let points = data
                            .iter()
                            .enumerate()
                            .map(|(i, value)| (i as f64, *value))
                            .collect();
                        let key = meta
                            .virtual_path()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| ds.name());
                        state.multi_chart.add_points_series(key, points);
                    } else {
                        state.multi_chart.add_linspace_series(ds, sel);
                    }
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
