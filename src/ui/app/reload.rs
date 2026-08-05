use std::{cell::RefCell, rc::Rc};

use crate::{
    configure::registry::ContentModeHandle,
    error::AppError,
    h5f::{self, HasPath, Node},
    ui::state::{self, AppState, AttributeCursor, Focus, MatrixViewState, TreeSelectionState},
};

type Result<T> = std::result::Result<T, AppError>;

#[derive(Clone)]
struct SelectedNodeSnapshot {
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

fn selection_state(snapshot: &ReloadSnapshot) -> Option<TreeSelectionState> {
    snapshot
        .selected_node
        .as_ref()
        .map(|node| TreeSelectionState {
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

fn clear_preview_state(state: &mut AppState<'_>, snapshot: &ReloadSnapshot) {
    state.content_generation = state.content_generation.wrapping_add(1);
    state.content_preview_state.pending_key = None;
    state.content_preview_state.error = None;
    state.content_preview_state.cached.clear();
    state.matrix_viewport_state.pending_key = None;
    state.matrix_viewport_state.error = None;
    state.matrix_viewport_state.cached.clear();
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
    state.chart_preview_state.rendered_mode = None;
    state.chart_preview_state.rendered_viewport = None;
    state.chart_preview_state.rendered_size = None;
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
    state.tree_load_generation = state.tree_load_generation.wrapping_add(1);
    state.navigation_generation = state.navigation_generation.wrapping_add(1);
    state.drain_tree_loads();
    state.drain_navigation_loads();
    state.drain_content_previews();
    state.drain_matrix_viewports();
    let snapshot = snapshot_reload_state(state);
    let file_path = state.file_watch.path.clone();
    let linked = state.file_watch.linked;
    let previous_requested_open_mode = state.requested_open_mode;
    let target_open_mode = previous_requested_open_mode.with_write(write);

    clear_preview_state(state, &snapshot);
    state.treeview.clear();
    state.searcher = None;
    let old_root = std::mem::replace(&mut state.root, placeholder_root(&file_path));
    state.tree_view_cursor = 0;
    state.compute_tree_view();
    let old_file = state.file.take();
    let old_snapshot_file = state.snapshot_file.take();
    drop(old_root);
    drop(old_snapshot_file);
    if let Some(old_file) = old_file {
        old_file.close().map_err(|e| {
            AppError::Hdf5(hdf5_metno::Error::from(format!(
                "Failed to close HDF5 file '{}' before reload: {}",
                file_path, e
            )))
        })?;
    }

    let reopened = match h5f::H5F::open(file_path.clone(), linked, target_open_mode) {
        Ok(reopened) => reopened,
        Err(target_error) => {
            let fallback = h5f::H5F::open(file_path.clone(), linked, previous_requested_open_mode).map_err(
                |fallback_error| {
                    AppError::Hdf5(hdf5_metno::Error::from(format!(
                        "Failed to reopen HDF5 file '{}' in {} mode after reload error (reload error: {}; fallback error: {})",
                        file_path,
                        previous_requested_open_mode.label(),
                        target_error,
                        fallback_error
                    )))
                },
            )?;
            state.file = Some(fallback.file);
            state.root = fallback.root;
            state.readonly = fallback.resolved_open_mode.readonly();
            state.requested_open_mode = fallback.requested_open_mode;
            state.resolved_open_mode = fallback.resolved_open_mode;
            state.snapshot_file = fallback.snapshot_file;
            state.focus = snapshot.focus.clone();
            state.show_tree_view = snapshot.show_tree_view;
            state.content_mode = snapshot.content_mode.clone();
            state.pending_tree_expansions = snapshot.expanded_paths.clone();
            state.pending_tree_selection = snapshot.selected_path.clone();
            state.pending_tree_selection_state = selection_state(&snapshot);
            state.resume_tree_requests()?;
            state.sync_file_watch();
            state.request_tree_children(state.root.clone());
            return Err(AppError::Hdf5(hdf5_metno::Error::from(format!(
                "Failed to reopen HDF5 file '{}' in {} mode: {}",
                file_path,
                target_open_mode.label(),
                target_error
            ))));
        }
    };

    state.file = Some(reopened.file);
    state.root = reopened.root;
    state.readonly = reopened.resolved_open_mode.readonly();
    state.requested_open_mode = reopened.requested_open_mode;
    state.resolved_open_mode = reopened.resolved_open_mode;
    state.snapshot_file = reopened.snapshot_file;
    state.focus = snapshot.focus.clone();
    state.show_tree_view = snapshot.show_tree_view;
    state.content_mode = snapshot.content_mode.clone();

    state.pending_tree_expansions = snapshot.expanded_paths.clone();
    state.pending_tree_selection = snapshot.selected_path.clone();
    state.pending_tree_selection_state = selection_state(&snapshot);
    state.resume_tree_requests()?;
    state.sync_file_watch();
    state.request_tree_children(state.root.clone());

    let reloaded_path = state.file_watch.path.clone();
    let readonly = state.readonly;
    crate::configure::dispatch_lua_event(state, "builtin.event.file_reloaded", |lua| {
        let event = lua.create_table()?;
        event.set("path", reloaded_path.clone())?;
        event.set("readonly", readonly)?;
        Ok(event)
    })?;

    Ok(match state.resolved_open_mode {
        h5f::ResolvedOpenMode::Write => "Reloaded file in write mode".to_string(),
        h5f::ResolvedOpenMode::ReadSwmr => "Reloaded file in SWMR read-only mode".to_string(),
        h5f::ResolvedOpenMode::ReadSnapshot => "Reloaded snapshot from source file".to_string(),
        h5f::ResolvedOpenMode::ReadOnly => "Reloaded file".to_string(),
    })
}
