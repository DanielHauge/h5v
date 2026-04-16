use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind};

use crate::{
    error::AppError,
    h5f::SYSTEM_ATTRIBUTES,
    ui::{
        edit::perform_edit,
        state::{
            AppState, AppToast,
            AttributeViewSelection::{Name, Value},
        },
    },
};

use super::EventResult;

pub fn handle_normal_attributes(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match (key_event.code, key_event.modifiers) {
                (KeyCode::Up, _) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    let mut node = tree_item.node.borrow_mut();
                    let attributes_count = node.read_attributes()?.rendered_attributes.len();
                    if node.attributes_view_cursor.attribute_index > 0 {
                        if node.attributes_view_cursor.attribute_index >= attributes_count {
                            node.attributes_view_cursor.attribute_index = attributes_count - 2;
                        } else {
                            node.attributes_view_cursor.attribute_index -= 1;
                        }
                        Ok(EventResult::Redraw)
                    } else {
                        node.attributes_view_cursor.attribute_index = 0;
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Down, _) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    let mut node = tree_item.node.borrow_mut();
                    let attributes_count = node.read_attributes()?.rendered_attributes.len();

                    if node.attributes_view_cursor.attribute_index < attributes_count - 1 {
                        node.attributes_view_cursor.attribute_index += 1;

                        Ok(EventResult::Redraw)
                    } else {
                        node.attributes_view_cursor.attribute_index = attributes_count - 1;
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Left, _) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    let mut node = tree_item.node.borrow_mut();
                    match node.attributes_view_cursor.attribute_view_selection {
                        Name => {}
                        Value => {
                            node.attributes_view_cursor.attribute_view_selection = Name;
                        }
                    }
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Right, _) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    let mut node = tree_item.node.borrow_mut();
                    match node.attributes_view_cursor.attribute_view_selection {
                        Name => {
                            node.attributes_view_cursor.attribute_view_selection = Value;
                        }
                        Value => {}
                    }
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Enter, _) | (KeyCode::Char('e'), _) => {
                    state.editing = true;
                    let mut node = state.treeview[state.tree_view_cursor].node.borrow_mut();
                    let node_attributes_view_cursor = node.attributes_view_cursor.clone();
                    let attributes = node.read_attributes()?;
                    let selected_rendered_attribute = attributes
                        .rendered_attributes
                        .get(node_attributes_view_cursor.attribute_index);
                    let Some(attribute) = selected_rendered_attribute else {
                        state.editing = false;
                        return Ok(EventResult::Toast(
                            AppToast::Error("No attribute selected".to_string()),
                            true,
                        ));
                    };
                    let attr_name = attribute
                        .0
                        .to_string()
                        .trim_end_matches('=')
                        .trim_end_matches('─')
                        .trim_end()
                        .to_string();
                    if SYSTEM_ATTRIBUTES.contains(&attr_name.as_str()) {
                        state.editing = false;
                        return Ok(EventResult::Toast(
                            AppToast::Error(format!(
                                "Editing metainfo-attribute '{}' is not allowed",
                                attr_name
                            )),
                            true,
                        ));
                    }
                    let content = match node_attributes_view_cursor.attribute_view_selection {
                        Name => attr_name.clone(),
                        Value => attribute
                            .1
                            .to_string()
                            .trim_start_matches("\"")
                            .trim_end_matches("\"")
                            .to_string(),
                    };
                    drop(node);

                    let new_value = match perform_edit(state, content) {
                        Ok(new_value) => new_value,
                        Err(e) => {
                            state.editing = false;
                            return Ok(EventResult::Toast(
                                AppToast::Error(format!("Failed to edit attribute: {}", e)),
                                true,
                            ));
                        }
                    };
                    state.editing = false;
                    let mut selected_node =
                        state.treeview[state.tree_view_cursor].node.borrow_mut();
                    match node_attributes_view_cursor.attribute_view_selection {
                        Name => {
                            if let Err(e) =
                                selected_node.update_attribute_name(&attr_name, &new_value)
                            {
                                return Ok(EventResult::Toast(
                                    AppToast::Error(e.to_string()),
                                    true,
                                ));
                            };
                        }
                        Value => {
                            if let Err(e) = selected_node.update_attribute(&attr_name, new_value) {
                                return Ok(EventResult::Toast(
                                    AppToast::Error(e.to_string()),
                                    true,
                                ));
                            }
                        }
                    }

                    selected_node.recompute_attributes()?;

                    Ok(EventResult::Toast(
                        AppToast::Info(format!("Attribute '{}' updated successfully", attr_name)),
                        true,
                    ))
                }
                (KeyCode::Char('y'), _) => {
                    let mut node = state.treeview[state.tree_view_cursor].node.borrow_mut();
                    let node_attributes_view_cursor = node.attributes_view_cursor.clone();
                    let attributes = node.read_attributes()?;
                    let selected_rendered_attribute = attributes
                        .rendered_attributes
                        .get(node_attributes_view_cursor.attribute_index);

                    match node_attributes_view_cursor.attribute_view_selection {
                        Name => {
                            if let Some(attribute) = selected_rendered_attribute {
                                let attr_name = attribute.0.to_string();
                                let name = attr_name
                                    .trim_end_matches('=')
                                    .trim_end_matches('─')
                                    .trim_end()
                                    .to_string();

                                match state.clipboard.set_text(name.to_string()) {
                                    Ok(()) => Ok(EventResult::Copying),
                                    Err(e) => Err(AppError::ClipboardError(format!(
                                        "Failed to copy attribute name to clipboard: {}",
                                        e
                                    ))),
                                }
                            } else {
                                Err(AppError::ClipboardError(
                                    "No attribute selected to copy".to_string(),
                                ))
                            }
                        }
                        Value => {
                            if let Some(attribute) = selected_rendered_attribute {
                                let value_string = attribute.1.to_string();
                                match state.clipboard.set_text(value_string) {
                                    Ok(()) => Ok(EventResult::Copying),
                                    Err(e) => Err(AppError::ClipboardError(format!(
                                        "Failed to copy attribute value to clipboard: {}",
                                        e
                                    ))),
                                }
                            } else {
                                Err(AppError::ClipboardError(
                                    "No attribute selected to copy".to_string(),
                                ))
                            }
                        }
                    }
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
