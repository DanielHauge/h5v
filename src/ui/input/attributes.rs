use std::path::Path;

use std::{ffi::CStr, os::raw::c_char, ptr};

use hdf5_metno::{types::Reference, Attribute, ObjectReference1, ReferencedObject};
use hdf5_metno_sys::{
    h5::hsize_t,
    h5p::H5P_DEFAULT,
    h5r::{
        hdset_reg_ref_t, H5R_ref_t, H5Rget_attr_name, H5Rget_name, H5Rget_obj_name, H5Rget_region,
        H5Rget_type, H5Ropen_region, H5R_ATTR, H5R_DATASET_REGION1, H5R_DATASET_REGION2,
        H5R_OBJECT2,
    },
    h5s::{
        H5Sclose, H5Sget_select_bounds, H5Sget_select_elem_npoints, H5Sget_select_hyper_nblocks,
        H5Sget_select_type, H5Sget_simple_extent_ndims, H5S_SEL_ALL, H5S_SEL_HYPERSLABS,
        H5S_SEL_POINTS,
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
            ContentShowMode, FixedStringOverflowChoice, FixedStringOverflowDialogState, Focus,
            Mode,
        },
    },
};

use super::{
    execute_bound_command, execute_bound_lua_callback, execute_bound_script,
    keymap::{attributes_action, AttributesAction, BoundAction, Direction, EffectiveKeymaps},
    EventResult,
};

struct DatasetRegionNavigationTarget {
    path: String,
    start: Vec<usize>,
    end: Vec<usize>,
    approximate: bool,
}

enum ReferenceNavigationTarget {
    Object {
        path: String,
        attr_name: Option<String>,
    },
    DatasetRegion(DatasetRegionNavigationTarget),
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
    let selection = node.attributes_view_cursor.attribute_view_selection;
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
        return Err(AppError::EditError(format!(
            "Failed to query {context} length"
        )));
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

fn read_single_reference_bytes<const N: usize>(attr: &Attribute) -> Result<[u8; N], AppError> {
    let bytes = read_attr_memory_bytes(attr)?;
    if bytes.len() != N {
        return Err(AppError::EditError(
            "Reference navigation requires a single target".to_string(),
        ));
    }
    let mut reference = [0_u8; N];
    reference.copy_from_slice(&bytes);
    Ok(reference)
}

fn read_region_bounds(space_id: i64) -> Result<(Vec<usize>, Vec<usize>, bool), AppError> {
    let ndims = unsafe { H5Sget_simple_extent_ndims(space_id) };
    if ndims < 0 {
        return Err(AppError::EditError(
            "Failed to inspect referenced region".to_string(),
        ));
    }
    let rank = ndims as usize;
    let mut start = vec![0 as hsize_t; rank];
    let mut end = vec![0 as hsize_t; rank];
    if unsafe { H5Sget_select_bounds(space_id, start.as_mut_ptr(), end.as_mut_ptr()) } < 0 {
        return Err(AppError::EditError(
            "Failed to inspect referenced region bounds".to_string(),
        ));
    }

    let selection_type = unsafe { H5Sget_select_type(space_id) };
    let approximate = match selection_type {
        H5S_SEL_POINTS => (unsafe { H5Sget_select_elem_npoints(space_id) }) > 1,
        H5S_SEL_HYPERSLABS => (unsafe { H5Sget_select_hyper_nblocks(space_id) }) > 1,
        H5S_SEL_ALL => false,
        unsupported => {
            return Err(AppError::EditError(format!(
                "Unsupported referenced region selection type: {unsupported:?}"
            )))
        }
    };

    Ok((
        start.into_iter().map(|idx| idx as usize).collect(),
        end.into_iter().map(|idx| idx as usize).collect(),
        approximate,
    ))
}

fn read_region_target_from_space(
    path: String,
    space_id: i64,
) -> Result<ReferenceNavigationTarget, AppError> {
    let result = read_region_bounds(space_id);
    unsafe {
        H5Sclose(space_id);
    }
    let (start, end, approximate) = result?;
    Ok(ReferenceNavigationTarget::DatasetRegion(
        DatasetRegionNavigationTarget {
            path,
            start,
            end,
            approximate,
        },
    ))
}

fn resolve_std_reference_target(attr: &Attribute) -> Result<ReferenceNavigationTarget, AppError> {
    let reference_bytes =
        read_single_reference_bytes::<{ std::mem::size_of::<H5R_ref_t>() }>(attr)?;
    let reference = reference_bytes.as_ptr().cast::<H5R_ref_t>();
    match unsafe { H5Rget_type(reference) } {
        H5R_OBJECT2 => Ok(ReferenceNavigationTarget::Object {
            path: read_hdf5_name(
                |name, size| unsafe {
                    H5Rget_obj_name(reference, H5P_DEFAULT, name, size) as isize
                },
                "reference target path",
            )?,
            attr_name: None,
        }),
        H5R_DATASET_REGION2 => {
            let path = read_hdf5_name(
                |name, size| unsafe {
                    H5Rget_obj_name(reference, H5P_DEFAULT, name, size) as isize
                },
                "reference target path",
            )?;
            let space_id = unsafe { H5Ropen_region(reference, H5P_DEFAULT, H5P_DEFAULT) };
            if space_id < 0 {
                return Err(AppError::EditError(
                    "Failed to open referenced dataset region".to_string(),
                ));
            }
            read_region_target_from_space(path, space_id)
        }
        H5R_ATTR => Ok(ReferenceNavigationTarget::Object {
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

fn resolve_region_reference_target(
    attr: &Attribute,
) -> Result<ReferenceNavigationTarget, AppError> {
    let file = attr.file()?;
    let reference_bytes =
        read_single_reference_bytes::<{ std::mem::size_of::<hdset_reg_ref_t>() }>(attr)?;
    let reference = reference_bytes.as_ptr().cast();
    let path = read_hdf5_name(
        |name, size| unsafe {
            H5Rget_name(file.id(), H5R_DATASET_REGION1, reference, name, size) as isize
        },
        "reference target path",
    )?;
    let space_id = unsafe { H5Rget_region(file.id(), H5R_DATASET_REGION1, reference) };
    if space_id < 0 {
        return Err(AppError::EditError(
            "Failed to open referenced dataset region".to_string(),
        ));
    }
    read_region_target_from_space(path, space_id)
}

fn select_dataset_region_axes(
    shape: &[usize],
    start: &[usize],
    end: &[usize],
) -> (usize, Option<usize>, usize) {
    let mut varying_dims = start
        .iter()
        .zip(end.iter())
        .enumerate()
        .filter(|(_, (start, end))| start != end)
        .map(|(dim, _)| dim)
        .collect::<Vec<_>>();
    if varying_dims.is_empty() {
        varying_dims.extend(
            shape
                .iter()
                .enumerate()
                .filter(|(_, len)| **len > 1)
                .map(|(dim, _)| dim),
        );
    }

    let row_dim = varying_dims.first().copied().unwrap_or(0);
    let col_dim = if shape.len() > 1 {
        varying_dims
            .iter()
            .copied()
            .find(|dim| *dim != row_dim)
            .or_else(|| {
                shape
                    .iter()
                    .enumerate()
                    .find(|(dim, _)| *dim != row_dim)
                    .map(|(dim, _)| dim)
            })
    } else {
        None
    };
    let selected_dim = shape
        .iter()
        .enumerate()
        .find(|(dim, _)| Some(*dim) != col_dim && *dim != row_dim)
        .map(|(dim, _)| dim)
        .unwrap_or(0);
    (row_dim, col_dim, selected_dim)
}

fn navigate_dataset_region_target(
    state: &mut AppState<'_>,
    target: &DatasetRegionNavigationTarget,
) -> Result<Option<EventResult>, EventResult> {
    state
        .select_tree_node_by_path(target.path.as_str())
        .map_err(|error| EventResult::Toast(AppToast::Error(error.to_string()), false))?;

    state.heatmap_viewport_region = None;
    state.heatmap_region = None;
    state.heatmap_render.drag_state = None;
    state.heatmap_render.current_key = None;
    state.heatmap_render.current_selection = None;
    state.heatmap_render.current_line_profile = None;
    state.heatmap_render.current_legend_summary = None;
    state.heatmap_render.current_slice_summary = None;
    state.heatmap_render.page_window = None;
    state.heatmap_render.selected_cells = None;
    state.heatmap_render.selected_line = None;
    state.heatmap_render.viewport = None;
    state.matrix_view_state.row_offset = 0;
    state.matrix_view_state.col_offset = 0;
    state.matrix_view_state.cursor_row = 0;
    state.matrix_view_state.cursor_col = 0;

    let current_node = &state.treeview[state.tree_view_cursor];
    let mut node = current_node.node.borrow_mut();
    let (shape, available_modes) = match &node.node {
        crate::h5f::Node::Dataset(_, meta) => (
            meta.shape.clone(),
            state.filter_runtime_content_modes(node.content_show_modes()),
        ),
        _ => {
            state.focus = Focus::Content;
            return Ok(None);
        }
    };

    if target.start.len() != shape.len() || target.end.len() != shape.len() {
        return Err(EventResult::Toast(
            AppToast::Error("Referenced region shape does not match target dataset".to_string()),
            false,
        ));
    }

    node.sync_selection_rank(shape.len());
    node.selected_indexes = target
        .start
        .iter()
        .enumerate()
        .map(|(dim, idx)| (*idx).min(shape[dim].saturating_sub(1)))
        .collect();

    let (row_dim, col_dim, selected_dim) =
        select_dataset_region_axes(&shape, &target.start, &target.end);
    node.selected_row = row_dim.min(shape.len().saturating_sub(1));
    if let Some(col_dim) = col_dim {
        node.selected_col = col_dim.min(shape.len().saturating_sub(1));
    }
    node.selected_dim = selected_dim.min(shape.len().saturating_sub(1));
    drop(node);

    if available_modes.contains(&ContentShowMode::Heatmap) && col_dim.is_some() {
        let col_dim = col_dim.unwrap_or(0);
        state.set_content_mode(ContentShowMode::Heatmap);
        state.heatmap_render.viewport = Some(crate::ui::state::HeatmapViewport {
            row_start: target.start[row_dim],
            row_len: target.end[row_dim]
                .saturating_sub(target.start[row_dim])
                .saturating_add(1),
            col_start: target.start[col_dim],
            col_len: target.end[col_dim]
                .saturating_sub(target.start[col_dim])
                .saturating_add(1),
        });
        state.heatmap_render.selected_cells =
            Some(crate::ui::state::HeatmapSelectedCells::normalized(
                target.start[row_dim],
                target.start[col_dim],
                target.end[row_dim],
                target.end[col_dim],
            ));
    } else if available_modes.contains(&ContentShowMode::Matrix) {
        state.set_content_mode(ContentShowMode::Matrix);
        state.matrix_view_state.row_offset = target.start[row_dim];
        if let Some(col_dim) = col_dim {
            state.matrix_view_state.col_offset = target.start[col_dim];
        }
    } else if available_modes.contains(&ContentShowMode::Preview) {
        state.set_content_mode(ContentShowMode::Preview);
    }
    state.focus = Focus::Content;

    if target.approximate {
        Ok(Some(EventResult::Toast(
            AppToast::Info(
                "Referenced region opened using its bounding box in the current view".to_string(),
            ),
            false,
        )))
    } else {
        Ok(Some(EventResult::Redraw))
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
            Some(ReferenceNavigationTarget::Object {
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
                resolve_region_reference_target(&attr).map_err(|error| {
                    EventResult::Toast(AppToast::Error(error.to_string()), false)
                })?,
            )
        }
        _ => None,
    };

    let Some(target) = target else {
        return Ok(None);
    };

    match target {
        ReferenceNavigationTarget::Object { path, attr_name } => {
            state
                .navigate_to_attribute_target(path.as_str(), attr_name.as_deref())
                .map_err(|error| EventResult::Toast(AppToast::Error(error.to_string()), false))?;
            Ok(Some(EventResult::Redraw))
        }
        ReferenceNavigationTarget::DatasetRegion(target) => {
            navigate_dataset_region_target(state, &target)
        }
    }
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
