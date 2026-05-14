use std::path::Path;

use std::{ffi::CStr, os::raw::c_char, ptr};

use hdf5_metno::{types::Reference, Attribute, ObjectReference1, ReferencedObject};
use hdf5_metno_sys::{
    h5p::H5P_DEFAULT,
    h5r::{
        H5R_ref_t, H5Rget_attr_name, H5Rget_obj_name, H5Rget_type, H5R_ATTR, H5R_DATASET_REGION2,
        H5R_OBJECT2,
    },
};
use ratatui::crossterm::event::{Event, KeyEventKind};

use crate::{
    error::AppError,
    h5f::{
        format_attr_for_edit, read_attr_memory_bytes, AttributeCreateType, HasPath, MetadataRowKind,
    },
    ui::{
        attributes::navigate_metadata_grid,
        edit::perform_edit,
        render::{attribute_type_descriptor, AttributeEditable},
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

struct ReferenceNavigationTarget {
    path: String,
    attr_name: Option<String>,
}

fn selected_metadata_row(
    state: &mut AppState<'_>,
) -> Result<
    (
        crate::h5f::RenderedAttributeRow,
        crate::ui::state::AttributeViewSelection,
    ),
    EventResult,
> {
    let mut node = state.treeview[state.tree_view_cursor].node.borrow_mut();
    let selection = node.attributes_view_cursor.attribute_view_selection.clone();
    let Some(row_index) = (match node.normalize_attribute_selection() {
        Ok(index) => index,
        Err(error) => {
            return Err(EventResult::Toast(
                AppToast::Error(format!("Failed to read attributes: {}", error)),
                true,
            ))
        }
    }) else {
        return Err(EventResult::Toast(
            AppToast::Error("No metadata selected".to_string()),
            true,
        ));
    };
    let row = match node.read_attributes() {
        Ok(attributes) => attributes.row(row_index).cloned(),
        Err(error) => {
            return Err(EventResult::Toast(
                AppToast::Error(format!("Failed to read attributes: {}", error)),
                true,
            ))
        }
    };
    let Some(row) = row else {
        return Err(EventResult::Toast(
            AppToast::Error("No metadata selected".to_string()),
            true,
        ));
    };
    Ok((row, selection))
}

fn selected_attribute(
    state: &mut AppState<'_>,
) -> Result<(String, Attribute, crate::ui::state::AttributeViewSelection), EventResult> {
    let (row, selection) = selected_metadata_row(state)?;
    if !matches!(row.kind, MetadataRowKind::Attribute) {
        let row_name = row.key.unwrap_or_else(|| "selected row".to_string());
        return Err(EventResult::Toast(
            AppToast::Warning(format!(
                "'{}' is a built-in h5v property and has no editable HDF5 attribute value",
                row_name
            )),
            false,
        ));
    }
    let attr_name = row.key.unwrap_or_else(|| "selected row".to_string());
    let mut node = state.treeview[state.tree_view_cursor].node.borrow_mut();
    let attributes = match node.read_attributes() {
        Ok(attributes) => attributes,
        Err(error) => {
            return Err(EventResult::Toast(
                AppToast::Error(format!("Failed to read attributes: {}", error)),
                true,
            ))
        }
    };
    let Some((_, attr)) = attributes
        .attributes
        .iter()
        .find(|(name, _)| name == &attr_name)
    else {
        return Err(EventResult::Toast(
            AppToast::Error(format!("Attribute '{}' not found", attr_name)),
            true,
        ));
    };
    Ok((attr_name, attr.clone(), selection))
}

fn selected_custom_attribute_name(state: &mut AppState<'_>) -> Result<String, EventResult> {
    let (row, _) = selected_metadata_row(state)?;
    if !matches!(row.kind, MetadataRowKind::Attribute) {
        let attr_name = row.key.unwrap_or_else(|| "selected row".to_string());
        return Err(EventResult::Toast(
            AppToast::Warning(format!(
                "'{}' is a built-in h5v property and cannot be modified",
                attr_name
            )),
            false,
        ));
    }
    row.key.ok_or_else(|| {
        EventResult::Toast(AppToast::Error("No attribute selected".to_string()), true)
    })
}

fn read_hdf5_name(
    reader: impl Fn(*mut c_char, usize) -> isize,
    context: &str,
) -> Result<String, AppError> {
    let len = reader(ptr::null_mut(), 0);
    if len < 0 {
        return Err(AppError::EditError(format!("Failed to read {context}")));
    }
    let mut buffer = vec![0 as c_char; len as usize + 1];
    let written = reader(buffer.as_mut_ptr(), buffer.len());
    if written < 0 {
        return Err(AppError::EditError(format!("Failed to read {context}")));
    }
    Ok(unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned())
}

fn referenced_object_path(object: ReferencedObject) -> Result<String, AppError> {
    match object {
        ReferencedObject::Group(group) => Ok(group.name()),
        ReferencedObject::Dataset(dataset) => Ok(dataset.name()),
        ReferencedObject::Datatype(datatype) => datatype
            .as_location()
            .map(|location| location.name())
            .map_err(AppError::from),
    }
}

fn resolve_std_reference_target(attr: &Attribute) -> Result<ReferenceNavigationTarget, AppError> {
    let bytes = read_attr_memory_bytes(attr)?;
    if bytes.len() != std::mem::size_of::<H5R_ref_t>() {
        return Err(AppError::EditError(
            "Reference navigation requires a single target".to_string(),
        ));
    }

    let reference = bytes.as_ptr().cast::<H5R_ref_t>();
    match unsafe { H5Rget_type(reference) } {
        H5R_OBJECT2 | H5R_DATASET_REGION2 => Ok(ReferenceNavigationTarget {
            path: read_hdf5_name(
                |name, size| unsafe {
                    H5Rget_obj_name(reference, H5P_DEFAULT, name, size) as isize
                },
                "reference target path",
            )?,
            attr_name: None,
        }),
        H5R_ATTR => Ok(ReferenceNavigationTarget {
            path: read_hdf5_name(
                |name, size| unsafe {
                    H5Rget_obj_name(reference, H5P_DEFAULT, name, size) as isize
                },
                "attribute owner path",
            )?,
            attr_name: Some(read_hdf5_name(
                |name, size| unsafe { H5Rget_attr_name(reference, name, size) as isize },
                "attribute target name",
            )?),
        }),
        _ => Err(AppError::EditError(
            "Unsupported reference target type".to_string(),
        )),
    }
}

fn navigate_reference_attribute_value(
    state: &mut AppState<'_>,
) -> Result<Option<EventResult>, EventResult> {
    let (row, selection) = selected_metadata_row(state)?;
    if !matches!(row.kind, MetadataRowKind::Attribute) || !matches!(selection, Value) {
        return Ok(None);
    }
    let attr_name = row
        .key
        .clone()
        .unwrap_or_else(|| "selected row".to_string());
    let (_, attr, _) = selected_attribute(state)?;

    let type_desc = match attribute_type_descriptor(&attr) {
        Ok(type_desc) => type_desc,
        Err(error) if error.to_string() == "Unsupported datatype class" => return Ok(None),
        Err(error) => {
            return Err(EventResult::Toast(
                AppToast::Error(error.to_string()),
                false,
            ))
        }
    };
    let target = match type_desc {
        hdf5_metno::types::TypeDescriptor::Reference(Reference::Object) => {
            if attr.size() != 1 {
                return Ok(Some(EventResult::Toast(
                    AppToast::Warning(format!(
                        "Attribute '{}' contains multiple references; navigation needs a single target",
                        attr_name
                    )),
                    false,
                )));
            }
            let reference = if attr.is_scalar() {
                attr.read_scalar::<ObjectReference1>().map_err(|error| {
                    EventResult::Toast(AppToast::Error(error.to_string()), false)
                })?
            } else {
                attr.read_1d::<ObjectReference1>()
                    .map_err(|error| EventResult::Toast(AppToast::Error(error.to_string()), false))?
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        EventResult::Toast(
                            AppToast::Error(
                                "Reference attribute unexpectedly contained no values".to_string(),
                            ),
                            false,
                        )
                    })?
            };
            let file = attr
                .file()
                .map_err(|error| EventResult::Toast(AppToast::Error(error.to_string()), false))?;
            let object = file
                .dereference(&reference)
                .map_err(|error| EventResult::Toast(AppToast::Error(error.to_string()), false))?;
            Some(ReferenceNavigationTarget {
                path: referenced_object_path(object).map_err(|error| {
                    EventResult::Toast(AppToast::Error(error.to_string()), false)
                })?,
                attr_name: None,
            })
        }
        hdf5_metno::types::TypeDescriptor::Reference(Reference::Std) => {
            if attr.size() != 1 {
                return Ok(Some(EventResult::Toast(
                    AppToast::Warning(format!(
                        "Attribute '{}' contains multiple references; navigation needs a single target",
                        attr_name
                    )),
                    false,
                )));
            }
            Some(
                resolve_std_reference_target(&attr).map_err(|error| {
                    EventResult::Toast(AppToast::Error(error.to_string()), false)
                })?,
            )
        }
        hdf5_metno::types::TypeDescriptor::Reference(Reference::Region) => {
            return Ok(Some(EventResult::Toast(
                AppToast::Warning(
                    "Dataset region reference navigation is not supported yet".to_string(),
                ),
                false,
            )));
        }
        _ => None,
    };

    let Some(target) = target else {
        return Ok(None);
    };

    state
        .navigate_to_attribute_target(target.path.as_str(), target.attr_name.as_deref())
        .map_err(|error| EventResult::Toast(AppToast::Error(error.to_string()), false))?;
    Ok(Some(EventResult::Redraw))
}

fn selected_attribute_edit_request(
    state: &mut AppState<'_>,
) -> Result<AttributeEditRequest, EventResult> {
    let (row, selection) = selected_metadata_row(state)?;
    let Some(attr_name) = row.key.clone() else {
        return Err(EventResult::Toast(
            AppToast::Error("No attribute selected".to_string()),
            true,
        ));
    };
    if !matches!(row.kind, MetadataRowKind::Attribute) {
        return Err(EventResult::Toast(
            AppToast::Warning(format!(
                "'{}' is a built-in h5v property and cannot be edited",
                attr_name
            )),
            false,
        ));
    }

    let (_, attr, _) = selected_attribute(state)?;
    let edit_name_hint = {
        let node = state.treeview[state.tree_view_cursor].node.borrow();
        let node_path = node.node.path();
        if matches!(selection, Name) || Path::new(&attr_name).extension().is_some() {
            attr_name.clone()
        } else {
            format!("{node_path}/{attr_name}")
        }
    };

    if let Err(e) = attr.can_edit() {
        if let Value = selection {
            return Err(EventResult::Toast(
                AppToast::Error(format!(
                    "Attribute '{}' value cannot be edited: {}",
                    attr_name, e
                )),
                false,
            ));
        }
    }

    let content = match selection {
        Name => attr_name.clone(),
        Value => format_attr_for_edit(&attr)
            .map_err(|error| EventResult::Toast(AppToast::Error(error.to_string()), false))?,
    };

    Ok(AttributeEditRequest {
        attr_name,
        content,
        selection,
        edit_name_hint,
    })
}

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
                    let selection = node.attributes_view_cursor.attribute_view_selection.clone();
                    let destination = {
                        let attributes = node.read_attributes()?;
                        let mut destination = None;
                        for _ in 0..amount {
                            let (row_index, next_selection) =
                                destination.unwrap_or((current_index, selection.clone()));
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
                    let selection = node.attributes_view_cursor.attribute_view_selection.clone();
                    let destination = {
                        let attributes = node.read_attributes()?;
                        let mut destination = None;
                        for _ in 0..amount {
                            let (row_index, next_selection) =
                                destination.unwrap_or((current_index, selection.clone()));
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
                    let selection = node.attributes_view_cursor.attribute_view_selection.clone();
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
                    let selection = node.attributes_view_cursor.attribute_view_selection.clone();
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
