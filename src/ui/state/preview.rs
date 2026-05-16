use std::{
    collections::{HashSet, VecDeque},
    io::BufReader,
    sync::mpsc::Sender,
};

use hdf5_metno::{ByteReader, Dataset};
use image::ImageFormat;
use ratatui::layout::Rect;
use ratatui_image::thread::{ResizeRequest, ThreadProtocol};

use crate::{
    data::{DatasetPlotingData, PreviewSelection},
    h5f::{DatasetMeta, ImageType},
    ui::{chart_math::normalized_axis_bounds, mchart::ChartItem},
};

pub const PREVIEW_CHART_VISIBLE_POINT_LIMIT: usize = 50;

pub struct ChartPreviewLoadRequest {
    pub key: ChartPreviewKey,
    pub source: ChartPreviewSource,
    pub page_state: PageState,
    pub width: u16,
    pub height: u16,
}

pub enum ChartPreviewSource {
    Dataset {
        ds: Dataset,
        selection: PreviewSelection,
    },
    ProjectedDataset {
        ds: Dataset,
        meta: Box<DatasetMeta>,
        selection: PreviewSelection,
    },
    Precomputed {
        data_preview: DatasetPlotingData,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardImageData {
    pub width: usize,
    pub height: usize,
    pub bytes: Vec<u8>,
}

pub struct ChartPreviwState {
    pub ds_loaded: Option<String>,
    pub protocol: Option<ThreadProtocol>,
    pub clipboard_image: Option<ClipboardImageData>,
    pub error: Option<String>,
    pub ds_selection: Option<PreviewSelection>,
    pub rendered_viewport: Option<PreviewChartViewport>,
    pub rendered_roi: Option<PreviewChartRoi>,
    pub pending_key: Option<ChartPreviewKey>,
    pub tx_resize_chartpreview: Sender<ResizeRequest>,
    pub tx_load_chartpreview: Sender<ChartPreviewLoadRequest>,
    pub cached_previews: VecDeque<CachedChartPreview>,
    pub viewport: Option<PreviewChartViewport>,
    pub data_bounds: Option<PreviewChartViewport>,
    pub current_data: Option<DatasetPlotingData>,
    pub roi: Option<PreviewChartRoi>,
    pub last_chart_area: Option<Rect>,
    pub last_plot_area: Option<Rect>,
    pub drag_state: Option<PreviewChartDragState>,
}

#[derive(Debug, Clone, Copy)]
pub struct PreviewChartViewport {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl PartialEq for PreviewChartViewport {
    fn eq(&self, other: &Self) -> bool {
        self.x_min.to_bits() == other.x_min.to_bits()
            && self.x_max.to_bits() == other.x_max.to_bits()
            && self.y_min.to_bits() == other.y_min.to_bits()
            && self.y_max.to_bits() == other.y_max.to_bits()
    }
}

impl Eq for PreviewChartViewport {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewChartZoomMode {
    Uniform,
    XOnly,
    YOnly,
}

#[derive(Debug, Clone, Copy)]
pub struct PreviewChartDragState {
    pub anchor_column: u16,
    pub anchor_row: u16,
    pub viewport: PreviewChartViewport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewChartRoi {
    pub start: usize,
    pub end: usize,
    pub precise: bool,
    pub selection_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewExpressionKey {
    pub group_path: String,
    pub expression: String,
    pub expression_revision: u64,
}

#[derive(Debug, Clone)]
pub struct PreviewExpressionRequest {
    pub key: PreviewExpressionKey,
    pub items: Vec<ChartItem>,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PreviewExpressionResult {
    Success {
        key: PreviewExpressionKey,
        data_preview: DatasetPlotingData,
    },
    Failure {
        key: PreviewExpressionKey,
        message: String,
    },
}

pub struct PreviewExpressionState {
    pub current_key: Option<PreviewExpressionKey>,
    pub pending_key: Option<PreviewExpressionKey>,
    pub data_preview: Option<DatasetPlotingData>,
    pub error: Option<String>,
    pub tx_load: Sender<PreviewExpressionRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageLoadKey {
    pub ds_path: String,
    pub idx: i32,
    pub window_axis: Option<ImageWindowAxis>,
    pub window_start: usize,
    pub window_len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageWindowAxis {
    Rows,
    Cols,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageWindowState {
    pub ds_path: String,
    pub axis: ImageWindowAxis,
    pub start: usize,
    pub len: usize,
    pub total: usize,
}

impl ImageWindowState {
    pub fn end(&self) -> usize {
        self.start + self.len
    }

    pub fn label(&self) -> &'static str {
        match self.axis {
            ImageWindowAxis::Rows => "rows",
            ImageWindowAxis::Cols => "cols",
        }
    }

    pub fn centered_start(total: usize, len: usize, target: usize) -> usize {
        let max_start = total.saturating_sub(len);
        target.saturating_sub(len / 2).min(max_start)
    }

    pub fn shift_by(&mut self, delta: isize) {
        let max_start = self.total.saturating_sub(self.len);
        let next = self.start as isize + delta;
        self.start = next.clamp(0, max_start as isize) as usize;
    }

    pub fn center_on(&mut self, idx: usize) {
        self.start =
            Self::centered_start(self.total, self.len, idx.min(self.total.saturating_sub(1)));
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChartPreviewKey {
    pub ds_path: String,
    pub selection: PreviewSelection,
    pub viewport: Option<PreviewChartViewport>,
    pub roi: Option<PreviewChartRoi>,
}

pub struct RawImageLoadRequest {
    pub key: ImageLoadKey,
    pub reader: BufReader<ByteReader>,
    pub format: ImageFormat,
}

pub struct VarLenImageLoadRequest {
    pub key: ImageLoadKey,
    pub dataset: Dataset,
    pub format: ImageFormat,
}

pub struct DatasetImageLoadRequest {
    pub key: ImageLoadKey,
    pub dataset: Dataset,
    pub image_type: ImageType,
    pub window: Option<ImageWindowState>,
}

pub struct ImgState {
    pub protocol: Option<ThreadProtocol>,
    pub tx_resize_img: Sender<ResizeRequest>,
    pub tx_load_imgfs: Sender<RawImageLoadRequest>,
    pub tx_load_imgfsvlen: Sender<VarLenImageLoadRequest>,
    pub tx_load_img: Sender<DatasetImageLoadRequest>,
    pub ds: Option<String>,
    pub current_key: Option<ImageLoadKey>,
    pub clipboard_image: Option<ClipboardImageData>,
    pub window: Option<ImageWindowState>,
    pub error: Option<String>,
    pub idx_to_load: i32,
    pub idx_loaded: i32,
    pub cached_images: VecDeque<CachedImage>,
    pub pending_keys: HashSet<ImageLoadKey>,
}

#[derive(Debug, Clone)]
pub struct CachedImage {
    pub key: ImageLoadKey,
    pub clipboard_image: ClipboardImageData,
}

pub const CHART_PREVIEW_CACHE_CAPACITY: usize = 6;

#[derive(Debug, Clone)]
pub struct CachedChartPreview {
    pub key: ChartPreviewKey,
    pub clipboard_image: ClipboardImageData,
    pub data_bounds: PreviewChartViewport,
    pub data_preview: DatasetPlotingData,
}

impl ImgState {
    pub fn current_request_key(&self) -> Option<ImageLoadKey> {
        self.current_key.clone()
    }

    pub fn has_cached_image(&self, key: &ImageLoadKey) -> bool {
        self.cached_images.iter().any(|entry| &entry.key == key)
    }

    pub fn touch_cached_image(&mut self, key: &ImageLoadKey) -> Option<ClipboardImageData> {
        let index = self
            .cached_images
            .iter()
            .position(|entry| &entry.key == key)?;
        let entry = self.cached_images.remove(index)?;
        let clipboard_image = entry.clipboard_image.clone();
        self.cached_images.push_back(entry);
        Some(clipboard_image)
    }

    pub fn cache_image(
        &mut self,
        key: ImageLoadKey,
        clipboard_image: ClipboardImageData,
        capacity: usize,
    ) {
        self.cached_images.retain(|entry| entry.key != key);
        self.cached_images.push_back(CachedImage {
            key,
            clipboard_image,
        });
        while self.cached_images.len() > capacity {
            self.cached_images.pop_front();
        }
    }

    pub fn begin_loading(&mut self, key: ImageLoadKey, idx_loaded: i32) {
        self.protocol = None;
        self.clipboard_image = None;
        self.error = None;
        self.ds = Some(key.ds_path.clone());
        self.current_key = Some(key.clone());
        self.idx_loaded = idx_loaded;
        self.pending_keys.insert(key);
    }
}

impl ChartPreviwState {
    pub fn current_request_key(&self) -> Option<ChartPreviewKey> {
        Some(ChartPreviewKey {
            ds_path: self.ds_loaded.clone()?,
            selection: self.ds_selection.clone()?,
            viewport: self.rendered_viewport,
            roi: self.rendered_roi,
        })
    }

    pub fn touch_cached_preview(
        &mut self,
        key: &ChartPreviewKey,
    ) -> Option<(ClipboardImageData, PreviewChartViewport, DatasetPlotingData)> {
        let index = self
            .cached_previews
            .iter()
            .position(|entry| &entry.key == key)?;
        let entry = self.cached_previews.remove(index)?;
        let clipboard_image = entry.clipboard_image.clone();
        let data_bounds = entry.data_bounds;
        let data_preview = entry.data_preview.clone();
        self.cached_previews.push_back(entry);
        Some((clipboard_image, data_bounds, data_preview))
    }

    pub fn cache_preview(
        &mut self,
        key: ChartPreviewKey,
        clipboard_image: ClipboardImageData,
        data_bounds: PreviewChartViewport,
        data_preview: DatasetPlotingData,
        capacity: usize,
    ) {
        self.cached_previews.retain(|entry| entry.key != key);
        self.cached_previews.push_back(CachedChartPreview {
            key,
            clipboard_image,
            data_bounds,
            data_preview,
        });
        while self.cached_previews.len() > capacity {
            self.cached_previews.pop_front();
        }
    }

    pub fn begin_loading(&mut self, key: ChartPreviewKey) {
        self.ds_loaded = Some(key.ds_path.clone());
        self.ds_selection = Some(key.selection.clone());
        self.rendered_viewport = key.viewport;
        self.rendered_roi = key.roi;
        self.protocol = None;
        self.clipboard_image = None;
        self.error = None;
        self.current_data = None;
        self.pending_key = Some(key);
    }

    pub fn reset_viewport(&mut self) {
        self.viewport = None;
        self.data_bounds = None;
        self.current_data = None;
        self.roi = None;
        self.last_chart_area = None;
        self.last_plot_area = None;
        self.drag_state = None;
    }

    pub fn sync_selection_identity(&mut self, ds_path: &str, selection: &PreviewSelection) {
        if self.ds_loaded.as_deref() != Some(ds_path)
            || self.ds_selection.as_ref() != Some(selection)
        {
            self.reset_viewport();
        }
    }

    pub fn sync_data_bounds(&mut self, bounds: Option<PreviewChartViewport>) {
        self.data_bounds = bounds;
        let Some(full_bounds) = self.data_bounds else {
            self.viewport = None;
            self.current_data = None;
            self.roi = None;
            self.drag_state = None;
            return;
        };
        self.viewport = match self.viewport {
            Some(viewport) => {
                let next = self.clamp_viewport(viewport, full_bounds);
                (!viewport_eq(next, full_bounds)).then_some(next)
            }
            None => None,
        };
    }

    pub fn set_current_data(&mut self, data: Option<DatasetPlotingData>) {
        self.current_data = data;
        if let Some(roi) = self.roi {
            let len = self
                .current_data
                .as_ref()
                .map(|data| data.data.len())
                .unwrap_or(0);
            if roi.start >= len || roi.end >= len {
                self.roi = None;
            }
        }
    }

    pub fn clear_roi(&mut self) -> bool {
        let had_roi = self.roi.is_some();
        self.roi = None;
        had_roi
    }

    pub fn clear_roi_or_zoom(&mut self) -> bool {
        if self.clear_roi() {
            true
        } else {
            self.clear_zoom()
        }
    }

    pub fn effective_viewport(&self) -> Option<PreviewChartViewport> {
        self.viewport.or(self.data_bounds)
    }

    pub fn has_explicit_viewport(&self) -> bool {
        self.viewport.is_some()
    }

    pub fn set_chart_area(&mut self, area: Option<Rect>) {
        self.last_chart_area = area;
        if area.is_none() {
            self.drag_state = None;
        }
    }

    pub fn set_plot_area(&mut self, area: Option<Rect>) {
        self.last_plot_area = area;
    }

    fn clamp_viewport(
        &self,
        viewport: PreviewChartViewport,
        bounds: PreviewChartViewport,
    ) -> PreviewChartViewport {
        let (x_min, x_max) =
            clamp_axis_range(viewport.x_min, viewport.x_max, bounds.x_min, bounds.x_max);
        let (y_min, y_max) =
            clamp_axis_range(viewport.y_min, viewport.y_max, bounds.y_min, bounds.y_max);
        PreviewChartViewport {
            x_min,
            x_max,
            y_min,
            y_max,
        }
    }

    fn set_explicit_viewport(&mut self, viewport: Option<PreviewChartViewport>) -> bool {
        let Some(full_bounds) = self.data_bounds else {
            return false;
        };
        let next = viewport
            .map(|value| self.clamp_viewport(value, full_bounds))
            .filter(|value| !viewport_eq(*value, full_bounds));
        if self.viewport == next {
            return false;
        }
        self.viewport = next;
        true
    }

    pub fn clear_zoom(&mut self) -> bool {
        self.set_explicit_viewport(None)
    }

    pub fn pan_by(&mut self, dx_percent: f64, dy_percent: f64) -> bool {
        let Some(current) = self.effective_viewport() else {
            return false;
        };
        let x_shift = (current.x_max - current.x_min) * dx_percent / 100.0;
        let y_shift = (current.y_max - current.y_min) * dy_percent / 100.0;
        self.set_explicit_viewport(Some(PreviewChartViewport {
            x_min: current.x_min + x_shift,
            x_max: current.x_max + x_shift,
            y_min: current.y_min + y_shift,
            y_max: current.y_max + y_shift,
        }))
    }

    pub fn zoom_with_anchor(
        &mut self,
        percent: f64,
        anchor_x_ratio: f64,
        anchor_y_ratio: f64,
        zoom_in: bool,
        mode: PreviewChartZoomMode,
    ) -> bool {
        let Some(bounds) = self.data_bounds else {
            return false;
        };
        let Some(current) = self.effective_viewport() else {
            return false;
        };

        let (x_min, x_max) = match mode {
            PreviewChartZoomMode::Uniform | PreviewChartZoomMode::XOnly => zoom_axis_range(
                current.x_min,
                current.x_max,
                bounds.x_min,
                bounds.x_max,
                anchor_x_ratio,
                percent,
                zoom_in,
            ),
            PreviewChartZoomMode::YOnly => (current.x_min, current.x_max),
        };
        let (y_min, y_max) = match mode {
            PreviewChartZoomMode::Uniform | PreviewChartZoomMode::YOnly => zoom_axis_range(
                current.y_min,
                current.y_max,
                bounds.y_min,
                bounds.y_max,
                anchor_y_ratio,
                percent,
                zoom_in,
            ),
            PreviewChartZoomMode::XOnly => (current.y_min, current.y_max),
        };
        let changed = self.set_explicit_viewport(Some(PreviewChartViewport {
            x_min,
            x_max,
            y_min,
            y_max,
        }));
        if changed {
            self.roi = None;
        }
        changed
    }

    pub fn zoom_in_at_position(
        &mut self,
        column: u16,
        row: u16,
        percent: f64,
        mode: PreviewChartZoomMode,
    ) -> bool {
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        if !point_in_rect(chart_area, column, row) {
            return false;
        }
        let relative_x = column.saturating_sub(chart_area.x) as f64;
        let relative_y = row.saturating_sub(chart_area.y) as f64;
        let x_denom = chart_area.width.saturating_sub(1).max(1) as f64;
        let y_denom = chart_area.height.saturating_sub(1).max(1) as f64;
        self.zoom_with_anchor(
            percent,
            relative_x / x_denom,
            1.0 - (relative_y / y_denom),
            true,
            mode,
        )
    }

    pub fn zoom_out_at_position(
        &mut self,
        column: u16,
        row: u16,
        percent: f64,
        mode: PreviewChartZoomMode,
    ) -> bool {
        let Some(chart_area) = self.last_plot_area else {
            return false;
        };
        if !point_in_rect(chart_area, column, row) {
            return false;
        }
        let relative_x = column.saturating_sub(chart_area.x) as f64;
        let relative_y = row.saturating_sub(chart_area.y) as f64;
        let x_denom = chart_area.width.saturating_sub(1).max(1) as f64;
        let y_denom = chart_area.height.saturating_sub(1).max(1) as f64;
        self.zoom_with_anchor(
            percent,
            relative_x / x_denom,
            1.0 - (relative_y / y_denom),
            false,
            mode,
        )
    }

    pub fn start_drag_at_position(&mut self, column: u16, row: u16) -> bool {
        if !self.chart_contains_position(column, row) || self.precise_point_mode() {
            return false;
        }
        let Some(viewport) = self.effective_viewport() else {
            return false;
        };
        self.drag_state = Some(PreviewChartDragState {
            anchor_column: column,
            anchor_row: row,
            viewport,
        });
        true
    }

    fn apply_drag_position(&mut self, column: u16, row: u16) -> bool {
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        let Some(drag_state) = self.drag_state else {
            return false;
        };
        if chart_area.width <= 1 || chart_area.height <= 1 {
            return false;
        }
        let delta_columns = column as f64 - drag_state.anchor_column as f64;
        let delta_rows = row as f64 - drag_state.anchor_row as f64;
        let x_span = drag_state.viewport.x_max - drag_state.viewport.x_min;
        let y_span = drag_state.viewport.y_max - drag_state.viewport.y_min;
        let x_shift = (delta_columns / chart_area.width.saturating_sub(1) as f64) * x_span;
        let y_shift = (delta_rows / chart_area.height.saturating_sub(1) as f64) * y_span;
        self.set_explicit_viewport(Some(PreviewChartViewport {
            x_min: drag_state.viewport.x_min - x_shift,
            x_max: drag_state.viewport.x_max - x_shift,
            y_min: drag_state.viewport.y_min + y_shift,
            y_max: drag_state.viewport.y_max + y_shift,
        }))
    }

    pub fn drag_to_position(&mut self, column: u16, row: u16) -> bool {
        let _ = (column, row);
        false
    }

    pub fn finish_drag_at_position(&mut self, column: u16, row: u16) -> bool {
        let changed = self.apply_drag_position(column, row);
        self.drag_state = None;
        changed
    }

    pub fn end_drag(&mut self) {
        self.drag_state = None;
    }

    pub fn chart_contains_position(&self, column: u16, row: u16) -> bool {
        self.last_chart_area
            .is_some_and(|chart_area| point_in_rect(chart_area, column, row))
    }

    fn selection_x_min(&self) -> f64 {
        match self.ds_selection.as_ref().map(|selection| &selection.slice) {
            Some(crate::data::SliceSelection::FromTo(start, _)) => *start as f64,
            _ => 0.0,
        }
    }

    fn visible_index_window(&self) -> Option<(usize, usize)> {
        let data = self.current_data.as_ref()?;
        let viewport = self.effective_viewport()?;
        let x_min = self.selection_x_min();
        let start = (viewport.x_min - x_min).floor().max(0.0) as usize;
        let end = (viewport.x_max - x_min)
            .ceil()
            .max(viewport.x_min - x_min)
            .min(data.data.len().saturating_sub(1) as f64) as usize;
        Some((start.min(end), end.max(start.min(end))))
    }

    fn precise_point_mode(&self) -> bool {
        let Some((start, end)) = self.visible_index_window() else {
            return false;
        };
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        let visible = end.saturating_sub(start).saturating_add(1);
        visible <= PREVIEW_CHART_VISIBLE_POINT_LIMIT
            && chart_area.width as usize >= visible.saturating_mul(2)
    }

    fn roi_at_position(&self, column: u16, row: u16) -> Option<PreviewChartRoi> {
        let chart_area = self.last_plot_area?;
        if !point_in_rect(chart_area, column, row) || chart_area.width == 0 {
            return None;
        }
        let (visible_start, visible_end) = self.visible_index_window()?;
        let visible_len = visible_end.saturating_sub(visible_start).saturating_add(1);
        if visible_len == 0 {
            return None;
        }
        let relative_col = column.saturating_sub(chart_area.x) as usize;
        if self.precise_point_mode() {
            let idx = visible_start
                + ((relative_col as f64 / chart_area.width.saturating_sub(1).max(1) as f64)
                    * visible_len.saturating_sub(1) as f64)
                    .round() as usize;
            let idx = idx.clamp(visible_start, visible_end);
            return Some(PreviewChartRoi {
                start: idx,
                end: idx,
                precise: true,
                selection_count: 1,
            });
        }
        let width = chart_area.width.max(1) as usize;
        let start = visible_start + (relative_col * visible_len) / width;
        let end = visible_start
            + (((relative_col + 1) * visible_len).div_ceil(width))
                .saturating_sub(1)
                .min(visible_len.saturating_sub(1));
        Some(PreviewChartRoi {
            start: start.min(visible_end),
            end: end.min(visible_end).max(start.min(visible_end)),
            precise: false,
            selection_count: 1,
        })
    }

    pub fn cycle_roi_at_position(&mut self, column: u16, row: u16) -> bool {
        let Some(hit) = self.roi_at_position(column, row) else {
            return false;
        };
        self.roi = match self.roi {
            None => Some(hit),
            Some(existing) if existing.selection_count < 2 => Some(PreviewChartRoi {
                start: existing.start.min(hit.start),
                end: existing.end.max(hit.end),
                precise: existing.precise && hit.precise,
                selection_count: 2,
            }),
            Some(_) => None,
        };
        true
    }

    pub fn zoom_to_roi(&mut self) -> bool {
        let Some(roi) = self.roi else {
            return false;
        };
        let Some(data) = self.current_data.as_ref() else {
            return false;
        };
        let x_min = self.selection_x_min();
        let start = roi.start.min(data.data.len().saturating_sub(1));
        let end = roi.end.min(data.data.len().saturating_sub(1)).max(start);
        let slice = &data.data[start..=end];
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        for &(_, y) in slice {
            if y.is_finite() {
                y_min = y_min.min(y);
                y_max = y_max.max(y);
            }
        }
        let x_start = x_min + start as f64;
        let x_end = x_min + end as f64 + if roi.precise { 0.0 } else { 1.0 };
        let Some((x_min, x_max)) = normalized_axis_bounds(x_start, x_end) else {
            return false;
        };
        let Some((y_min, y_max)) = normalized_axis_bounds(y_min, y_max) else {
            return false;
        };
        let changed = self.set_explicit_viewport(Some(PreviewChartViewport {
            x_min,
            x_max,
            y_min,
            y_max,
        }));
        if changed {
            self.roi = None;
        }
        changed
    }
}

fn point_in_rect(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn viewport_eq(left: PreviewChartViewport, right: PreviewChartViewport) -> bool {
    (left.x_min - right.x_min).abs() < 1e-9
        && (left.x_max - right.x_max).abs() < 1e-9
        && (left.y_min - right.y_min).abs() < 1e-9
        && (left.y_max - right.y_max).abs() < 1e-9
}

fn minimum_zoom_span(bounds_min: f64, bounds_max: f64) -> f64 {
    let span = (bounds_max - bounds_min).abs();
    span.mul_add(1e-6, f64::EPSILON).max(1e-9)
}

fn clamp_axis_range(mut start: f64, mut end: f64, bounds_min: f64, bounds_max: f64) -> (f64, f64) {
    if bounds_max <= bounds_min {
        return (bounds_min, bounds_max);
    }
    if start > end {
        std::mem::swap(&mut start, &mut end);
    }
    let bounds_span = bounds_max - bounds_min;
    let span = (end - start)
        .max(minimum_zoom_span(bounds_min, bounds_max))
        .min(bounds_span);
    if span >= bounds_span {
        return (bounds_min, bounds_max);
    }

    let mut clamped_start = start;
    let mut clamped_end = clamped_start + span;
    if clamped_start < bounds_min {
        clamped_end += bounds_min - clamped_start;
        clamped_start = bounds_min;
    }
    if clamped_end > bounds_max {
        let overflow = clamped_end - bounds_max;
        clamped_start -= overflow;
        clamped_end = bounds_max;
    }
    clamped_start = clamped_start.max(bounds_min);
    clamped_end = clamped_end.min(bounds_max);
    (clamped_start, clamped_end)
}

fn zoom_axis_range(
    current_min: f64,
    current_max: f64,
    bounds_min: f64,
    bounds_max: f64,
    anchor_ratio: f64,
    percent: f64,
    zoom_in: bool,
) -> (f64, f64) {
    let current_span = (current_max - current_min).abs();
    let bounds_span = (bounds_max - bounds_min).abs();
    if bounds_span <= f64::EPSILON {
        return (bounds_min, bounds_max);
    }

    let anchor_ratio = anchor_ratio.clamp(0.0, 1.0);
    let delta = current_span * percent / 100.0;
    let min_span = minimum_zoom_span(bounds_min, bounds_max);
    let next_span = if zoom_in {
        (current_span - 2.0 * delta).max(min_span)
    } else {
        (current_span + 2.0 * delta).min(bounds_span)
    };
    let anchor = current_min + current_span * anchor_ratio;
    let next_min = anchor - next_span * anchor_ratio;
    let next_max = next_min + next_span;
    clamp_axis_range(next_min, next_max, bounds_min, bounds_max)
}

#[derive(Clone)]
pub enum PageType {
    Image,
    Chart,
    Unpaged,
}

#[derive(Clone)]
pub struct PageState {
    pub idx: i32,
    pub paged: PageType,
    pub page_count: i32,
}

impl PageState {
    pub fn max_index(&self) -> Option<i32> {
        (self.page_count > 0).then_some(self.page_count.saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn preview_key(name: &str) -> ChartPreviewKey {
        ChartPreviewKey {
            ds_path: name.to_string(),
            selection: PreviewSelection {
                index: vec![0],
                x: 0,
                slice: crate::data::SliceSelection::All,
            },
            viewport: None,
            roi: None,
        }
    }

    fn bounds() -> PreviewChartViewport {
        PreviewChartViewport {
            x_min: 0.0,
            x_max: 1.0,
            y_min: 0.0,
            y_max: 1.0,
        }
    }

    fn clipboard_image(id: u8) -> ClipboardImageData {
        ClipboardImageData {
            width: 1,
            height: 1,
            bytes: vec![id, 0, 0, 255],
        }
    }

    fn data_preview() -> DatasetPlotingData {
        DatasetPlotingData {
            data: vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
            length: 3,
            min: 1.0,
            max: 3.0,
        }
    }

    #[test]
    fn chart_preview_cache_touches_existing_entries() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            rendered_viewport: None,
            rendered_roi: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: None,
            current_data: None,
            roi: None,
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };
        let first = preview_key("first");
        let second = preview_key("second");
        state.cache_preview(
            first.clone(),
            clipboard_image(1),
            bounds(),
            data_preview(),
            2,
        );
        state.cache_preview(
            second.clone(),
            clipboard_image(2),
            bounds(),
            data_preview(),
            2,
        );

        let (touched, cached_bounds, cached_data) = state.touch_cached_preview(&first).unwrap();

        assert_eq!(touched.bytes, clipboard_image(1).bytes);
        assert_eq!(cached_bounds, bounds());
        assert_eq!(cached_data.length, 3);
        assert_eq!(state.cached_previews.back().unwrap().key, first);
        assert_eq!(state.cached_previews.front().unwrap().key, second);
    }

    #[test]
    fn chart_preview_cache_respects_capacity() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            rendered_viewport: None,
            rendered_roi: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: None,
            current_data: None,
            roi: None,
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };

        state.cache_preview(
            preview_key("first"),
            clipboard_image(1),
            bounds(),
            data_preview(),
            2,
        );
        state.cache_preview(
            preview_key("second"),
            clipboard_image(2),
            bounds(),
            data_preview(),
            2,
        );
        state.cache_preview(
            preview_key("third"),
            clipboard_image(3),
            bounds(),
            data_preview(),
            2,
        );

        assert!(state
            .cached_previews
            .iter()
            .all(|entry| entry.key != preview_key("first")));
        assert!(state
            .cached_previews
            .iter()
            .any(|entry| entry.key == preview_key("second")));
        assert!(state
            .cached_previews
            .iter()
            .any(|entry| entry.key == preview_key("third")));
    }

    #[test]
    fn chart_preview_begin_loading_clears_active_preview_state() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: Some("stale".to_string()),
            protocol: None,
            clipboard_image: Some(clipboard_image(9)),
            error: Some("old".to_string()),
            ds_selection: None,
            rendered_viewport: Some(bounds()),
            rendered_roi: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: None,
            current_data: Some(data_preview()),
            roi: None,
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };

        let key = preview_key("fresh");
        state.begin_loading(key.clone());

        assert_eq!(state.ds_loaded, Some("fresh".to_string()));
        assert_eq!(state.ds_selection, Some(key.selection.clone()));
        assert_eq!(state.rendered_viewport, None);
        assert!(state.clipboard_image.is_none());
        assert!(state.error.is_none());
        assert_eq!(state.pending_key, Some(key));
    }

    #[test]
    fn chart_preview_current_request_key_tracks_rendered_viewport() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let viewport = PreviewChartViewport {
            x_min: 1.0,
            x_max: 5.0,
            y_min: -2.0,
            y_max: 3.0,
        };
        let key = ChartPreviewKey {
            viewport: Some(viewport),
            ..preview_key("viewported")
        };
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            rendered_viewport: None,
            rendered_roi: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: None,
            current_data: None,
            roi: None,
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };

        state.begin_loading(key.clone());

        assert_eq!(state.current_request_key(), Some(key));
    }

    #[test]
    fn chart_preview_sync_selection_identity_clears_existing_viewport() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let key = preview_key("same");
        let mut state = ChartPreviwState {
            ds_loaded: Some(key.ds_path.clone()),
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: Some(key.selection.clone()),
            rendered_viewport: None,
            rendered_roi: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: Some(bounds()),
            data_bounds: Some(bounds()),
            current_data: Some(data_preview()),
            roi: None,
            last_chart_area: Some(Rect::new(0, 0, 10, 10)),
            last_plot_area: Some(Rect::new(0, 0, 10, 10)),
            drag_state: None,
        };
        let changed_selection = PreviewSelection {
            x: 0,
            index: vec![1],
            slice: crate::data::SliceSelection::All,
        };

        state.sync_selection_identity("same", &changed_selection);

        assert!(state.viewport.is_none());
        assert!(state.data_bounds.is_none());
        assert!(state.last_chart_area.is_none());
    }

    #[test]
    fn chart_preview_zoom_with_anchor_creates_explicit_viewport() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            rendered_viewport: None,
            rendered_roi: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 10.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            current_data: None,
            roi: None,
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };

        assert!(state.zoom_with_anchor(10.0, 0.5, 0.5, true, PreviewChartZoomMode::Uniform));
        assert!(state.viewport.is_some());
        assert_ne!(state.viewport, state.data_bounds);
    }

    #[test]
    fn chart_preview_zoom_with_anchor_clears_roi() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            rendered_viewport: None,
            rendered_roi: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 10.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            current_data: Some(data_preview()),
            roi: Some(PreviewChartRoi {
                start: 0,
                end: 1,
                precise: true,
                selection_count: 1,
            }),
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };

        assert!(state.zoom_with_anchor(10.0, 0.5, 0.5, true, PreviewChartZoomMode::Uniform));
        assert!(state.roi.is_none());
    }

    #[test]
    fn chart_preview_drag_updates_viewport_before_mouse_up() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let initial_viewport = PreviewChartViewport {
            x_min: 2.0,
            x_max: 8.0,
            y_min: 1.0,
            y_max: 9.0,
        };
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            rendered_viewport: None,
            rendered_roi: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: Some(initial_viewport),
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 10.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            current_data: None,
            roi: None,
            last_chart_area: Some(Rect::new(0, 0, 10, 10)),
            last_plot_area: None,
            drag_state: None,
        };

        assert!(state.start_drag_at_position(5, 5));
        assert!(!state.drag_to_position(6, 5));
        assert_eq!(state.viewport, Some(initial_viewport));
        assert!(state.finish_drag_at_position(6, 5));
        assert_ne!(state.viewport, Some(initial_viewport));
    }

    #[test]
    fn chart_preview_roi_coarse_clicks_cycle_from_first_to_second_to_clear() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: Some(preview_key("roi").selection),
            rendered_viewport: None,
            rendered_roi: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 100.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 100.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            current_data: Some(DatasetPlotingData {
                data: (0..100).map(|i| (i as f64, i as f64)).collect(),
                length: 100,
                min: 0.0,
                max: 99.0,
            }),
            roi: None,
            last_chart_area: Some(Rect::new(5, 3, 10, 10)),
            last_plot_area: Some(Rect::new(5, 3, 10, 10)),
            drag_state: None,
        };

        assert!(state.cycle_roi_at_position(6, 4));
        let first = state.roi.expect("first roi");
        assert_eq!(first.selection_count, 1);
        assert!(first.end >= first.start);

        assert!(state.cycle_roi_at_position(13, 4));
        let second = state.roi.expect("second roi");
        assert_eq!(second.selection_count, 2);
        assert!(second.end > first.end);

        assert!(state.cycle_roi_at_position(10, 4));
        assert!(state.roi.is_none());
    }

    #[test]
    fn chart_preview_zoom_to_roi_allows_single_selection() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: Some(preview_key("roi").selection),
            rendered_viewport: None,
            rendered_roi: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 10.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            current_data: Some(data_preview()),
            roi: Some(PreviewChartRoi {
                start: 0,
                end: 2,
                precise: false,
                selection_count: 1,
            }),
            last_chart_area: Some(Rect::new(0, 0, 10, 10)),
            last_plot_area: Some(Rect::new(0, 0, 10, 10)),
            drag_state: None,
        };

        assert!(state.zoom_to_roi());
        assert!(state.viewport.is_some());
        assert!(state.roi.is_none());
    }

    #[test]
    fn chart_preview_starts_drag_when_points_are_not_selectable() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: Some(preview_key("drag").selection),
            rendered_viewport: None,
            rendered_roi: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 100.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 100.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            current_data: Some(DatasetPlotingData {
                data: (0..100).map(|i| (i as f64, i as f64)).collect(),
                length: 100,
                min: 0.0,
                max: 99.0,
            }),
            roi: None,
            last_chart_area: Some(Rect::new(5, 3, 10, 10)),
            last_plot_area: Some(Rect::new(5, 3, 10, 10)),
            drag_state: None,
        };

        assert!(state.start_drag_at_position(6, 4));
        assert!(state.drag_state.is_some());
    }

    #[test]
    fn chart_preview_blocks_drag_when_precise_points_are_visible() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: Some(preview_key("drag").selection),
            rendered_viewport: None,
            rendered_roi: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 3.0,
                y_min: 0.0,
                y_max: 3.0,
            }),
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 3.0,
                y_min: 0.0,
                y_max: 3.0,
            }),
            current_data: Some(DatasetPlotingData {
                data: (0..3).map(|i| (i as f64, i as f64)).collect(),
                length: 3,
                min: 0.0,
                max: 2.0,
            }),
            roi: Some(PreviewChartRoi {
                start: 0,
                end: 1,
                precise: true,
                selection_count: 1,
            }),
            last_chart_area: Some(Rect::new(5, 3, 10, 10)),
            last_plot_area: Some(Rect::new(5, 3, 10, 10)),
            drag_state: None,
        };

        assert!(!state.start_drag_at_position(6, 4));
        assert!(state.drag_state.is_none());
    }

    #[test]
    fn chart_preview_clear_roi_or_zoom_prefers_roi() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            rendered_viewport: None,
            rendered_roi: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: Some(bounds()),
            data_bounds: Some(bounds()),
            current_data: Some(data_preview()),
            roi: Some(PreviewChartRoi {
                start: 0,
                end: 1,
                precise: false,
                selection_count: 2,
            }),
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };

        assert!(state.clear_roi_or_zoom());
        assert!(state.roi.is_none());
        assert!(state.viewport.is_some());
        assert!(state.clear_roi_or_zoom());
        assert!(state.viewport.is_none());
    }

    #[test]
    fn image_begin_loading_clears_active_image_state() {
        let (tx_resize_img, _) = channel();
        let (tx_load_imgfs, _) = channel();
        let (tx_load_imgfsvlen, _) = channel();
        let (tx_load_img, _) = channel();
        let mut state = ImgState {
            protocol: None,
            tx_resize_img,
            tx_load_imgfs,
            tx_load_imgfsvlen,
            tx_load_img,
            ds: Some("stale".to_string()),
            current_key: None,
            clipboard_image: Some(clipboard_image(7)),
            window: None,
            error: Some("old".to_string()),
            idx_to_load: 3,
            idx_loaded: -1,
            cached_images: Default::default(),
            pending_keys: Default::default(),
        };
        let key = ImageLoadKey {
            ds_path: "fresh".to_string(),
            idx: 2,
            window_axis: None,
            window_start: 0,
            window_len: 0,
        };

        state.begin_loading(key.clone(), 2);

        assert_eq!(state.ds, Some("fresh".to_string()));
        assert_eq!(state.current_key, Some(key.clone()));
        assert!(state.clipboard_image.is_none());
        assert!(state.error.is_none());
        assert_eq!(state.idx_loaded, 2);
        assert!(state.pending_keys.contains(&key));
    }
}
