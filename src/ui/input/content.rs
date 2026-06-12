use std::borrow::Cow;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

use arboard::ImageData;
#[cfg(target_os = "linux")]
use arboard::SetExtLinux;
use hdf5_metno::types::{EnumType, TypeDescriptor};
use hdf5_metno::{Dataset, Hyperslab, Selection, SliceOrIndex};
use image::{DynamicImage, ImageBuffer, Rgb};
use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind};

use crate::{
    data::{PreviewSelection, Previewable, SliceSelection},
    error::AppError,
    h5f::{
        format_dataset_value_for_edit, plot_projected, read_opaque_dataset_preview,
        read_projected_scalar, read_scalar_string_dataset, read_single_value_dataset,
        write_dataset_value_from_text, DatasetMeta, H5FNode, Node,
    },
    ui::{
        edit::perform_edit,
        matrix::compound_root_matrix_cell_text,
        preview::chart::render_image_chart,
        preview::preview_text_for_compound_schema,
        render::MatrixRenderType,
        state::{preview_selection_for_node, AppState, AppToast, ContentShowMode},
    },
};

use super::{
    execute_bound_command, execute_bound_lua_callback, execute_bound_script,
    keymap::{
        content_action, heatmap_action, BoundAction, ContentAction, Direction, EffectiveKeymaps,
    },
    EventResult,
};

#[derive(Clone)]
struct ContentEditRequest {
    dataset: Dataset,
    meta: DatasetMeta,
    selection: Option<Selection>,
    content: String,
    edit_name_hint: String,
}

fn exact_element_selection(indices: &[usize]) -> Selection {
    let slice = indices
        .iter()
        .copied()
        .map(SliceOrIndex::Index)
        .collect::<Vec<_>>();
    Selection::Hyperslab(Hyperslab::from(slice))
}

fn copy_text_to_clipboard(
    state: &mut AppState<'_>,
    text: String,
    success_message: &str,
) -> Result<EventResult, AppError> {
    if let Err(error) = state.set_clipboard_text(text) {
        return Ok(EventResult::Toast(AppToast::Warning(error), false));
    }
    Ok(EventResult::Toast(
        AppToast::Info(success_message.to_string()),
        false,
    ))
}

fn redraw_if(changed: bool) -> EventResult {
    if changed {
        EventResult::Redraw
    } else {
        EventResult::Continue
    }
}

fn copy_image_to_clipboard(
    state: &mut AppState<'_>,
    width: usize,
    height: usize,
    bytes: Vec<u8>,
    success_message: &str,
) -> Result<EventResult, AppError> {
    let image = ImageData {
        width,
        height,
        bytes: Cow::Owned(bytes),
    };
    let Some(clipboard) = state.clipboard.as_mut() else {
        return Ok(EventResult::Toast(
            AppToast::Warning(state.clipboard_unavailable_message()),
            false,
        ));
    };
    #[cfg(target_os = "linux")]
    let copy_result = clipboard
        .set()
        .wait_until(Instant::now() + Duration::from_millis(300))
        .image(image);
    #[cfg(not(target_os = "linux"))]
    let copy_result = clipboard.set_image(image);

    if let Err(error) = copy_result {
        return Ok(EventResult::Toast(
            AppToast::Warning(format!("Failed to copy preview image: {error}")),
            false,
        ));
    }
    Ok(EventResult::Toast(
        AppToast::Info(success_message.to_string()),
        false,
    ))
}

fn selected_matrix_copy_text(state: &mut AppState<'_>) -> Result<String, EventResult> {
    let tree_item = state.treeview[state.tree_view_cursor].node.clone();
    let mut node = tree_item.borrow_mut();
    let (dataset, meta) = match &node.node {
        Node::Dataset(dataset, meta) => (dataset.clone(), meta.clone()),
        _ => {
            return Err(EventResult::Toast(
                AppToast::Warning("Only datasets can be copied from content view".to_string()),
                false,
            ))
        }
    };

    if meta.is_compound_container() && meta.supports_compound_root_matrix() {
        let field_count = meta.compound_root_matrix_column_count().unwrap_or_default();
        let row_dim = if meta
            .shape
            .get(node.selected_row)
            .copied()
            .unwrap_or_default()
            > 1
        {
            node.selected_row
        } else {
            meta.shape
                .iter()
                .enumerate()
                .find(|(_, len)| **len > 1)
                .map(|(dim, _)| dim)
                .unwrap_or(0)
        };
        let row_count = meta.shape.get(row_dim).copied().unwrap_or_default();
        if row_count == 0 || field_count == 0 {
            return Err(EventResult::Toast(
                AppToast::Warning("Current matrix content cannot be copied".to_string()),
                false,
            ));
        }
        let visible_rows = state
            .matrix_view_state
            .rows_currently_available
            .max(1)
            .min(row_count);
        let visible_cols = state
            .matrix_view_state
            .cols_currently_available
            .max(1)
            .min(field_count);
        let row_index = state
            .matrix_view_state
            .row_offset
            .min(row_count.saturating_sub(visible_rows))
            + state
                .matrix_view_state
                .cursor_row
                .min(visible_rows.saturating_sub(1));
        let field_index = state
            .matrix_view_state
            .col_offset
            .min(field_count.saturating_sub(visible_cols))
            + state
                .matrix_view_state
                .cursor_col
                .min(visible_cols.saturating_sub(1));
        return compound_root_matrix_cell_text(
            &dataset,
            &meta,
            row_dim,
            row_index,
            field_index,
            &node.selected_indexes,
        )
        .map_err(|error| EventResult::Toast(AppToast::Error(error.to_string()), false));
    }

    if meta.matrixable.is_none() {
        return Err(EventResult::Toast(
            AppToast::Warning("Current matrix content cannot be copied".to_string()),
            false,
        ));
    }

    let shape = dataset.shape();
    let rank = shape.len();
    node.sync_selection_rank(rank);
    let mut indices = node.selected_indexes.clone();
    indices.resize(rank, 0);

    let selection = if rank == 0 {
        None
    } else if rank == 1 {
        let visible_rows = state
            .matrix_view_state
            .rows_currently_available
            .max(1)
            .min(shape[0]);
        let base_row = state
            .matrix_view_state
            .row_offset
            .min(shape[0].saturating_sub(visible_rows));
        indices[0] = base_row
            + state
                .matrix_view_state
                .cursor_row
                .min(visible_rows.saturating_sub(1));
        Some(exact_element_selection(&indices))
    } else {
        let row_dim = node.selected_row.min(rank.saturating_sub(1));
        let col_dim = node.selected_col.min(rank.saturating_sub(1));
        let visible_rows = state
            .matrix_view_state
            .rows_currently_available
            .max(1)
            .min(shape[row_dim]);
        let visible_cols = state
            .matrix_view_state
            .cols_currently_available
            .max(1)
            .min(shape[col_dim]);
        let base_row = state
            .matrix_view_state
            .row_offset
            .min(shape[row_dim].saturating_sub(visible_rows));
        let base_col = state
            .matrix_view_state
            .col_offset
            .min(shape[col_dim].saturating_sub(visible_cols));
        indices[row_dim] = base_row
            + state
                .matrix_view_state
                .cursor_row
                .min(visible_rows.saturating_sub(1));
        indices[col_dim] = base_col
            + state
                .matrix_view_state
                .cursor_col
                .min(visible_cols.saturating_sub(1));
        Some(exact_element_selection(&indices))
    };

    format_dataset_value_for_edit(&dataset, &meta, selection.as_ref())
        .map_err(|error| EventResult::Toast(AppToast::Error(error.to_string()), false))
}

fn selected_heatmap_copy_text(state: &mut AppState<'_>) -> Result<String, EventResult> {
    let Some(region) = state.heatmap_region.as_ref() else {
        return Err(EventResult::Toast(
            AppToast::Warning("Heatmap selection is not available yet".to_string()),
            false,
        ));
    };
    Ok(region.summary())
}

fn enum_value_to_string(enum_type: &EnumType, value: u64) -> String {
    enum_type
        .members
        .iter()
        .find(|member| member.value == value)
        .map(|member| member.name.clone())
        .unwrap_or_else(|| format!("Unknown enum value: {value}"))
}

fn chart_preview_selection(
    node: &mut H5FNode,
    shape: &[usize],
    page_idx: i32,
) -> Option<PreviewSelection> {
    preview_selection_for_node(node, shape, page_idx)
}

fn preview_text_value(
    node: &mut H5FNode,
    dataset: &Dataset,
    meta: &DatasetMeta,
) -> Result<Option<String>, AppError> {
    if meta.is_opaque() {
        return Ok(Some(read_opaque_dataset_preview(dataset, meta)?));
    }

    if meta.image.is_some() {
        return Ok(None);
    }

    if let Some(schema) = preview_text_for_compound_schema(meta) {
        return Ok(Some(schema));
    }

    if meta.matrixable.is_none() {
        return Ok(Some(read_scalar_string_dataset(dataset, &meta.encoding)?));
    }

    let Some(_) = chart_preview_selection(node, &dataset.shape(), 0) else {
        return match meta.matrixable {
            Some(MatrixRenderType::Float64) => {
                if meta.is_compound_leaf() {
                    Ok(Some(
                        read_projected_scalar::<f64>(dataset, meta)?.to_string(),
                    ))
                } else {
                    Ok(Some(read_single_value_dataset::<f64>(dataset)?.to_string()))
                }
            }
            Some(MatrixRenderType::Uint64) => {
                if meta.is_compound_leaf() {
                    Ok(Some(
                        read_projected_scalar::<u64>(dataset, meta)?.to_string(),
                    ))
                } else {
                    Ok(Some(read_single_value_dataset::<u64>(dataset)?.to_string()))
                }
            }
            Some(MatrixRenderType::Int64) => {
                if meta.is_compound_leaf() {
                    Ok(Some(
                        read_projected_scalar::<i64>(dataset, meta)?.to_string(),
                    ))
                } else {
                    Ok(Some(read_single_value_dataset::<i64>(dataset)?.to_string()))
                }
            }
            Some(MatrixRenderType::Opaque) => {
                Ok(Some(format_dataset_value_for_edit(dataset, meta, None)?))
            }
            Some(MatrixRenderType::Strings) => {
                if meta.is_compound_leaf() {
                    Ok(Some(read_projected_scalar::<String>(dataset, meta)?))
                } else {
                    Ok(Some(read_scalar_string_dataset(dataset, &meta.encoding)?))
                }
            }
            Some(MatrixRenderType::Enum) => {
                let TypeDescriptor::Enum(enum_type) = &meta.type_descriptor else {
                    return Err(AppError::EditError(
                        "Enum preview lost its enum type descriptor".to_string(),
                    ));
                };
                let rendered = if meta.is_compound_leaf() {
                    enum_value_to_string(enum_type, read_projected_scalar::<u64>(dataset, meta)?)
                } else {
                    enum_value_to_string(enum_type, read_single_value_dataset::<u64>(dataset)?)
                };
                Ok(Some(rendered))
            }
            Some(MatrixRenderType::ByteArray) => Ok(None),
            _ => Ok(None),
        };
    };

    Ok(None)
}

fn copy_chart_preview(
    state: &mut AppState<'_>,
    ds_path: &str,
    selection: PreviewSelection,
    data_preview: crate::data::DatasetPlotingData,
) -> Result<EventResult, AppError> {
    if let Some(chart_area) = state.chart_preview_state.last_chart_area {
        if state.chart_preview_state.current_request_key()
            == Some(crate::ui::state::ChartPreviewKey {
                ds_path: ds_path.to_string(),
                selection: selection.clone(),
                viewport: state.chart_preview_state.rendered_viewport,
                roi: state.chart_preview_state.rendered_roi,
                width: chart_area.width,
                height: chart_area.height,
            })
        {
            if let Some(image) = &state.chart_preview_state.clipboard_image {
                return copy_image_to_clipboard(
                    state,
                    image.width,
                    image.height,
                    image.bytes.clone(),
                    "Copied chart preview image to clipboard",
                );
            }
        }
    }

    let width = state
        .ui_layout
        .content
        .map(|area| u32::from(area.width.max(20)) * u32::from(state.image_cell_size.0.max(1)))
        .unwrap_or(800);
    let height = state
        .ui_layout
        .content
        .map(|area| u32::from(area.height.max(12)) * u32::from(state.image_cell_size.1.max(1)))
        .unwrap_or(600);
    let mut buffer = vec![0; (width * height * 3) as usize];
    let x_min = match selection.slice {
        SliceSelection::All => 0.0,
        SliceSelection::FromTo(start, _) => start as f64,
    };
    render_image_chart(
        &mut buffer,
        width,
        height,
        x_min,
        data_preview,
        state.chart_preview_state.effective_viewport(),
        state.chart_preview_state.roi,
    )?;
    let image = ImageBuffer::<Rgb<u8>, _>::from_raw(width, height, buffer).ok_or_else(|| {
        AppError::DrawingError("Failed to build chart preview image for clipboard".to_string())
    })?;
    let rgba = DynamicImage::ImageRgb8(image).to_rgba8();
    copy_image_to_clipboard(
        state,
        rgba.width() as usize,
        rgba.height() as usize,
        rgba.into_raw(),
        "Copied chart preview image to clipboard",
    )
}

fn copy_preview_content(state: &mut AppState<'_>) -> Result<EventResult, AppError> {
    let tree_item = state.treeview[state.tree_view_cursor].node.clone();
    let mut node = tree_item.borrow_mut();
    let (dataset, meta) = match &node.node {
        Node::Dataset(dataset, meta) => (dataset.clone(), meta.clone()),
        _ => {
            return Ok(EventResult::Toast(
                AppToast::Warning("Only datasets can be copied from content view".to_string()),
                false,
            ))
        }
    };
    let ds_path = meta.virtual_path().unwrap_or(&dataset.name()).to_string();

    if meta.image.is_some() {
        if let Some(message) = &state.img_state.error {
            return Ok(EventResult::Toast(
                AppToast::Warning(message.clone()),
                false,
            ));
        }
        if state.img_state.ds.as_deref() != Some(ds_path.as_str()) {
            return Ok(EventResult::Toast(
                AppToast::Warning("Image preview is still loading".to_string()),
                false,
            ));
        }
        let Some(image) = &state.img_state.clipboard_image else {
            return Ok(EventResult::Toast(
                AppToast::Warning("Image preview is still loading".to_string()),
                false,
            ));
        };
        return copy_image_to_clipboard(
            state,
            image.width,
            image.height,
            image.bytes.clone(),
            "Copied preview image to clipboard",
        );
    }

    if let Some(text) = preview_text_value(&mut node, &dataset, &meta)? {
        return copy_text_to_clipboard(state, text, "Copied preview value to clipboard");
    }

    let shape = dataset.shape();
    let Some(selection) = chart_preview_selection(&mut node, &shape, state.page_state.idx) else {
        return Ok(EventResult::Toast(
            AppToast::Warning("Current preview cannot be copied".to_string()),
            false,
        ));
    };

    let data_preview = if meta.is_compound_leaf() {
        plot_projected(&dataset, &meta, &selection)?
    } else {
        dataset.plot(&selection)?
    };
    copy_chart_preview(state, &ds_path, selection, data_preview)
}

fn selected_content_edit_request(
    state: &mut AppState<'_>,
) -> Result<ContentEditRequest, EventResult> {
    let tree_item = state.treeview[state.tree_view_cursor].node.clone();
    let content_mode = state.active_content_mode();
    let mut node = tree_item.borrow_mut();
    let (dataset, meta) = match &node.node {
        Node::Dataset(dataset, meta) => (dataset.clone(), meta.clone()),
        _ => {
            return Err(EventResult::Toast(
                AppToast::Warning("Only datasets can be edited from content view".to_string()),
                false,
            ))
        }
    };
    let dataset_shape = dataset.shape();
    let dataset_rank = dataset_shape.len();

    let selection = match content_mode {
        ContentShowMode::Preview => {
            if meta.image.is_some() {
                return Err(EventResult::Toast(
                    AppToast::Warning("Image previews are not editable".to_string()),
                    false,
                ));
            }
            if meta.is_compound_container() {
                return Err(EventResult::Toast(
                    AppToast::Warning("Compound schema previews are not editable".to_string()),
                    false,
                ));
            }
            if dataset.size() != 1 {
                return Err(EventResult::Toast(
                    AppToast::Warning(
                        "Preview editing only supports a single selected value; use Matrix mode for cell edits"
                            .to_string(),
                    ),
                    false,
                ));
            }
            if dataset.is_scalar() {
                None
            } else {
                Some(exact_element_selection(&vec![0; dataset_rank]))
            }
        }
        ContentShowMode::Matrix => {
            if meta.matrixable.is_none() {
                return Err(EventResult::Toast(
                    AppToast::Warning("Current matrix content is not editable".to_string()),
                    false,
                ));
            }

            let rank = dataset_rank;
            node.sync_selection_rank(rank);
            let mut indices = node.selected_indexes.clone();
            indices.resize(rank, 0);

            if rank == 0 {
                None
            } else if rank == 1 {
                let visible_rows = state
                    .matrix_view_state
                    .rows_currently_available
                    .max(1)
                    .min(meta.shape[0]);
                let base_row = state
                    .matrix_view_state
                    .row_offset
                    .min(meta.shape[0].saturating_sub(visible_rows));
                indices[0] = base_row
                    + state
                        .matrix_view_state
                        .cursor_row
                        .min(visible_rows.saturating_sub(1));
                Some(exact_element_selection(&indices))
            } else {
                let row_dim = node.selected_row.min(rank.saturating_sub(1));
                let col_dim = node.selected_col.min(rank.saturating_sub(1));
                let visible_rows = state
                    .matrix_view_state
                    .rows_currently_available
                    .max(1)
                    .min(meta.shape[row_dim]);
                let visible_cols = state
                    .matrix_view_state
                    .cols_currently_available
                    .max(1)
                    .min(meta.shape[col_dim]);
                let base_row = state
                    .matrix_view_state
                    .row_offset
                    .min(meta.shape[row_dim].saturating_sub(visible_rows));
                let base_col = state
                    .matrix_view_state
                    .col_offset
                    .min(meta.shape[col_dim].saturating_sub(visible_cols));
                indices[row_dim] = base_row
                    + state
                        .matrix_view_state
                        .cursor_row
                        .min(visible_rows.saturating_sub(1));
                indices[col_dim] = base_col
                    + state
                        .matrix_view_state
                        .cursor_col
                        .min(visible_cols.saturating_sub(1));
                Some(exact_element_selection(&indices))
            }
        }
        ContentShowMode::Heatmap => {
            return Err(EventResult::Toast(
                AppToast::Warning(
                    "Heatmap mode is read-only for now; use Matrix mode to edit values".to_string(),
                ),
                false,
            ));
        }
    };
    let edit_name_hint = meta.virtual_path().unwrap_or(&dataset.name()).to_string();

    let content = format_dataset_value_for_edit(&dataset, &meta, selection.as_ref())
        .map_err(|error| EventResult::Toast(AppToast::Error(error.to_string()), false))?;

    Ok(ContentEditRequest {
        dataset,
        meta,
        selection,
        content,
        edit_name_hint,
    })
}

fn apply_content_edit_request(
    state: &mut AppState<'_>,
    request: &ContentEditRequest,
) -> Result<EventResult, AppError> {
    if state.readonly {
        return Ok(EventResult::Toast(
            AppToast::Warning(
                "Cannot edit in read-only mode; reopen with -w to modify the file".to_string(),
            ),
            false,
        ));
    }

    state.editing = true;
    let edited_content = match perform_edit(
        state,
        request.content.clone(),
        Some(&request.edit_name_hint),
    ) {
        Ok(content) => content,
        Err(error) => {
            state.editing = false;
            return Ok(EventResult::Toast(
                AppToast::Error(format!("Failed to edit content: {}", error)),
                true,
            ));
        }
    };
    state.editing = false;
    if edited_content == request.content {
        return Ok(EventResult::Continue);
    }

    write_dataset_value_from_text(
        &request.dataset,
        &request.meta,
        request.selection.as_ref(),
        &edited_content,
    )?;

    state.chart_preview_state.ds_loaded = None;
    state.chart_preview_state.ds_selection = None;
    state.chart_preview_state.protocol = None;
    state.chart_preview_state.clipboard_image = None;
    state.chart_preview_state.error = None;
    state.chart_preview_state.rendered_viewport = None;
    state.chart_preview_state.rendered_size = None;
    state.chart_preview_state.pending_key = None;
    state.chart_preview_state.cached_previews.clear();
    state.chart_preview_state.reset_viewport();
    state.img_state.ds = None;
    state.img_state.current_key = None;
    state.img_state.protocol = None;
    state.img_state.clipboard_image = None;
    state.img_state.error = None;
    state.heatmap_viewport_region = None;
    state.heatmap_region = None;
    state.heatmap_render.current_key = None;
    state.heatmap_render.current_selection = None;
    state.heatmap_render.current_line_profile = None;
    state.heatmap_render.current_legend_summary = None;
    state.heatmap_render.current_slice_summary = None;
    state.heatmap_render.viewport = None;
    state.heatmap_render.selected_cells = None;
    state.heatmap_render.selected_line = None;
    state.heatmap_render.page_window = None;
    state.heatmap_render.cached_pages.clear();
    state.heatmap_render.pending_keys.clear();
    state.acknowledge_file_write();

    if matches!(request.meta.matrixable, Some(MatrixRenderType::ByteArray)) {
        return Ok(EventResult::ReloadFile {
            write: !state.readonly,
        });
    }

    Ok(EventResult::Toast(
        AppToast::Info("Updated content value".to_string()),
        true,
    ))
}

pub fn handle_normal_content_event(
    state: &mut AppState<'_>,
    event: Event,
    keymaps: &EffectiveKeymaps,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => {
                let content_mode = state.active_content_mode();
                if matches!(content_mode, ContentShowMode::Preview)
                    && key_event.code == KeyCode::Esc
                {
                    return Ok(redraw_if(state.chart_preview_state.clear_roi_or_zoom()));
                }
                let action = if matches!(content_mode, ContentShowMode::Heatmap) {
                    heatmap_action(&key_event, keymaps)
                        .or_else(|| content_action(&key_event, keymaps))
                } else {
                    content_action(&key_event, keymaps)
                };
                match (action, content_mode) {
                    (
                        Some(BoundAction::Action(ContentAction::Move(Direction::Left, amount))),
                        ContentShowMode::Matrix,
                    ) => {
                        let max = state
                            .matrix_view_state
                            .cols_currently_available
                            .saturating_sub(1);

                        let move_within_view = state.matrix_view_state.cursor_col.min(amount);
                        let remaining = amount.saturating_sub(move_within_view);
                        if remaining > 0 && state.matrix_view_state.col_offset > 0 {
                            state.left(remaining as isize)?;
                        }
                        let new_cursor = state
                            .matrix_view_state
                            .cursor_col
                            .saturating_sub(move_within_view)
                            .clamp(0, max);
                        state.matrix_view_state.cursor_col = new_cursor;

                        Ok(EventResult::Redraw)
                    }
                    (
                        Some(BoundAction::Action(ContentAction::Move(Direction::Left, amount))),
                        ContentShowMode::Heatmap,
                    ) => state.left(amount as isize),
                    (
                        Some(BoundAction::Action(ContentAction::Move(Direction::Right, amount))),
                        ContentShowMode::Matrix,
                    ) => {
                        let max = state
                            .matrix_view_state
                            .cols_currently_available
                            .saturating_sub(1);

                        let move_within_view = max
                            .saturating_sub(state.matrix_view_state.cursor_col)
                            .min(amount);
                        let remaining = amount.saturating_sub(move_within_view);
                        if remaining > 0 {
                            state.right(remaining as isize)?;
                        }
                        let new_cursor = state
                            .matrix_view_state
                            .cursor_col
                            .saturating_add(move_within_view)
                            .clamp(0, max);
                        state.matrix_view_state.cursor_col = new_cursor;
                        Ok(EventResult::Redraw)
                    }
                    (
                        Some(BoundAction::Action(ContentAction::Move(Direction::Right, amount))),
                        ContentShowMode::Heatmap,
                    ) => state.right(amount as isize),
                    (
                        Some(BoundAction::Action(ContentAction::Move(Direction::Up, amount))),
                        ContentShowMode::Matrix,
                    ) => {
                        let max = state
                            .matrix_view_state
                            .rows_currently_available
                            .saturating_sub(1);

                        let move_within_view = state.matrix_view_state.cursor_row.min(amount);
                        let remaining = amount.saturating_sub(move_within_view);
                        if remaining > 0 && state.matrix_view_state.row_offset > 0 {
                            state.up(remaining)?;
                        }
                        let new_cursor = state
                            .matrix_view_state
                            .cursor_row
                            .saturating_sub(move_within_view)
                            .clamp(0, max);
                        state.matrix_view_state.cursor_row = new_cursor;
                        Ok(EventResult::Redraw)
                    }
                    (
                        Some(BoundAction::Action(ContentAction::Move(Direction::Up, amount))),
                        ContentShowMode::Heatmap,
                    ) => state.up(amount),
                    (
                        Some(BoundAction::Action(ContentAction::Move(Direction::Down, amount))),
                        ContentShowMode::Matrix,
                    ) => {
                        let max = state
                            .matrix_view_state
                            .rows_currently_available
                            .saturating_sub(1);
                        let move_within_view = max
                            .saturating_sub(state.matrix_view_state.cursor_row)
                            .min(amount);
                        let remaining = amount.saturating_sub(move_within_view);
                        if remaining > 0 {
                            state.down(remaining)?;
                        }
                        let new_cursor = state
                            .matrix_view_state
                            .cursor_row
                            .saturating_add(move_within_view)
                            .clamp(0, max);
                        state.matrix_view_state.cursor_row = new_cursor;

                        Ok(EventResult::Redraw)
                    }
                    (
                        Some(BoundAction::Action(ContentAction::Move(Direction::Down, amount))),
                        ContentShowMode::Heatmap,
                    ) => state.down(amount),
                    (
                        Some(BoundAction::Action(ContentAction::Move(Direction::Down, amount))),
                        ContentShowMode::Preview,
                    ) => state.down(amount),
                    (
                        Some(BoundAction::Action(ContentAction::Move(Direction::Up, amount))),
                        ContentShowMode::Preview,
                    ) => state.up(amount),
                    (
                        Some(BoundAction::Action(ContentAction::Move(Direction::Right, amount))),
                        ContentShowMode::Preview,
                    ) => state.right(amount as isize),
                    (
                        Some(BoundAction::Action(ContentAction::Move(Direction::Left, amount))),
                        ContentShowMode::Preview,
                    ) => state.left(amount as isize),
                    (Some(BoundAction::Action(ContentAction::Edit)), _) => {
                        if matches!(content_mode, ContentShowMode::Heatmap) {
                            return Ok(EventResult::Toast(
                                AppToast::Info(
                                    "Heatmap mode is read-only for now; use Matrix mode to edit values"
                                        .to_string(),
                                ),
                                false,
                            ));
                        }
                        let request = match selected_content_edit_request(state) {
                            Ok(request) => request,
                            Err(event_result) => return Ok(event_result),
                        };
                        apply_content_edit_request(state, &request)
                    }
                    (Some(BoundAction::Action(ContentAction::Copy)), ContentShowMode::Preview) => {
                        copy_preview_content(state)
                    }
                    (Some(BoundAction::Action(ContentAction::Copy)), ContentShowMode::Matrix) => {
                        let text = match selected_matrix_copy_text(state) {
                            Ok(text) => text,
                            Err(event_result) => return Ok(event_result),
                        };
                        copy_text_to_clipboard(state, text, "Copied matrix value to clipboard")
                    }
                    (Some(BoundAction::Action(ContentAction::Copy)), ContentShowMode::Heatmap) => {
                        let text = match selected_heatmap_copy_text(state) {
                            Ok(text) => text,
                            Err(event_result) => return Ok(event_result),
                        };
                        copy_text_to_clipboard(state, text, "Copied heatmap region to clipboard")
                    }
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapZoomIn)),
                        ContentShowMode::Heatmap,
                    ) => Ok(redraw_if(state.zoom_heatmap(None, true))),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapZoomIn)),
                        ContentShowMode::Preview,
                    ) => Ok(redraw_if(state.chart_preview_state.zoom_with_anchor(
                        10.0,
                        0.5,
                        0.5,
                        true,
                        crate::ui::state::PreviewChartZoomMode::Uniform,
                    ))),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapZoomOut)),
                        ContentShowMode::Heatmap,
                    ) => Ok(redraw_if(state.zoom_heatmap(None, false))),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapZoomOut)),
                        ContentShowMode::Preview,
                    ) => Ok(redraw_if(state.chart_preview_state.zoom_with_anchor(
                        10.0,
                        0.5,
                        0.5,
                        false,
                        crate::ui::state::PreviewChartZoomMode::Uniform,
                    ))),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapResetView)),
                        ContentShowMode::Heatmap,
                    ) => Ok(redraw_if(state.reset_heatmap_view())),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapResetView)),
                        ContentShowMode::Preview,
                    ) => Ok(redraw_if(state.chart_preview_state.clear_zoom())),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapClearSelection)),
                        ContentShowMode::Heatmap,
                    ) => Ok(redraw_if(state.clear_heatmap_selection())),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapPan(Direction::Left))),
                        ContentShowMode::Heatmap,
                    ) => Ok(redraw_if(state.pan_heatmap_by(-1, 0))),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapPan(Direction::Left))),
                        ContentShowMode::Preview,
                    ) => Ok(redraw_if(state.chart_preview_state.pan_by(-10.0, 0.0))),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapPan(Direction::Right))),
                        ContentShowMode::Heatmap,
                    ) => Ok(redraw_if(state.pan_heatmap_by(1, 0))),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapPan(Direction::Right))),
                        ContentShowMode::Preview,
                    ) => Ok(redraw_if(state.chart_preview_state.pan_by(10.0, 0.0))),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapPan(Direction::Up))),
                        ContentShowMode::Heatmap,
                    ) => Ok(redraw_if(state.pan_heatmap_by(0, -1))),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapPan(Direction::Up))),
                        ContentShowMode::Preview,
                    ) => Ok(redraw_if(state.chart_preview_state.pan_by(0.0, -10.0))),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapPan(Direction::Down))),
                        ContentShowMode::Heatmap,
                    ) => Ok(redraw_if(state.pan_heatmap_by(0, 1))),
                    (
                        Some(BoundAction::Action(ContentAction::HeatmapPan(Direction::Down))),
                        ContentShowMode::Preview,
                    ) => Ok(redraw_if(state.chart_preview_state.pan_by(0.0, 10.0))),
                    (Some(BoundAction::Command(command)), _) => {
                        execute_bound_command(state, &command)
                    }
                    (Some(BoundAction::Script(script)), _) => {
                        execute_bound_script(state, &script, "keybinding script")
                    }
                    (Some(BoundAction::LuaCallback(callback_id)), _) => {
                        execute_bound_lua_callback(state, &callback_id)
                    }
                    _ => Ok(EventResult::Continue),
                }
            }
            KeyEventKind::Repeat => Ok(EventResult::Continue),
            KeyEventKind::Release => Ok(EventResult::Continue),
        },
        Event::Resize(_, _) => Ok(EventResult::Redraw),
        _ => Ok(EventResult::Continue),
    }
}
