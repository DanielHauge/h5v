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
                    let mut current_node = tree_item.node.borrow_mut();
                    let attributes_count =
                        current_node.read_attributes()?.rendered_attributes.len();
                    if state.attributes_view_cursor.attribute_index > 0 {
                        if state.attributes_view_cursor.attribute_index >= attributes_count {
                            state.attributes_view_cursor.attribute_index = attributes_count - 2;
                        } else {
                            state.attributes_view_cursor.attribute_index -= 1;
                        }
                        Ok(EventResult::Redraw)
                    } else {
                        state.attributes_view_cursor.attribute_index = 0;
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Down, _) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    let mut current_node = tree_item.node.borrow_mut();
                    let attributes_count =
                        current_node.read_attributes()?.rendered_attributes.len();

                    if state.attributes_view_cursor.attribute_index < attributes_count - 1 {
                        state.attributes_view_cursor.attribute_index += 1;
                        Ok(EventResult::Redraw)
                    } else {
                        state.attributes_view_cursor.attribute_index = attributes_count - 1;
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Left, _) => {
                    match state.attributes_view_cursor.attribute_view_selection {
                        Name => {}
                        Value => {
                            state.attributes_view_cursor.attribute_view_selection = Name;
                        }
                    }
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Right, _) => {
                    match state.attributes_view_cursor.attribute_view_selection {
                        Name => {
                            state.attributes_view_cursor.attribute_view_selection = Value;
                        }
                        Value => {}
                    }
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Enter, _) | (KeyCode::Char('e'), _) => {
                    state.editing = true;
                    let mut selected_node =
                        state.treeview[state.tree_view_cursor].node.borrow_mut();
                    let attributes = selected_node.read_attributes()?;
                    let selected_rendered_attribute = attributes
                        .rendered_attributes
                        .get(state.attributes_view_cursor.attribute_index);
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
                    let content = match state.attributes_view_cursor.attribute_view_selection {
                        Name => attr_name.clone(),
                        Value => attribute
                            .1
                            .to_string()
                            .trim_start_matches("\"")
                            .trim_end_matches("\"")
                            .to_string(),
                    };
                    drop(selected_node);

                    let new_value = perform_edit(state, content)?;
                    let mut selected_node =
                        state.treeview[state.tree_view_cursor].node.borrow_mut();
                    match state.attributes_view_cursor.attribute_view_selection {
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
                    state.file.flush()?;
                    state.editing = false;

                    Ok(EventResult::Toast(
                        AppToast::Info(format!("Attribute '{}' updated successfully", attr_name)),
                        true,
                    ))
                }
                (KeyCode::Char('y'), _) => {
                    let mut selected_node =
                        state.treeview[state.tree_view_cursor].node.borrow_mut();
                    let attributes = selected_node.read_attributes()?;
                    let selected_rendered_attribute = attributes
                        .rendered_attributes
                        .get(state.attributes_view_cursor.attribute_index);

                    match state.attributes_view_cursor.attribute_view_selection {
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
