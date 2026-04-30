use ratatui::crossterm::event::{Event, KeyEventKind};

use crate::{
    error::AppError,
    h5f::{format_attr_for_edit, SYSTEM_ATTRIBUTES},
    sprint_attributes::AttributeEditable,
    ui::{
        edit::perform_edit,
        state::{
            AppState, AppToast, AttributeEditRequest,
            AttributeViewSelection::{Name, Value},
            FixedStringOverflowChoice, FixedStringOverflowDialogState, Mode,
        },
    },
};

use super::{
    keymap::{attributes_action, AttributesAction, Direction},
    EventResult,
};

fn selected_attribute_edit_request(
    state: &mut AppState<'_>,
) -> Result<AttributeEditRequest, EventResult> {
    let mut node = state.treeview[state.tree_view_cursor].node.borrow_mut();

    let node_attributes_view_cursor = node.attributes_view_cursor.clone();
    let attributes = match node.read_attributes() {
        Ok(attributes) => attributes,
        Err(error) => {
            return Err(EventResult::Toast(
                AppToast::Error(format!("Failed to read attributes: {}", error)),
                true,
            ))
        }
    };
    let selected_rendered_attribute = attributes
        .rendered_attributes
        .get(node_attributes_view_cursor.attribute_index);
    let Some(attribute) = selected_rendered_attribute else {
        return Err(EventResult::Toast(
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
        return Err(EventResult::Toast(
            AppToast::Error(format!(
                "Editing metainfo-attribute '{}' is not allowed",
                attr_name
            )),
            true,
        ));
    }

    let (_, attr) = match attributes
        .attributes
        .iter()
        .find(|(name, _)| name == &attr_name)
    {
        Some(attr) => attr,
        None => {
            return Err(EventResult::Toast(
                AppToast::Error(format!("Attribute '{}' not found", attr_name)),
                true,
            ))
        }
    };

    if let Err(e) = attr.can_edit() {
        if let Value = node_attributes_view_cursor.attribute_view_selection {
            return Err(EventResult::Toast(
                AppToast::Error(format!(
                    "Attribute '{}' value cannot be edited: {}",
                    attr_name, e
                )),
                false,
            ));
        }
    }

    let content = match node_attributes_view_cursor.attribute_view_selection {
        Name => attr_name.clone(),
        Value => format_attr_for_edit(attr)
            .map_err(|error| EventResult::Toast(AppToast::Error(error.to_string()), false))?,
    };

    Ok(AttributeEditRequest {
        attr_name,
        content,
        selection: node_attributes_view_cursor.attribute_view_selection,
    })
}

pub fn apply_attribute_edit_request(
    state: &mut AppState<'_>,
    request: &AttributeEditRequest,
) -> Result<EventResult, AppError> {
    state.editing = true;
    let new_value = match perform_edit(state, request.content.clone()) {
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

    let mut selected_node = state.treeview[state.tree_view_cursor].node.borrow_mut();
    match request.selection {
        Name => {
            if let Err(e) = selected_node.update_attribute_name(&request.attr_name, &new_value) {
                return Ok(EventResult::Toast(AppToast::Error(e.to_string()), true));
            };
        }
        Value => {
            if let Err(e) = selected_node.update_attribute(&request.attr_name, new_value.clone()) {
                if let AppError::FixedStringOverflow(overflow) = &e {
                    drop(selected_node);
                    state.fixed_string_overflow_dialog = Some(FixedStringOverflowDialogState {
                        request: request.clone(),
                        new_value,
                        overflow: overflow.clone(),
                        selected_choice: FixedStringOverflowChoice::Cancel,
                        size_input: overflow.required_size.to_string(),
                    });
                    state.mode = Mode::FixedStringOverflowDialog;
                    return Ok(EventResult::Toast(AppToast::Empty, true));
                }
                if let AppError::EditWarning(warning) = &e {
                    return Ok(EventResult::Toast(
                        AppToast::Warning(warning.to_string()),
                        true,
                    ));
                }
                return Ok(EventResult::Toast(AppToast::Error(e.to_string()), true));
            }
        }
    }
    drop(selected_node);
    state.acknowledge_file_write();

    eprintln!("Attribute '{}' updated successfully", request.attr_name);
    let selected_node = state.treeview[state.tree_view_cursor].node.borrow();
    match selected_node.computed_attributes {
        Some(ref x) => {
            eprintln!("Computed attributes:");
            for (ref key, ref value, ref t) in x.rendered_attributes.iter() {
                eprintln!("  {} = {} ({})", key, value, t);
            }
        }
        None => eprintln!("No computed attributes"),
    };

    Ok(EventResult::Toast(
        AppToast::Info(format!(
            "Attribute '{}' updated successfully",
            request.attr_name
        )),
        true,
    ))
}

pub fn handle_normal_attributes(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match attributes_action(&key_event) {
                Some(AttributesAction::Move(Direction::Up)) => {
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
                Some(AttributesAction::Move(Direction::Down)) => {
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
                Some(AttributesAction::Move(Direction::Left)) => {
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
                Some(AttributesAction::Move(Direction::Right)) => {
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
                Some(AttributesAction::Edit) => {
                    let request = match selected_attribute_edit_request(state) {
                        Ok(request) => request,
                        Err(event_result) => return Ok(event_result),
                    };

                    if state.readonly {
                        return Ok(EventResult::Toast(
                            AppToast::Warning(
                                "Cannot edit in read-only mode; reopen with -w to modify the file"
                                    .to_string(),
                            ),
                            false,
                        ));
                    }

                    apply_attribute_edit_request(state, &request)
                }
                Some(AttributesAction::Copy) => {
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
