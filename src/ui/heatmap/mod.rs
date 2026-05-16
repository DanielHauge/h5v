use std::{
    fmt::Display,
    sync::mpsc::{channel, Sender},
    thread,
};

use hdf5_metno::{Dataset, Hyperslab, Selection, SliceOrIndex};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    Frame,
};
use ratatui_image::StatefulImage;

use crate::{
    error::AppError,
    h5f::{DatasetMeta, H5FNode, Node},
    ui::{
        app::{AppEvent, HeatmapLoadedResult},
        page_scroll::PageDisplayInfo,
        render::MatrixRenderType,
        state::{
            AppState, HeatmapLoadPriority, HeatmapLoadRequest, HeatmapPageAxis, HeatmapPageWindow,
            HeatmapRenderKey, HeatmapViewport,
        },
    },
};

use super::{
    dims::render_dim_selector, matrix::render_not_yet_implemented,
    std_comp_render::render_empty_dataset,
};

mod load;
mod panels;
mod render;
#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests;

const SMART_HEATMAP_PAGE_MIN_CLIPPED_FRACTION: f32 = 0.5;
const HEATMAP_PREFETCH_RADIUS: i32 = 2;
pub(crate) const HEATMAP_CACHE_CAPACITY: usize = 5;

trait HeatmapNumber: Copy + Display {
    fn to_f64(self) -> f64;
}

impl HeatmapNumber for f64 {
    fn to_f64(self) -> f64 {
        self
    }
}

impl HeatmapNumber for u64 {
    fn to_f64(self) -> f64 {
        self as f64
    }
}

impl HeatmapNumber for i64 {
    fn to_f64(self) -> f64 {
        self as f64
    }
}

pub fn render_heatmap(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    state: &mut AppState,
) -> Result<(), AppError> {
    let (ds, attr) = match node.node.clone() {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => {
            render_not_yet_implemented(f, area, "Heatmap mode is only available for datasets");
            return Ok(());
        }
    };
    if attr.is_empty() {
        render_empty_dataset(f, area);
        return Ok(());
    }

    let ds_path = attr.virtual_path().unwrap_or(&ds.name()).to_string();

    match attr.matrixable {
        Some(MatrixRenderType::Float64)
        | Some(MatrixRenderType::Uint64)
        | Some(MatrixRenderType::Int64) => {
            render_heatmap_with_dataset(f, area, &ds, &attr, node, state, &ds_path)?;
        }
        Some(MatrixRenderType::Enum)
        | Some(MatrixRenderType::Strings)
        | Some(MatrixRenderType::ByteArray)
        | Some(MatrixRenderType::Opaque)
        | Some(MatrixRenderType::Compound)
        | None => render_not_yet_implemented(
            f,
            area,
            "Heatmap mode currently supports numeric datasets and numeric compound leaves",
        ),
    }

    Ok(())
}

fn render_heatmap_with_dataset(
    f: &mut Frame,
    area: &Rect,
    ds: &Dataset,
    attr: &DatasetMeta,
    node: &mut H5FNode,
    state: &mut AppState,
    ds_path: &str,
) -> Result<(), AppError> {
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 1,
        vertical: 1,
    });
    if area_inner.width < 4 || area_inner.height < 4 {
        return Ok(());
    }

    let shape_len = attr.shape.len();
    node.sync_selection_rank(shape_len);
    if attr.shape.iter().filter(|len| **len > 1).count() < 2 {
        render_not_yet_implemented(
            f,
            area,
            "Heatmap mode requires at least two non-singleton dimensions",
        );
        return Ok(());
    }

    normalize_heatmap_axes(node, &attr.shape);

    let source_rows = attr.shape[node.selected_row];
    let source_cols = attr.shape[node.selected_col];
    if state
        .heatmap_render
        .current_key
        .as_ref()
        .is_some_and(|key| key.ds_path != ds_path)
    {
        clear_heatmap_render_state(state);
    }

    let base_viewport = heatmap_base_viewport(state, source_rows, source_cols);
    let layout = HeatmapLayout::new(area_inner);
    let show_profile_panel = state.heatmap_render.selected_line.is_some()
        || state.heatmap_render.current_line_profile.is_some();
    let (heatmap_body, profile_area) = split_heatmap_body(layout.body, show_profile_panel);
    let heatmap_body_inner = panels::heatmap_frame_inner(&heatmap_body);
    let header_page_window = compute_heatmap_page_window(
        ds_path,
        base_viewport.row_len,
        base_viewport.col_len,
        heatmap_body_inner,
        state.image_cell_size,
        state.heatmap_render.page_window.as_ref(),
    );
    let header_page_info = header_page_window.as_ref().map(|window| {
        let (range_start, range_end) = window.current_range();
        PageDisplayInfo {
            title: "Page",
            current: window.page.max(0) as usize,
            total: window.page_count.max(1) as usize,
            range_start,
            range_end,
            total_items: window.total,
            unit: window.label(),
        }
    });
    render_dim_selector(
        f,
        &layout.header_dims,
        node,
        &attr.shape,
        true,
        header_page_info.as_ref(),
        "Slice selection",
        None,
    )?;
    if let Some(settings_area) = layout.settings {
        panels::render_heatmap_settings(f, &settings_area, state);
    }

    render_heatmap_body(
        f,
        &heatmap_body,
        layout.sidebar.as_ref(),
        attr,
        node,
        state,
        ds,
        ds_path,
        header_page_window,
    )?;
    panels::render_heatmap_region_panel(f, &layout.region, attr, node, state);
    if let Some(profile_area) = profile_area {
        panels::render_heatmap_profile_slot(f, &profile_area, state)?;
    }
    Ok(())
}

fn clear_heatmap_render_state(state: &mut AppState<'_>) {
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

fn heatmap_base_viewport(
    state: &mut AppState<'_>,
    source_rows: usize,
    source_cols: usize,
) -> HeatmapViewport {
    let base_viewport = clamp_heatmap_viewport(
        state.heatmap_render.viewport.unwrap_or(HeatmapViewport {
            row_start: 0,
            row_len: source_rows.max(1),
            col_start: 0,
            col_len: source_cols.max(1),
        }),
        source_rows,
        source_cols,
    );
    state.heatmap_render.viewport = if base_viewport.row_start == 0
        && base_viewport.col_start == 0
        && base_viewport.row_len == source_rows.max(1)
        && base_viewport.col_len == source_cols.max(1)
    {
        None
    } else {
        Some(base_viewport)
    };
    base_viewport
}

struct HeatmapLayout {
    settings: Option<Rect>,
    header_dims: Rect,
    region: Rect,
    body: Rect,
    sidebar: Option<Rect>,
}

impl HeatmapLayout {
    fn new(area_inner: Rect) -> Self {
        let sections =
            Layout::vertical([Constraint::Length(8), Constraint::Min(4)]).split(area_inner);
        let header_split = if sections[0].width >= 42 {
            Layout::horizontal([Constraint::Length(22), Constraint::Min(12)]).split(sections[0])
        } else {
            Layout::horizontal([Constraint::Length(0), Constraint::Min(12)]).split(sections[0])
        };
        let header_right =
            Layout::vertical([Constraint::Length(4), Constraint::Min(3)]).split(header_split[1]);
        if sections[1].width >= 24 {
            let split =
                Layout::horizontal([Constraint::Min(4), Constraint::Length(20)]).split(sections[1]);
            Self {
                settings: (header_split[0].width > 0).then_some(header_split[0]),
                header_dims: header_right[0],
                region: header_right[1],
                body: split[0],
                sidebar: Some(split[1]),
            }
        } else {
            Self {
                settings: (header_split[0].width > 0).then_some(header_split[0]),
                header_dims: header_right[0],
                region: header_right[1],
                body: sections[1],
                sidebar: None,
            }
        }
    }
}

fn split_heatmap_body(area: Rect, show_profile: bool) -> (Rect, Option<Rect>) {
    if !show_profile || area.height < 12 {
        return (area, None);
    }
    let profile_height = 7u16.min(area.height.saturating_sub(5));
    let split =
        Layout::vertical([Constraint::Min(4), Constraint::Length(profile_height)]).split(area);
    (split[0], Some(split[1]))
}

fn queue_heatmap_load(
    state: &mut AppState<'_>,
    render_key: &HeatmapRenderKey,
    ds: &Dataset,
    attr: &DatasetMeta,
) -> Result<(), AppError> {
    if state.heatmap_render.pending_keys.contains(render_key) {
        return Ok(());
    }
    state.heatmap_render.pending_keys.insert(render_key.clone());
    state
        .heatmap_render
        .tx_load_heatmap
        .send(HeatmapLoadRequest {
            key: render_key.clone(),
            dataset: ds.clone(),
            attr: attr.clone(),
            priority: HeatmapLoadPriority::Current,
        })?;
    Ok(())
}

struct CachedHeatmapRenderData {
    slice_summary: crate::ui::state::HeatmapSliceSummary,
    legend_summary: crate::ui::state::HeatmapLegendSummary,
    viewport_selection: crate::ui::state::HeatmapRegionSelection,
    selection: Option<crate::ui::state::HeatmapRegionSelection>,
    line_profile: Option<crate::ui::state::HeatmapLineProfile>,
}

fn render_cached_heatmap_protocol(
    f: &mut Frame,
    heatmap_inner: Rect,
    entry: &mut crate::ui::state::HeatmapCachedPage,
) -> CachedHeatmapRenderData {
    let result = CachedHeatmapRenderData {
        slice_summary: entry.slice_summary.clone(),
        legend_summary: entry.legend_summary.clone(),
        viewport_selection: entry.viewport_selection.clone(),
        selection: entry.selection.clone(),
        line_profile: entry.line_profile.clone(),
    };
    f.render_stateful_widget(StatefulImage::default(), heatmap_inner, &mut entry.protocol);
    result
}

fn apply_cached_heatmap_entry(
    f: &mut Frame,
    sidebar_area: Option<&Rect>,
    state: &mut AppState<'_>,
    render_data: CachedHeatmapRenderData,
) -> Result<(), AppError> {
    let CachedHeatmapRenderData {
        slice_summary,
        legend_summary,
        viewport_selection,
        selection,
        line_profile,
    } = render_data;
    state.heatmap_viewport_region = Some(viewport_selection.clone());
    state.heatmap_region = selection
        .clone()
        .or_else(|| Some(viewport_selection.clone()));
    state.heatmap_render.current_selection = selection;
    state.heatmap_render.current_line_profile = line_profile;
    state.heatmap_render.current_legend_summary = Some(legend_summary.clone());
    state.heatmap_render.current_slice_summary = Some(slice_summary);
    if let Some(sidebar_area) = sidebar_area {
        panels::render_heatmap_sidebar(f, sidebar_area, state, &legend_summary)?;
    }
    Ok(())
}

fn normalize_heatmap_axes(node: &mut H5FNode, shape: &[usize]) {
    let selectable_dims: Vec<usize> = shape
        .iter()
        .enumerate()
        .filter(|(_, v)| **v > 1)
        .map(|(i, _)| i)
        .collect();
    for (i, selected_index) in node.selected_indexes.iter_mut().enumerate() {
        if !selectable_dims.contains(&i) {
            *selected_index = 0;
        }
    }
    if !selectable_dims.contains(&node.selected_row) {
        node.selected_row = selectable_dims[0];
    }
    if !selectable_dims.contains(&node.selected_col) || node.selected_col == node.selected_row {
        node.selected_col = selectable_dims
            .iter()
            .copied()
            .find(|dim| *dim != node.selected_row)
            .unwrap_or(node.selected_row);
    }
    if node.selected_dim == node.selected_row || node.selected_dim == node.selected_col {
        node.selected_dim = selectable_dims
            .iter()
            .copied()
            .find(|dim| *dim != node.selected_row && *dim != node.selected_col)
            .unwrap_or(0);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_heatmap_body(
    f: &mut Frame,
    area: &Rect,
    sidebar_area: Option<&Rect>,
    attr: &DatasetMeta,
    node: &mut H5FNode,
    state: &mut AppState,
    ds: &Dataset,
    ds_path: &str,
    page_window: Option<HeatmapPageWindow>,
) -> Result<(), AppError> {
    let previous_key = state.heatmap_render.current_key.clone();
    let heatmap_inner = panels::heatmap_frame_inner(area);
    if heatmap_inner.width == 0 || heatmap_inner.height == 0 {
        return Ok(());
    }

    let source_rows = attr.shape[node.selected_row];
    let source_cols = attr.shape[node.selected_col];
    let base_viewport = clamp_heatmap_viewport(
        state.heatmap_render.viewport.unwrap_or(HeatmapViewport {
            row_start: 0,
            row_len: source_rows.max(1),
            col_start: 0,
            col_len: source_cols.max(1),
        }),
        source_rows,
        source_cols,
    );
    state.heatmap_render.page_window = page_window.clone();
    let page_range = page_window.as_ref().map(|window| window.current_range());
    let ((row_start, row_end), (col_start, col_end)) =
        page_ranges(page_window.as_ref(), base_viewport);
    let visible_rows = row_end.saturating_sub(row_start).max(1);
    let visible_cols = col_end.saturating_sub(col_start).max(1);
    let viewport_rows = usize::from(heatmap_inner.height).min(visible_rows).max(1);
    let viewport_cols = usize::from(heatmap_inner.width).min(visible_cols).max(1);

    state.matrix_view_state.row_offset = 0;
    state.matrix_view_state.col_offset = 0;
    state.matrix_view_state.rows_currently_available = viewport_rows;
    state.matrix_view_state.cols_currently_available = viewport_cols;
    if let Some(line) = state.heatmap_render.selected_line {
        state.heatmap_render.selected_cells = Some(line.bounds());
    }
    if state.heatmap_render.selected_cells.is_some_and(|selected| {
        selected.row_start < row_start
            || selected.row_end >= row_end
            || selected.col_start < col_start
            || selected.col_end >= col_end
    }) {
        state.clear_heatmap_selection();
    }

    let render_key = HeatmapRenderKey {
        ds_path: ds_path.to_string(),
        width: heatmap_inner.width,
        height: heatmap_inner.height,
        cell_width: state.image_cell_size.0,
        cell_height: state.image_cell_size.1,
        viewport: state.heatmap_render.viewport,
        page_axis: page_window.as_ref().map(|window| window.axis),
        page_start: page_range.map_or(0, |range| range.0),
        page_len: page_range.map_or(0, |range| range.1.saturating_sub(range.0)),
        selected_row: node.selected_row,
        selected_col: node.selected_col,
        selected_indexes: node.selected_indexes.clone(),
        selected_cells: state.heatmap_render.selected_cells,
        line_selection: state.heatmap_render.selected_line,
        settings: state.heatmap_render.settings.clone(),
    };
    state.heatmap_render.current_key = Some(render_key.clone());
    let is_loading = state.heatmap_render.pending_keys.contains(&render_key);
    panels::render_heatmap_frame(f, area, is_loading);
    render::populate_viewport_hitboxes(state, heatmap_inner, viewport_rows, viewport_cols);

    let cached_pos = state
        .heatmap_render
        .cached_pages
        .iter()
        .position(|entry| entry.key == render_key);
    if let Some(cache_index) = cached_pos {
        if let Some(entry) = state.heatmap_render.cached_pages.get_mut(cache_index) {
            let render_data = render_cached_heatmap_protocol(f, heatmap_inner, entry);
            apply_cached_heatmap_entry(f, sidebar_area, state, render_data)?;
            schedule_heatmap_prefetch(state, ds, attr, &render_key, page_window.as_ref())?;
        } else {
            queue_heatmap_load(state, &render_key, ds, attr)?;
            if let Some(previous_key) = previous_key.as_ref() {
                if let Some(previous_entry) = state
                    .heatmap_render
                    .cached_pages
                    .iter_mut()
                    .find(|entry| entry.key == *previous_key)
                {
                    let render_data =
                        render_cached_heatmap_protocol(f, heatmap_inner, previous_entry);
                    apply_cached_heatmap_entry(f, sidebar_area, state, render_data)?;
                }
            }
        }
    } else {
        queue_heatmap_load(state, &render_key, ds, attr)?;
        if let Some(previous_key) = previous_key.as_ref() {
            if let Some(previous_entry) = state
                .heatmap_render
                .cached_pages
                .iter_mut()
                .find(|entry| entry.key == *previous_key)
            {
                let render_data = render_cached_heatmap_protocol(f, heatmap_inner, previous_entry);
                apply_cached_heatmap_entry(f, sidebar_area, state, render_data)?;
            }
        }
    }

    Ok(())
}

fn build_heatmap_selection(
    selected_row: usize,
    selected_col: usize,
    selected_indexes: &[usize],
    shape: &[usize],
    row_range: (usize, usize),
    col_range: (usize, usize),
) -> Selection {
    let slice = shape
        .iter()
        .enumerate()
        .map(|(dim, len)| {
            if dim == selected_row {
                SliceOrIndex::SliceTo {
                    start: row_range.0,
                    step: 1,
                    end: row_range.1.min(*len),
                    block: 1,
                }
            } else if dim == selected_col {
                SliceOrIndex::SliceTo {
                    start: col_range.0,
                    step: 1,
                    end: col_range.1.min(*len),
                    block: 1,
                }
            } else {
                SliceOrIndex::Index(selected_indexes.get(dim).copied().unwrap_or_default())
            }
        })
        .collect::<Vec<_>>();
    Selection::Hyperslab(Hyperslab::from(slice))
}

fn compute_heatmap_page_window(
    ds_path: &str,
    source_rows: usize,
    source_cols: usize,
    area: Rect,
    image_cell_size: (u16, u16),
    current: Option<&HeatmapPageWindow>,
) -> Option<HeatmapPageWindow> {
    let viewport_width = area.width.max(1) as f32 * image_cell_size.0.max(1) as f32;
    let viewport_height = area.height.max(1) as f32 * image_cell_size.1.max(1) as f32;
    let viewport_aspect = viewport_width / viewport_height;
    let candidate = if (source_cols as f32 / source_rows.max(1) as f32) > viewport_aspect {
        let len = ((source_rows as f32 * viewport_aspect).floor() as usize).clamp(1, source_cols);
        (len < source_cols).then_some((HeatmapPageAxis::Cols, source_cols, len))
    } else {
        let len = ((source_cols as f32 / viewport_aspect).floor() as usize).clamp(1, source_rows);
        (len < source_rows).then_some((HeatmapPageAxis::Rows, source_rows, len))
    }?;

    let (axis, total, len) = candidate;
    let clipped_fraction = 1.0 - (len as f32 / total.max(1) as f32);
    if clipped_fraction < SMART_HEATMAP_PAGE_MIN_CLIPPED_FRACTION {
        return None;
    }

    let step = (len / 2).max(1);
    let page_count = if total <= len {
        1
    } else {
        1 + total.saturating_sub(len).div_ceil(step)
    } as i32;
    let mut window = HeatmapPageWindow {
        ds_path: ds_path.to_string(),
        axis,
        len,
        total,
        page: 0,
        page_count,
    };
    if let Some(existing) = current {
        if existing.ds_path == ds_path && existing.axis == axis && existing.total == total {
            window.page = existing.page.clamp(0, page_count.saturating_sub(1));
        }
    }

    Some(window)
}

fn clamp_heatmap_viewport(
    mut viewport: HeatmapViewport,
    source_rows: usize,
    source_cols: usize,
) -> HeatmapViewport {
    viewport.row_len = viewport.row_len.clamp(1, source_rows.max(1));
    viewport.col_len = viewport.col_len.clamp(1, source_cols.max(1));
    viewport.row_start = viewport
        .row_start
        .min(source_rows.saturating_sub(viewport.row_len));
    viewport.col_start = viewport
        .col_start
        .min(source_cols.saturating_sub(viewport.col_len));
    viewport
}

fn page_ranges(
    page_window: Option<&HeatmapPageWindow>,
    base_viewport: HeatmapViewport,
) -> ((usize, usize), (usize, usize)) {
    match page_window {
        Some(window) => {
            let (start, end) = window.current_range();
            match window.axis {
                HeatmapPageAxis::Rows => (
                    (
                        base_viewport.row_start + start,
                        base_viewport.row_start + end,
                    ),
                    (
                        base_viewport.col_start,
                        base_viewport.col_start + base_viewport.col_len,
                    ),
                ),
                HeatmapPageAxis::Cols => (
                    (
                        base_viewport.row_start,
                        base_viewport.row_start + base_viewport.row_len,
                    ),
                    (
                        base_viewport.col_start + start,
                        base_viewport.col_start + end,
                    ),
                ),
            }
        }
        None => (
            (
                base_viewport.row_start,
                base_viewport.row_start + base_viewport.row_len,
            ),
            (
                base_viewport.col_start,
                base_viewport.col_start + base_viewport.col_len,
            ),
        ),
    }
}

fn schedule_heatmap_prefetch(
    state: &mut AppState<'_>,
    ds: &Dataset,
    attr: &DatasetMeta,
    current_key: &HeatmapRenderKey,
    page_window: Option<&HeatmapPageWindow>,
) -> Result<(), AppError> {
    let Some(window) = page_window else {
        state.heatmap_render.pending_keys.clear();
        return Ok(());
    };
    for delta in -HEATMAP_PREFETCH_RADIUS..=HEATMAP_PREFETCH_RADIUS {
        if delta == 0 {
            continue;
        }
        let page = window.page + delta;
        if page < 0 || page >= window.page_count {
            continue;
        }
        let (page_start, page_end) = window.range_for_page(page);
        let mut key = current_key.clone();
        key.page_start = page_start;
        key.page_len = page_end.saturating_sub(page_start);
        if state
            .heatmap_render
            .cached_pages
            .iter()
            .any(|entry| entry.key == key)
            || state.heatmap_render.pending_keys.contains(&key)
        {
            continue;
        }
        state.heatmap_render.pending_keys.insert(key.clone());
        state
            .heatmap_render
            .tx_load_heatmap
            .send(HeatmapLoadRequest {
                key,
                dataset: ds.clone(),
                attr: attr.clone(),
                priority: HeatmapLoadPriority::Prefetch,
            })?;
    }
    Ok(())
}

pub fn handle_heatmap_load(tx_events: Sender<AppEvent>) -> Sender<HeatmapLoadRequest> {
    let (tx_load, rx_load) = channel::<HeatmapLoadRequest>();
    thread::spawn(move || loop {
        if let Ok(req) = rx_load.recv() {
            let mut batch = vec![req];
            while let Ok(queued) = rx_load.try_recv() {
                batch.push(queued);
            }
            let selected_index = batch
                .iter()
                .rposition(|request| matches!(request.priority, HeatmapLoadPriority::Current))
                .unwrap_or(batch.len().saturating_sub(1));
            let req = batch.swap_remove(selected_index);
            for dropped in batch {
                let _ = tx_events.send(AppEvent::HeatmapLoad(HeatmapLoadedResult::Dropped {
                    key: dropped.key,
                }));
            }
            let result = load::build_heatmap_page(&req.dataset, &req.attr, &req.key);
            let event = match result {
                Ok(page) => AppEvent::HeatmapLoad(HeatmapLoadedResult::Success { page }),
                Err(error) => AppEvent::HeatmapLoad(HeatmapLoadedResult::Failure {
                    key: req.key,
                    message: error.to_string(),
                }),
            };
            let _ = tx_events.send(event);
        }
    });
    tx_load
}
