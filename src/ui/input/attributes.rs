use ratatui::crossterm::event::{Event, KeyEventKind};

use crate::{
    error::AppError,
    h5f::{format_attr_for_edit, AttributeCreateType, MetadataRowKind},
    ui::{
        attributes::navigate_metadata_grid,
        edit::perform_edit,
        state::{
            AppState, AppToast, AttributeCreateDialogState, AttributeCreateField,
            AttributeDeleteDialogState, AttributeEditRequest,
            AttributeViewSelection::{Name, Value},
            FixedStringOverflowChoice, FixedStringOverflowDialogState, Mode,
        },
    },
};

use super::{
    execute_bound_command, execute_bound_lua_callback, execute_bound_script,
    keymap::{attributes_action, AttributesAction, BoundAction, Direction, EffectiveKeymaps},
    EventResult,
};

mod references;
mod selection;

use references::navigate_reference_attribute_value;
use selection::{selected_attribute_edit_request, selected_custom_attribute_name};

#[cfg(test)]
use references::select_dataset_region_axes;

pub fn apply_attribute_edit_request(
    state: &mut AppState<'_>,
    request: &AttributeEditRequest,
) -> Result<EventResult, AppError> {
    state.editing = true;
    let new_value = match perform_edit(
        state,
        request.content.clone(),
        Some(&request.edit_name_hint),
    ) {
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
            for row in &x.rendered_rows {
                eprintln!(
                    "  {} = {} ({})",
                    row.name_line, row.value_line, row.type_line
                );
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
    keymaps: &EffectiveKeymaps,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match attributes_action(&key_event, keymaps) {
                Some(BoundAction::Action(AttributesAction::Move(Direction::Up, amount))) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    let mut node = tree_item.node.borrow_mut();
                    let Some(current_index) = node.normalize_attribute_selection()? else {
                        return Ok(EventResult::Continue);
                    };
                    let outer_width = state
                        .ui_layout
                        .attributes
                        .as_ref()
                        .map(|hitbox| hitbox.outer.width)
                        .unwrap_or(0);
                    let selection = node.attributes_view_cursor.attribute_view_selection;
                    let destination = {
                        let attributes = node.read_attributes()?;
                        let mut destination = None;
                        for _ in 0..amount {
                            let (row_index, next_selection) =
                                destination.unwrap_or((current_index, selection));
                            destination = navigate_metadata_grid(
                                attributes,
                                outer_width,
                                row_index,
                                next_selection,
                                Direction::Up,
                            );
                            if destination.is_none() {
                                break;
                            }
                        }
                        destination
                    };
                    if let Some((new_index, new_selection)) = destination {
                        node.attributes_view_cursor.attribute_index = new_index;
                        node.attributes_view_cursor.attribute_view_selection = new_selection;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                Some(BoundAction::Action(AttributesAction::Move(Direction::Down, amount))) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    let mut node = tree_item.node.borrow_mut();
                    let Some(current_index) = node.normalize_attribute_selection()? else {
                        return Ok(EventResult::Continue);
                    };
                    let outer_width = state
                        .ui_layout
                        .attributes
                        .as_ref()
                        .map(|hitbox| hitbox.outer.width)
                        .unwrap_or(0);
                    let selection = node.attributes_view_cursor.attribute_view_selection;
                    let destination = {
                        let attributes = node.read_attributes()?;
                        let mut destination = None;
                        for _ in 0..amount {
                            let (row_index, next_selection) =
                                destination.unwrap_or((current_index, selection));
                            destination = navigate_metadata_grid(
                                attributes,
                                outer_width,
                                row_index,
                                next_selection,
                                Direction::Down,
                            );
                            if destination.is_none() {
                                break;
                            }
                        }
                        destination
                    };
                    if let Some((new_index, new_selection)) = destination {
                        node.attributes_view_cursor.attribute_index = new_index;
                        node.attributes_view_cursor.attribute_view_selection = new_selection;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                Some(BoundAction::Action(AttributesAction::Move(Direction::Left, _))) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    let mut node = tree_item.node.borrow_mut();
                    let Some(current_index) = node.normalize_attribute_selection()? else {
                        return Ok(EventResult::Continue);
                    };
                    let outer_width = state
                        .ui_layout
                        .attributes
                        .as_ref()
                        .map(|hitbox| hitbox.outer.width)
                        .unwrap_or(0);
                    let selection = node.attributes_view_cursor.attribute_view_selection;
                    let destination = {
                        let attributes = node.read_attributes()?;
                        navigate_metadata_grid(
                            attributes,
                            outer_width,
                            current_index,
                            selection,
                            Direction::Left,
                        )
                    };
                    if let Some((new_index, new_selection)) = destination {
                        node.attributes_view_cursor.attribute_index = new_index;
                        node.attributes_view_cursor.attribute_view_selection = new_selection;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                Some(BoundAction::Action(AttributesAction::Move(Direction::Right, _))) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    let mut node = tree_item.node.borrow_mut();
                    let Some(current_index) = node.normalize_attribute_selection()? else {
                        return Ok(EventResult::Continue);
                    };
                    let outer_width = state
                        .ui_layout
                        .attributes
                        .as_ref()
                        .map(|hitbox| hitbox.outer.width)
                        .unwrap_or(0);
                    let selection = node.attributes_view_cursor.attribute_view_selection;
                    let destination = {
                        let attributes = node.read_attributes()?;
                        navigate_metadata_grid(
                            attributes,
                            outer_width,
                            current_index,
                            selection,
                            Direction::Right,
                        )
                    };
                    if let Some((new_index, new_selection)) = destination {
                        node.attributes_view_cursor.attribute_index = new_index;
                        node.attributes_view_cursor.attribute_view_selection = new_selection;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                Some(BoundAction::Action(AttributesAction::Edit)) => {
                    match navigate_reference_attribute_value(state) {
                        Ok(Some(event_result)) => return Ok(event_result),
                        Ok(None) => {}
                        Err(event_result) => return Ok(event_result),
                    }
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
                Some(BoundAction::Action(AttributesAction::Copy)) => {
                    let mut node = state.treeview[state.tree_view_cursor].node.borrow_mut();
                    let Some(row_index) = node.normalize_attribute_selection()? else {
                        return Ok(EventResult::Toast(
                            AppToast::Error("No metadata selected to copy".to_string()),
                            true,
                        ));
                    };
                    let node_attributes_view_cursor = node.attributes_view_cursor.clone();
                    let attributes = node.read_attributes()?;
                    let selected_rendered_attribute = attributes.row(row_index);
                    let copy_request = match node_attributes_view_cursor.attribute_view_selection {
                        Name => selected_rendered_attribute
                            .and_then(|row| row.key.clone().map(|key| (key, "metadata name")))
                            .ok_or_else(|| {
                                AppError::ClipboardError("No metadata selected to copy".to_string())
                            }),
                        Value => selected_rendered_attribute
                            .map(|row| {
                                let value_string = if matches!(row.kind, MetadataRowKind::Attribute)
                                {
                                    let attr_name = row
                                        .key
                                        .clone()
                                        .unwrap_or_else(|| "selected row".to_string());
                                    if let Some((_, attr)) = attributes
                                        .attributes
                                        .iter()
                                        .find(|(name, _)| name == &attr_name)
                                    {
                                        format_attr_for_edit(attr)?
                                    } else {
                                        row.value_line.to_string().trim_end().to_string()
                                    }
                                } else {
                                    row.value_line.to_string().trim_end().to_string()
                                };
                                Ok((value_string, "metadata value"))
                            })
                            .unwrap_or_else(|| {
                                Err(AppError::ClipboardError(
                                    "No metadata selected to copy".to_string(),
                                ))
                            }),
                    }?;
                    drop(node);

                    match state.set_clipboard_text(copy_request.0) {
                        Ok(()) => Ok(EventResult::Copying),
                        Err(error) => Ok(EventResult::Toast(
                            AppToast::Warning(format!(
                                "Failed to copy {} to clipboard: {error}",
                                copy_request.1
                            )),
                            false,
                        )),
                    }
                }
                Some(BoundAction::Action(AttributesAction::Create)) => {
                    if state.readonly {
                        return Ok(EventResult::Toast(
                            AppToast::Warning(
                                "Cannot edit in read-only mode; reopen with -w to modify the file"
                                    .to_string(),
                            ),
                            false,
                        ));
                    }
                    state.attribute_create_dialog = Some(AttributeCreateDialogState {
                        name: String::new(),
                        name_cursor: 0,
                        attr_type: AttributeCreateType::String,
                        value: String::new(),
                        value_cursor: 0,
                        active_field: AttributeCreateField::Name,
                    });
                    state.mode = Mode::AttributeCreateDialog;
                    Ok(EventResult::Redraw)
                }
                Some(BoundAction::Action(AttributesAction::Delete)) => {
                    if state.readonly {
                        return Ok(EventResult::Toast(
                            AppToast::Warning(
                                "Cannot edit in read-only mode; reopen with -w to modify the file"
                                    .to_string(),
                            ),
                            false,
                        ));
                    }
                    let attr_name = match selected_custom_attribute_name(state) {
                        Ok(attr_name) => attr_name,
                        Err(event_result) => return Ok(event_result),
                    };
                    state.attribute_delete_dialog = Some(AttributeDeleteDialogState { attr_name });
                    state.mode = Mode::AttributeDeleteDialog;
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

#[cfg(test)]
mod tests {
    use super::select_dataset_region_axes;

    #[test]
    fn region_axes_prefer_varying_dimensions() {
        let (row_dim, col_dim, selected_dim) =
            select_dataset_region_axes(&[4, 5, 6], &[1, 2, 0], &[3, 4, 0]);
        assert_eq!(row_dim, 0);
        assert_eq!(col_dim, Some(1));
        assert_eq!(selected_dim, 2);
    }

    #[test]
    fn region_axes_fall_back_to_dataset_shape_for_single_point() {
        let (row_dim, col_dim, selected_dim) =
            select_dataset_region_axes(&[3, 4, 5], &[2, 1, 4], &[2, 1, 4]);
        assert_eq!(row_dim, 0);
        assert_eq!(col_dim, Some(1));
        assert_eq!(selected_dim, 2);
    }

    #[test]
    fn region_axes_handle_one_dimensional_datasets() {
        let (row_dim, col_dim, selected_dim) = select_dataset_region_axes(&[8], &[3], &[5]);
        assert_eq!(row_dim, 0);
        assert_eq!(col_dim, None);
        assert_eq!(selected_dim, 0);
    }
}
