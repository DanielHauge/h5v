use hdf5_metno::File;
use image::{DynamicImage, ImageBuffer, Rgb};
use ratatui::layout::Rect;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::mpsc::{channel, Sender},
    thread,
};

use crate::{
    configure,
    data::{plot_dataset_with_cap, DatasetPlotingData, PreviewSelection, SliceSelection},
    h5f::{plot_projected_with_cap, DatasetMeta},
    ui::app::AppEvent,
};

mod derived;
mod eval;
mod expression;
mod interaction;
mod load;
mod model;
mod prompt;
mod render;
use eval::{
    dataset_ploting_data_from_points, eval_expression_at, eval_scalar_expression,
    resolve_expression_item_value, resolve_expression_load_value, validate_expression_load_ref,
    validate_expression_series_compatibility, EvaluatedExpression, ExpressionSeriesInput,
    ExpressionSeriesResolution, ResolvedExpressionItemValue, ResolvedExpressionLoad,
    ValidatedExpressionLoad,
};
use expression::{
    collect_parsed_expression_refs, parse_derived_expression, tokenize_expression, ExpressionAst,
    ExpressionObjectTarget, ExpressionRefs, ExpressionToken, ParsedExpression,
};
pub use load::{handle_mchart_load, handle_mchart_render};
use model::sanitize_chart_points;
#[allow(unused_imports)]
pub use model::{
    ChartItem, ChartItemId, ChartItemStats, ChartLodWindow, ChartSeries, ChartSource,
    ChartXAxisPolicy, DatasetChartKind, DatasetChartSource, DerivedExpressionKind,
    MultiChartLoadState, Point,
};
use prompt::{
    ExpressionPromptFocus, ExpressionPromptInputKind, ExpressionPromptMessageKind,
    ExpressionPromptMode, ExpressionPromptState, ExpressionPromptSuggestion,
    ExpressionPromptSuggestionKind, EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS,
};
#[allow(unused_imports)]
use render::chart_plot_area_in_rect;

#[derive(Debug, Clone, Copy, PartialEq)]
struct ChartViewport {
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct MultiChartItemHitbox {
    pub(super) area: Rect,
    pub(super) index: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct MultiChartEditorHitbox {
    pub(super) area: Rect,
    pub(super) name_area: Rect,
    pub(super) expression_area: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChartZoomMode {
    Uniform,
    XOnly,
    YOnly,
}

#[derive(Debug, Clone)]
struct ChartDragState {
    anchor_column: u16,
    anchor_row: u16,
    viewport: ChartViewport,
}

#[derive(Debug, Clone)]
struct PreparedChartSeries {
    label: String,
    color_slot: usize,
    points: Vec<Point>,
    is_selected: bool,
}

#[derive(Debug, Clone)]
struct PreparedChartData {
    plot_x_min: f64,
    plot_x_max: f64,
    y_min: f64,
    y_max: f64,
    series: Vec<PreparedChartSeries>,
}

#[derive(Debug, Clone)]
pub struct MultiChartRenderRequest {
    generation: u64,
    chart_area: Rect,
    width: u32,
    height: u32,
    x_axis_label: String,
    prepared: PreparedChartData,
}

#[derive(Debug, Clone)]
pub enum MultiChartRenderResult {
    Success {
        generation: u64,
        chart_area: Rect,
        width: u32,
        height: u32,
        rgb_bytes: Vec<u8>,
        plot_x_range: std::ops::Range<i32>,
        plot_y_range: std::ops::Range<i32>,
    },
    Failure {
        generation: u64,
        message: String,
    },
}
#[derive(Debug, Clone)]
pub struct CapturedMultiChartItem {
    pub source: ChartSource,
    pub initial_points: Option<Vec<Point>>,
    pub source_len: usize,
    pub load_state: MultiChartLoadState,
    pub request: Option<MultiChartLoadRequest>,
}

#[derive(Debug, Clone)]
pub struct MultiChartLoadRequest {
    pub item_id: ChartItemId,
    pub kind: MultiChartLoadKind,
    pub source: MultiChartLoadSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiChartLoadKind {
    Overview {
        generation: u64,
    },
    Detail {
        generation: u64,
        window: ChartLodWindow,
    },
}

#[derive(Debug, Clone)]
pub enum MultiChartLoadSource {
    Dataset {
        dataset: hdf5_metno::Dataset,
        selection: PreviewSelection,
    },
    CompoundLeaf {
        dataset: hdf5_metno::Dataset,
        meta: Box<DatasetMeta>,
        selection: PreviewSelection,
    },
}

#[derive(Debug, Clone)]
pub enum MultiChartLoadResult {
    Started {
        item_id: ChartItemId,
        kind: MultiChartLoadKind,
    },
    Success {
        item_id: ChartItemId,
        kind: MultiChartLoadKind,
        points: Vec<Point>,
        source_len: usize,
    },
    Failure {
        item_id: ChartItemId,
        kind: MultiChartLoadKind,
        message: String,
    },
}
pub struct MultiChartState {
    items: Vec<ChartItem>,
    pub modified: bool,
    pub height: u32,
    pub width: u32,
    pub picker: Picker,
    pub idx: usize,
    viewport: Option<ChartViewport>,
    tx_load: Sender<MultiChartLoadRequest>,
    tx_render: Sender<MultiChartRenderRequest>,
    stateful_protocol: Option<StatefulProtocol>,
    render_generation: u64,
    pending_render_generation: Option<u64>,
    render_error: Option<String>,
    next_id: u64,
    next_color_slot: usize,
    x_axis_policy: ChartXAxisPolicy,
    expression_prompt: Option<ExpressionPromptState>,
    last_chart_area: Option<Rect>,
    last_chart_panel_area: Option<Rect>,
    drag_state: Option<ChartDragState>,
    pub(super) item_hitboxes: Vec<MultiChartItemHitbox>,
    pub(super) editor_hitbox: Option<MultiChartEditorHitbox>,
}

impl MultiChartState {
    pub fn new(
        picker: Picker,
        tx_load: Sender<MultiChartLoadRequest>,
        tx_render: Sender<MultiChartRenderRequest>,
    ) -> Self {
        Self {
            items: Vec::new(),
            modified: false,
            idx: 0,
            height: 0,
            width: 0,
            picker,
            viewport: None,
            tx_load,
            tx_render,
            stateful_protocol: None,
            render_generation: 0,
            pending_render_generation: None,
            render_error: None,
            next_id: 1,
            next_color_slot: 0,
            x_axis_policy: ChartXAxisPolicy::SampleIndex,
            expression_prompt: None,
            last_chart_area: None,
            last_chart_panel_area: None,
            drag_state: None,
            item_hitboxes: Vec::new(),
            editor_hitbox: None,
        }
    }

    pub fn chart_items(&self) -> &[ChartItem] {
        &self.items
    }

    pub(crate) fn queue_chart_render(&mut self, chart_area: Rect) -> bool {
        let Some(prepared) = self.prepared_chart_data() else {
            self.pending_render_generation = None;
            self.render_error = None;
            self.stateful_protocol = None;
            self.last_chart_area = None;
            self.modified = false;
            return false;
        };
        self.render_generation = self.render_generation.saturating_add(1);
        let generation = self.render_generation;
        let request = MultiChartRenderRequest {
            generation,
            chart_area,
            width: self.width,
            height: self.height,
            x_axis_label: self.x_axis_policy.label().to_string(),
            prepared,
        };
        self.pending_render_generation = Some(generation);
        self.render_error = None;
        self.modified = false;
        if self.tx_render.send(request).is_err() {
            self.pending_render_generation = None;
            self.render_error = Some("multichart renderer unavailable".to_string());
            return false;
        }
        true
    }

    pub(crate) fn apply_render_result(&mut self, result: MultiChartRenderResult) {
        match result {
            MultiChartRenderResult::Success {
                generation,
                chart_area,
                width,
                height,
                rgb_bytes,
                plot_x_range,
                plot_y_range,
            } => {
                if generation != self.render_generation {
                    return;
                }
                self.pending_render_generation = None;
                self.render_error = None;
                self.last_chart_panel_area = Some(chart_area);
                self.last_chart_area = render::chart_plot_area_in_rect(
                    chart_area,
                    width,
                    height,
                    plot_x_range,
                    plot_y_range,
                );
                let image = ImageBuffer::<Rgb<u8>, _>::from_raw(width, height, rgb_bytes);
                let Some(image) = image else {
                    self.render_error =
                        Some("Failed to create image buffer from plot buffer".to_string());
                    self.stateful_protocol = None;
                    return;
                };
                self.stateful_protocol = Some(
                    self.picker
                        .new_resize_protocol(DynamicImage::ImageRgb8(image)),
                );
            }
            MultiChartRenderResult::Failure {
                generation,
                message,
            } => {
                if generation != self.render_generation {
                    return;
                }
                self.pending_render_generation = None;
                self.render_error = Some(message);
            }
        }
    }

    #[cfg(test)]
    pub fn source_item_count(&self, path: &str) -> usize {
        self.items
            .iter()
            .filter(|item| item.matches_path(path))
            .count()
    }

    pub fn visible_item_count(&self) -> usize {
        self.items.iter().filter(|item| item.visible).count()
    }

    pub fn loading_item_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| !matches!(item.load_state, MultiChartLoadState::Ready))
            .count()
    }

    pub fn selected_item(&self) -> Option<&ChartItem> {
        self.items.get(self.idx)
    }

    pub fn is_expression_prompt_active(&self) -> bool {
        self.expression_prompt.is_some()
    }

    pub(crate) fn refresh_expression_detail_series(
        &mut self,
        file: Option<&File>,
    ) -> Result<(), String> {
        if !configure::current_multichart_settings().derived_detail_enabled {
            for item in self
                .items
                .iter_mut()
                .filter(|item| matches!(item.source, ChartSource::DerivedExpression { .. }))
            {
                item.clear_detail_state(true);
            }
            self.modified = true;
            return Ok(());
        }
        let derived_ids = self
            .items
            .iter()
            .filter_map(|item| {
                matches!(item.source, ChartSource::DerivedExpression { .. }).then_some(item.id)
            })
            .collect::<Vec<_>>();
        for id in self.expression_recompute_order(&derived_ids)? {
            let expression = match self.item_by_id(id).map(|item| item.source.clone()) {
                Some(ChartSource::DerivedExpression { expression, .. }) => expression,
                _ => continue,
            };
            let index = self
                .items
                .iter()
                .position(|item| item.id == id)
                .ok_or_else(|| format!("Chart item ${} no longer exists", id.0))?;
            let Some(window) = self.expression_detail_window(&expression)? else {
                self.items[index].clear_detail_state(true);
                continue;
            };
            match self.evaluate_expression_with_resolution(
                &expression,
                file,
                ExpressionSeriesResolution::Active,
                false,
            ) {
                Ok(evaluated) => {
                    let points = sanitize_chart_points(evaluated.points);
                    let Some(series) = ChartSeries::from_points(points) else {
                        self.items[index].clear_detail_state(true);
                        continue;
                    };
                    self.items[index].detail_series = Some(series);
                    self.items[index].detail_window = Some(window);
                    self.items[index].pending_detail_window = None;
                    self.items[index].load_state = MultiChartLoadState::Ready;
                }
                Err(_) => {
                    self.items[index].clear_detail_state(true);
                }
            }
        }
        self.modified = true;
        Ok(())
    }

    #[cfg(test)]
    pub fn add_chart_item(
        &mut self,
        source: ChartSource,
        points: Vec<Point>,
    ) -> Option<ChartItemId> {
        self.add_chart_item_with_status(
            source,
            Some(points),
            None,
            0,
            MultiChartLoadState::Ready,
            false,
        )
    }

    pub fn add_chart_item_with_scalar(
        &mut self,
        source: ChartSource,
        points: Vec<Point>,
        scalar_value: Option<f64>,
    ) -> Option<ChartItemId> {
        self.add_chart_item_with_status(
            source,
            Some(points),
            scalar_value,
            0,
            MultiChartLoadState::Ready,
            false,
        )
    }

    pub fn add_chart_item_with_status(
        &mut self,
        source: ChartSource,
        points: Option<Vec<Point>>,
        scalar_value: Option<f64>,
        source_len: usize,
        load_state: MultiChartLoadState,
        sampled: bool,
    ) -> Option<ChartItemId> {
        let loaded_len = points.as_ref().map_or(0, Vec::len);
        let series = ChartSeries::from_points(points.unwrap_or_else(|| vec![(0.0, 0.0)]))?;
        if let Some((idx, item)) = self
            .items
            .iter_mut()
            .enumerate()
            .find(|(_, item)| item.source == source)
        {
            item.series = series;
            item.scalar_value = scalar_value;
            item.clear_detail_state(true);
            item.detail_generation = 0;
            item.source_len = if source_len == 0 {
                loaded_len.max(item.series.len())
            } else {
                source_len
            };
            item.sampled = sampled || item.source_len > loaded_len.max(1);
            item.load_state = load_state;
            item.visible = true;
            item.name = None;
            self.idx = idx;
            self.modified = true;
            return Some(item.id);
        }

        let id = ChartItemId(self.next_id);
        self.next_id += 1;
        let color_slot = self.next_color_slot;
        self.next_color_slot += 1;
        self.items.push(ChartItem {
            id,
            color_slot,
            label: source.label(),
            name: None,
            source,
            series,
            scalar_value,
            detail_series: None,
            detail_window: None,
            pending_detail_window: None,
            detail_generation: 0,
            source_len: if source_len == 0 {
                loaded_len
            } else {
                source_len
            },
            sampled: sampled || source_len > loaded_len.max(1),
            load_state,
            visible: true,
        });
        self.idx = self.items.len().saturating_sub(1);
        self.modified = true;
        Some(id)
    }

    pub fn queue_loaded_item(
        &mut self,
        mut item: CapturedMultiChartItem,
    ) -> Result<ChartItemId, String> {
        let initial_len = item.initial_points.as_ref().map_or(0, Vec::len);
        let item_id = self
            .add_chart_item_with_status(
                item.source,
                item.initial_points.take(),
                None,
                item.source_len,
                item.load_state.clone(),
                item.source_len > initial_len.max(1),
            )
            .ok_or_else(|| "Failed to add dataset to multichart".to_string())?;
        if let Some(mut request) = item.request {
            request.item_id = item_id;
            request.kind = MultiChartLoadKind::Overview { generation: 0 };
            self.queue_load(request, MultiChartLoadState::Queued);
        }
        Ok(item_id)
    }

    pub fn queue_load(&mut self, request: MultiChartLoadRequest, status: MultiChartLoadState) {
        let item_id = request.item_id;
        let kind = request.kind;
        if let Some(item) = self.items.iter_mut().find(|item| item.id == item_id) {
            item.load_state = status;
            if let MultiChartLoadKind::Detail { window, generation } = kind {
                item.pending_detail_window = Some(window);
                item.detail_generation = generation;
            }
        }
        if self.tx_load.send(request).is_err() {
            self.apply_load_failure(item_id, kind, "multichart loader unavailable".to_string());
        }
        self.modified = true;
    }

    pub fn apply_load_started(&mut self, item_id: ChartItemId, kind: MultiChartLoadKind) {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == item_id) {
            item.load_state = match kind {
                MultiChartLoadKind::Overview { .. } => MultiChartLoadState::Sampling,
                MultiChartLoadKind::Detail { .. } => MultiChartLoadState::Refining,
            };
            self.modified = true;
        }
    }

    pub fn apply_loaded_item(
        &mut self,
        item_id: ChartItemId,
        kind: MultiChartLoadKind,
        points: Vec<Point>,
        source_len: usize,
    ) -> Result<(), String> {
        let series = ChartSeries::from_points(points)
            .ok_or_else(|| format!("Loaded series for ${} had no finite points", item_id.0))?;
        let Some(item) = self.items.iter_mut().find(|item| item.id == item_id) else {
            return Ok(());
        };
        match kind {
            MultiChartLoadKind::Overview { .. } => {
                item.series = series;
                if source_len != 0 {
                    item.source_len = source_len;
                }
                item.sampled = item.source_len > item.series.len();
                item.load_state = if item.pending_detail_window.is_some() {
                    MultiChartLoadState::Refining
                } else {
                    MultiChartLoadState::Ready
                };
            }
            MultiChartLoadKind::Detail { generation, window } => {
                if generation != item.detail_generation {
                    return Ok(());
                }
                item.detail_series = Some(series);
                item.detail_window = Some(window);
                item.pending_detail_window = None;
                item.load_state = MultiChartLoadState::Ready;
            }
        }
        self.modified = true;
        Ok(())
    }

    pub fn apply_load_failure(
        &mut self,
        item_id: ChartItemId,
        kind: MultiChartLoadKind,
        message: String,
    ) {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == item_id) {
            match kind {
                MultiChartLoadKind::Overview { .. } => {
                    item.load_state = MultiChartLoadState::Error(message);
                }
                MultiChartLoadKind::Detail { generation, .. } => {
                    if generation != item.detail_generation {
                        return;
                    }
                    item.pending_detail_window = None;
                    item.load_state = MultiChartLoadState::Ready;
                }
            }
            self.modified = true;
        }
    }

    fn dataset_selection_len(source: &DatasetChartSource) -> usize {
        match source.selection.slice {
            SliceSelection::All => source
                .shape
                .get(source.selection.x)
                .copied()
                .unwrap_or_default(),
            SliceSelection::FromTo(start, end) => end.saturating_sub(start),
        }
    }

    fn detail_window_for_viewport(
        source: &DatasetChartSource,
        viewport: ChartViewport,
        sample_cap: usize,
    ) -> Option<ChartLodWindow> {
        let settings = configure::current_multichart_settings();
        let total_len = Self::dataset_selection_len(source);
        if total_len <= sample_cap {
            return None;
        }
        let viewport_start = viewport.x_min.floor().max(0.0) as usize;
        let viewport_end = viewport.x_max.ceil().max(0.0) as usize;
        let viewport_end = viewport_end.clamp(1, total_len);
        let viewport_start = viewport_start.min(viewport_end.saturating_sub(1));
        let span = viewport_end.saturating_sub(viewport_start).max(1);
        if span >= total_len {
            return None;
        }
        let pad = ((span as f64) * settings.detail_padding_ratio).round() as usize;
        let start = viewport_start.saturating_sub(pad);
        let end = viewport_end.saturating_add(pad).min(total_len);
        Some(ChartLodWindow {
            start,
            end: end.max(start + 1),
            sample_cap,
        })
    }

    fn viewport_sample_cap(&self) -> Option<usize> {
        let settings = configure::current_multichart_settings();
        if !settings.detail_enabled {
            return None;
        }
        let chart_area = self.last_chart_area?;
        Some(
            (chart_area.width as usize)
                .saturating_mul(settings.detail_samples_per_column)
                .clamp(settings.detail_min_samples, settings.detail_max_samples),
        )
    }

    pub(crate) fn schedule_viewport_detail_loads(&mut self, file: Option<&File>) {
        let Some(viewport) = self.viewport else {
            return;
        };
        let Some(sample_cap) = self.viewport_sample_cap() else {
            return;
        };
        let Some(file) = file else {
            return;
        };
        let mut requests = Vec::new();
        for item in self.items.iter_mut().filter(|item| item.visible) {
            let Some(source) = item.source.dataset_source().cloned() else {
                continue;
            };
            let Some(window) = Self::detail_window_for_viewport(&source, viewport, sample_cap)
            else {
                item.clear_detail_state(true);
                continue;
            };
            if item.detail_window == Some(window) || item.pending_detail_window == Some(window) {
                continue;
            }
            item.clear_detail_state(true);
            let Ok(dataset) = file.dataset(&source.dataset_path) else {
                continue;
            };
            let request_source = match source.kind {
                DatasetChartKind::Dataset => MultiChartLoadSource::Dataset {
                    dataset,
                    selection: source.selection,
                },
                DatasetChartKind::CompoundLeaf => continue,
            };
            requests.push((item.id, window, request_source));
        }
        for (item_id, window, source) in requests {
            let generation = self
                .items
                .iter()
                .find(|item| item.id == item_id)
                .map(|item| item.detail_generation.saturating_add(1))
                .unwrap_or(1);
            self.queue_load(
                MultiChartLoadRequest {
                    item_id,
                    kind: MultiChartLoadKind::Detail { generation, window },
                    source,
                },
                MultiChartLoadState::Refining,
            );
        }
    }

    pub fn move_up(&mut self) {
        self.idx = self.idx.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        self.idx = self
            .idx
            .saturating_add(1)
            .clamp(0, self.items.len().saturating_sub(1));
    }

    pub fn toggle_selected_visible(&mut self) {
        if let Some(item) = self.items.get_mut(self.idx) {
            item.visible = !item.visible;
            self.modified = true;
        }
    }

    pub fn set_selected_visible(&mut self, visible: bool) {
        if let Some(item) = self.items.get_mut(self.idx) {
            if item.visible != visible {
                item.visible = visible;
                self.modified = true;
            }
        }
    }

    pub fn clear_selected(&mut self) -> Result<(), String> {
        if self.idx < self.items.len() {
            let selected_id = self.items[self.idx].id;
            let selected_ref = format!("${}", selected_id.0);
            let direct_dependents = self.direct_expression_dependents_of(selected_id);
            if !direct_dependents.is_empty() {
                let refs = direct_dependents
                    .into_iter()
                    .map(|id| format!("${}", id.0))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(format!(
                    "Cannot delete {selected_ref}; it is used by {refs}"
                ));
            }
        }
        if self.idx < self.items.len() {
            self.items.remove(self.idx);
            self.idx = self.idx.clamp(0, self.items.len().saturating_sub(1));
            if self.items.is_empty() {
                self.clear_zoom();
                self.stateful_protocol = None;
                self.last_chart_panel_area = None;
            }
            self.modified = true;
        }
        Ok(())
    }

    pub fn clear_all(&mut self) {
        self.items.clear();
        self.idx = 0;
        self.clear_zoom();
        self.stateful_protocol = None;
        self.last_chart_panel_area = None;
        self.modified = true;
    }

    pub fn create_expression_derived_command(
        &mut self,
        expression: String,
        file: Option<&File>,
    ) -> Result<ChartItemId, String> {
        self.create_expression_derived_with_file(expression, file)
    }

    pub fn add_dataset_reference_command(
        &mut self,
        dataset_spec: &str,
        file: Option<&File>,
    ) -> Result<ChartItemId, String> {
        let normalized = dataset_spec.trim();
        if normalized.is_empty() {
            return Err("Dataset reference cannot be empty".to_string());
        }
        let prefixed = if normalized.starts_with("load(") {
            normalized.to_string()
        } else if let Some((path, selectors)) = normalized.split_once('[') {
            format!("load({path})[{selectors}")
        } else {
            format!("load({normalized})")
        };
        let Some(series_ref) = Self::raw_dataset_reference(&prefixed)? else {
            return Err(format!(
                "Dataset reference '{}' must look like load(/path) or load(/path)[..,0]",
                dataset_spec
            ));
        };
        let file = file.ok_or_else(|| {
            "Adding a dataset by path requires an open file handle, but no file is loaded"
                .to_string()
        })?;
        let ExpressionObjectTarget::AbsolutePath(path) = &series_ref.target;
        let dataset = file.dataset(path).map_err(|error| {
            format!(
                "Dataset reference {} could not be opened: {}",
                series_ref.render(),
                error
            )
        })?;
        let shape = dataset.shape();
        let selection = series_ref.to_series_preview_selection(&shape)?;
        let source = ChartSource::DatasetSelection(DatasetChartSource {
            dataset_path: dataset.name(),
            display_path: dataset.name(),
            selection: selection.clone(),
            shape,
            kind: DatasetChartKind::Dataset,
        });
        self.queue_loaded_item(CapturedMultiChartItem {
            source,
            initial_points: None,
            source_len: 0,
            load_state: MultiChartLoadState::Queued,
            request: Some(MultiChartLoadRequest {
                item_id: ChartItemId(0),
                kind: MultiChartLoadKind::Overview { generation: 0 },
                source: MultiChartLoadSource::Dataset { dataset, selection },
            }),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod derived_tests;
#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod prompt_tests;
#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod tests;
#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod viewport_tests;
