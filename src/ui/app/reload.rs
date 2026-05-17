use std::{cell::RefCell, rc::Rc};

use crate::{
    configure::registry::ContentModeHandle,
    error::AppError,
    h5f::{self, HasPath, Node},
    ui::state::{self, AppState, AttributeCursor, Focus, MatrixViewState},
};

type Result<T> = std::result::Result<T, AppError>;

#[derive(Clone)]
struct SelectedNodeSnapshot {
    path: String,
    selected_dim: usize,
    selected_x: usize,
    selected_row: usize,
    selected_col: usize,
    line_offset: usize,
    col_offset: isize,
    selected_indexes: Vec<usize>,
    attributes_view_cursor: AttributeCursor,
}

#[derive(Clone)]
struct ReloadSnapshot {
    root_selected: bool,
    selected_path: Option<String>,
    expanded_paths: Vec<String>,
    selected_node: Option<SelectedNodeSnapshot>,
    focus: Focus,
    show_tree_view: bool,
    content_mode: ContentModeHandle,
    page_state: state::PageState,
    matrix_view_state: MatrixViewState,
    img_idx_to_load: i32,
}

fn normalized_node_path(path: &str) -> &str {
    path.trim_start_matches('/')
}

fn same_node_path(lhs: &str, rhs: &str) -> bool {
    normalized_node_path(lhs) == normalized_node_path(rhs)
}

fn collect_expanded_paths(node: &Rc<RefCell<h5f::H5FNode>>, out: &mut Vec<String>) {
    let node_ref = node.borrow();
    for child in &node_ref.children {
        let child_ref = child.borrow();
        if child_ref.is_expandable() && child_ref.expanded {
            out.push(child_ref.node.path());
        }
        drop(child_ref);
        collect_expanded_paths(child, out);
    }
}

fn snapshot_selected_node(state: &AppState<'_>) -> Option<SelectedNodeSnapshot> {
    let tree_item = state.treeview.get(state.tree_view_cursor)?;
    let node = tree_item.node.borrow();
    Some(SelectedNodeSnapshot {
        path: node.node.path(),
        selected_dim: node.selected_dim,
        selected_x: node.selected_x,
        selected_row: node.selected_row,
        selected_col: node.selected_col,
        line_offset: node.line_offset,
        col_offset: node.col_offset,
        selected_indexes: node.selected_indexes.clone(),
        attributes_view_cursor: node.attributes_view_cursor.clone(),
    })
}

fn snapshot_reload_state(state: &AppState<'_>) -> ReloadSnapshot {
    let mut expanded_paths = Vec::new();
    collect_expanded_paths(&state.root, &mut expanded_paths);
    expanded_paths.sort_by_key(|path| normalized_node_path(path).matches('/').count());

    ReloadSnapshot {
        root_selected: state.tree_view_cursor == 0,
        selected_path: state.selected_tree_path(),
        expanded_paths,
        selected_node: snapshot_selected_node(state),
        focus: state.focus.clone(),
        show_tree_view: state.show_tree_view,
        content_mode: state.content_mode.clone(),
        page_state: state.page_state.clone(),
        matrix_view_state: state.matrix_view_state.clone(),
        img_idx_to_load: state.img_state.idx_to_load,
    }
}

fn restore_tree_selection(state: &mut AppState<'_>, snapshot: &ReloadSnapshot) {
    if snapshot.root_selected {
        state.tree_view_cursor = 0;
        return;
    }

    let Some(selected_path) = snapshot.selected_path.as_deref() else {
        state.tree_view_cursor = 0;
        return;
    };

    if let Some((idx, _)) = state
        .treeview
        .iter()
        .enumerate()
        .find(|(_, item)| same_node_path(&item.node.borrow().node.path(), selected_path))
    {
        state.tree_view_cursor = idx;
        return;
    }

    let mut fallback = selected_path.to_string();
    while let Some((prefix, _)) = normalized_node_path(&fallback).rsplit_once('/') {
        if prefix.is_empty() {
            break;
        }
        fallback = prefix.to_string();
        if let Some((idx, _)) = state
            .treeview
            .iter()
            .enumerate()
            .find(|(_, item)| same_node_path(&item.node.borrow().node.path(), &fallback))
        {
            state.tree_view_cursor = idx;
            return;
        }
    }

    state.tree_view_cursor = 0;
}

fn restore_selected_node_state(state: &mut AppState<'_>, snapshot: &ReloadSnapshot) {
    let Some(selected_snapshot) = snapshot.selected_node.as_ref() else {
        return;
    };
    let Some(tree_item) = state.treeview.get(state.tree_view_cursor) else {
        return;
    };
    if !same_node_path(
        &tree_item.node.borrow().node.path(),
        &selected_snapshot.path,
    ) {
        return;
    }

    let mut node = tree_item.node.borrow_mut();
    let shape = match &node.node {
        Node::Dataset(_, meta) => meta.shape.clone(),
        _ => Vec::new(),
    };
    let rank = shape.len();
    node.sync_selection_rank(rank);
    for ((dst, src), dim_len) in node
        .selected_indexes
        .iter_mut()
        .zip(selected_snapshot.selected_indexes.iter().copied())
        .zip(shape.iter().copied())
    {
        *dst = src.min(dim_len.saturating_sub(1));
    }
    node.selected_dim = node.selected_dim.min(rank.saturating_sub(1));
    node.selected_x = selected_snapshot.selected_x.min(rank.saturating_sub(1));
    node.selected_row = selected_snapshot.selected_row.min(rank.saturating_sub(1));
    node.selected_col = if rank > 1 {
        selected_snapshot.selected_col.min(rank.saturating_sub(1))
    } else {
        0
    };
    node.selected_dim = selected_snapshot.selected_dim.min(rank.saturating_sub(1));
    node.line_offset = selected_snapshot.line_offset;
    node.col_offset = selected_snapshot.col_offset.max(0);
    node.attributes_view_cursor = selected_snapshot.attributes_view_cursor.clone();
}

fn clear_preview_state(state: &mut AppState<'_>, snapshot: &ReloadSnapshot) {
    state.clear_preview_debounce();
    state.page_state = snapshot.page_state.clone();
    state.matrix_view_state = snapshot.matrix_view_state.clone();
    state.img_state.protocol = None;
    state.img_state.clipboard_image = None;
    state.img_state.ds = None;
    state.img_state.current_key = None;
    state.img_state.window = None;
    state.img_state.idx_loaded = -1;
    state.img_state.idx_to_load = snapshot.img_idx_to_load;
    state.img_state.error = None;
    state.img_state.cached_images.clear();
    state.img_state.pending_keys.clear();
    state.chart_preview_state.ds_loaded = None;
    state.chart_preview_state.protocol = None;
    state.chart_preview_state.clipboard_image = None;
    state.chart_preview_state.error = None;
    state.chart_preview_state.ds_selection = None;
    state.chart_preview_state.rendered_viewport = None;
    state.chart_preview_state.pending_key = None;
    state.chart_preview_state.cached_previews.clear();
    state.chart_preview_state.reset_viewport();
    state.preview_expression_state.current_key = None;
    state.preview_expression_state.pending_key = None;
    state.preview_expression_state.data_preview = None;
    state.preview_expression_state.error = None;
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
    state.heatmap_render.drag_state = None;
    state.heatmap_render.page_window = None;
    state.heatmap_render.cached_pages.clear();
    state.heatmap_render.pending_keys.clear();
}

fn placeholder_root(path: &str) -> Rc<RefCell<h5f::H5FNode>> {
    Rc::new(RefCell::new(h5f::H5FNode::new(Node::Broken(
        path.to_string(),
    ))))
}

pub(super) fn reload_current_file(state: &mut AppState<'_>, write: bool) -> Result<String> {
    let snapshot = snapshot_reload_state(state);
    let file_path = state.file_watch.path.clone();
    let linked = state.file_watch.linked;
    let previous_write = !state.readonly;

    clear_preview_state(state, &snapshot);
    state.treeview.clear();
    state.searcher = None;
    let old_root = std::mem::replace(&mut state.root, placeholder_root(&file_path));
    state.tree_view_cursor = 0;
    state.compute_tree_view();
    let old_file = state.file.take();
    drop(old_root);
    if let Some(old_file) = old_file {
        old_file.close().map_err(|e| {
            AppError::Hdf5(hdf5_metno::Error::from(format!(
                "Failed to close HDF5 file '{}' before reload: {}",
                file_path, e
            )))
        })?;
    }

    let reopened = match h5f::H5F::open(file_path.clone(), linked, write) {
        Ok(reopened) => reopened,
        Err(target_error) => {
            let fallback = h5f::H5F::open(file_path.clone(), linked, previous_write).map_err(
                |fallback_error| {
                    AppError::Hdf5(hdf5_metno::Error::from(format!(
                        "Failed to reopen HDF5 file '{}' in {:?} mode after reload error (reload error: {}; fallback error: {})",
                        file_path,
                        if previous_write { "write" } else { "read-only" },
                        target_error,
                        fallback_error
                    )))
                },
            )?;
            state.file = Some(fallback.file);
            state.root = fallback.root;
            state.readonly = !previous_write;
            state.focus = snapshot.focus.clone();
            state.show_tree_view = snapshot.show_tree_view;
            state.content_mode = snapshot.content_mode.clone();
            for path in &snapshot.expanded_paths {
                let relative = normalized_node_path(path);
                if relative.is_empty() {
                    continue;
                }
                let _ = state.root.borrow_mut().expand_path(relative);
            }
            state.compute_tree_view();
            restore_tree_selection(state, &snapshot);
            restore_selected_node_state(state, &snapshot);
            state.compute_tree_view();
            restore_tree_selection(state, &snapshot);
            state.sync_file_watch();
            return Err(AppError::Hdf5(hdf5_metno::Error::from(format!(
                "Failed to reopen HDF5 file '{}' in {} mode: {}",
                file_path,
                if write { "write" } else { "read-only" },
                target_error
            ))));
        }
    };

    state.file = Some(reopened.file);
    state.root = reopened.root;
    state.readonly = !write;
    state.focus = snapshot.focus.clone();
    state.show_tree_view = snapshot.show_tree_view;
    state.content_mode = snapshot.content_mode.clone();

    for path in &snapshot.expanded_paths {
        let relative = normalized_node_path(path);
        if relative.is_empty() {
            continue;
        }
        let _ = state.root.borrow_mut().expand_path(relative);
    }

    state.compute_tree_view();
    restore_tree_selection(state, &snapshot);
    restore_selected_node_state(state, &snapshot);
    state.compute_tree_view();
    restore_tree_selection(state, &snapshot);
    state.sync_file_watch();

    let reloaded_path = state.file_watch.path.clone();
    let readonly = state.readonly;
    crate::configure::dispatch_lua_event(state, "builtin.event.file_reloaded", |lua| {
        let event = lua.create_table()?;
        event.set("path", reloaded_path.clone())?;
        event.set("readonly", readonly)?;
        Ok(event)
    })?;

    Ok(if write {
        "Reloaded file in write mode".to_string()
    } else {
        "Reloaded file".to_string()
    })
}
