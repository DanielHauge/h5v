use hdf5_metno::{Dataset, Hyperslab, Selection, SliceOrIndex};
use ratatui::crossterm::event::{Event, KeyEventKind};

use crate::{
    error::AppError,
    h5f::{format_dataset_value_for_edit, write_dataset_value_from_text, DatasetMeta, Node},
    ui::{
        edit::perform_edit,
        state::{AppState, AppToast, ContentShowMode},
    },
};

use super::{
    keymap::{content_action, ContentAction, Direction},
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

fn selected_content_edit_request(
    state: &mut AppState<'_>,
) -> Result<ContentEditRequest, EventResult> {
    let tree_item = state.treeview[state.tree_view_cursor].node.clone();
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

    let selection = match state.content_mode {
        ContentShowMode::Preview => {
            if meta.image.is_some() {
                return Err(EventResult::Toast(
                    AppToast::Warning("Image previews are not editable".to_string()),
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

    let edited_content = perform_edit(
        state,
        request.content.clone(),
        Some(&request.edit_name_hint),
    )?;
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
    state.chart_preview_state.error = None;
    state.img_state.ds = None;
    state.img_state.current_key = None;
    state.img_state.protocol = None;
    state.img_state.error = None;
    state.acknowledge_file_write();

    Ok(EventResult::Toast(
        AppToast::Info("Updated content value".to_string()),
        true,
    ))
}

pub fn handle_normal_content_event(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match (content_action(&key_event), state.content_mode) {
                (Some(ContentAction::Move(Direction::Left, amount)), ContentShowMode::Matrix) => {
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
                (Some(ContentAction::Move(Direction::Right, amount)), ContentShowMode::Matrix) => {
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
                (Some(ContentAction::Move(Direction::Up, amount)), ContentShowMode::Matrix) => {
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
                (Some(ContentAction::Move(Direction::Down, amount)), ContentShowMode::Matrix) => {
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
                (Some(ContentAction::Move(Direction::Down, amount)), ContentShowMode::Preview) => {
                    state.down(amount)
                }
                (Some(ContentAction::Move(Direction::Up, amount)), ContentShowMode::Preview) => {
                    state.up(amount)
                }
                (Some(ContentAction::Move(Direction::Right, amount)), ContentShowMode::Preview) => {
                    state.right(amount as isize)
                }
                (Some(ContentAction::Move(Direction::Left, amount)), ContentShowMode::Preview) => {
                    state.left(amount as isize)
                }
                (Some(ContentAction::Edit), _) => {
                    let request = match selected_content_edit_request(state) {
                        Ok(request) => request,
                        Err(event_result) => return Ok(event_result),
                    };
                    apply_content_edit_request(state, &request)
                }
                (Some(ContentAction::Copy), ContentShowMode::Matrix) => Ok(EventResult::Copying),
                _ => Ok(EventResult::Continue),
            },
            KeyEventKind::Repeat => Ok(EventResult::Continue),
            KeyEventKind::Release => Ok(EventResult::Continue),
        },
        Event::Resize(_, _) => Ok(EventResult::Redraw),
        _ => Ok(EventResult::Continue),
    }
}
