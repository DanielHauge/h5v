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

use crate::{
    error::AppError,
    h5f::{read_attr_memory_bytes, MetadataRowKind},
    ui::{
        render::attribute_type_descriptor,
        state::{AppState, AppToast, ContentShowMode, Focus},
    },
};

use super::{selection::selected_attribute, EventResult, Value};

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

pub(super) fn select_dataset_region_axes(
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

pub(super) fn navigate_reference_attribute_value(
    state: &mut AppState<'_>,
) -> Result<Option<EventResult>, EventResult> {
    let (row, selection) = super::selection::selected_metadata_row(state)?;
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
