use hdf5_metno::File;
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
    h5f::{plot_projected, plot_projected_with_cap, DatasetMeta},
    ui::app::AppEvent,
};

mod eval;
mod expression;
mod interaction;
mod model;
mod prompt;
mod render;
use eval::{
    dataset_ploting_data_from_points, eval_expression_at, resolve_expression_item_value,
    resolve_expression_scalar_values, resolve_expression_series_values,
    validate_expression_series_compatibility, EvaluatedExpression, ExpressionSeriesInput,
    ExpressionSeriesResolution,
};
use expression::{
    collect_expression_input_ids, collect_parsed_expression_refs, parse_derived_expression,
    tokenize_expression, ExpressionObjectTarget, ExpressionRefs, ExpressionToken, ParsedExpression,
};
use model::sanitize_chart_points;
#[allow(unused_imports)]
pub use model::{
    ChartItem, ChartItemId, ChartItemStats, ChartLodWindow, ChartSeries, ChartSource,
    ChartXAxisPolicy, DatasetChartKind, DatasetChartSource, DerivedExpressionKind,
    MultiChartLoadState, Point,
};
use prompt::{
    ExpressionPromptInputKind, ExpressionPromptMessageKind, ExpressionPromptMode,
    ExpressionPromptState, ExpressionPromptSuggestion, ExpressionPromptSuggestionKind,
    EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS,
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

pub fn handle_mchart_load(tx_events: Sender<AppEvent>) -> Sender<MultiChartLoadRequest> {
    let (tx_load, rx_load) = channel::<MultiChartLoadRequest>();
    thread::spawn(move || loop {
        let Ok(request) = rx_load.recv() else {
            return;
        };
        let _ = tx_events.send(AppEvent::MultiChartLoad(MultiChartLoadResult::Started {
            item_id: request.item_id,
            kind: request.kind,
        }));
        let result = match (&request.kind, request.source) {
            (
                MultiChartLoadKind::Overview { .. },
                MultiChartLoadSource::Dataset { dataset, selection },
            ) => plot_dataset_with_cap(
                &dataset,
                &selection,
                configure::current_multichart_settings().overview_max_samples,
            )
            .map(|preview| MultiChartLoadResult::Success {
                item_id: request.item_id,
                kind: request.kind,
                points: preview.data,
                source_len: preview.length,
            })
            .map_err(|error| format!("Failed loading sampled series: {error}")),
            (
                MultiChartLoadKind::Overview { .. },
                MultiChartLoadSource::CompoundLeaf {
                    dataset,
                    meta,
                    selection,
                },
            ) => plot_projected(&dataset, meta.as_ref(), &selection)
                .map(|preview| MultiChartLoadResult::Success {
                    item_id: request.item_id,
                    kind: request.kind,
                    points: preview.data,
                    source_len: preview.length,
                })
                .map_err(|error| format!("Failed loading sampled series: {error}")),
            (
                MultiChartLoadKind::Detail { window, .. },
                MultiChartLoadSource::Dataset { dataset, selection },
            ) => {
                let detail_selection = selection_with_window(&selection, window.start, window.end);
                plot_dataset_with_cap(&dataset, &detail_selection, window.sample_cap)
                    .map(|preview| MultiChartLoadResult::Success {
                        item_id: request.item_id,
                        kind: request.kind,
                        points: offset_points(preview.data, window.start),
                        source_len: 0,
                    })
                    .map_err(|error| format!("Failed loading viewport detail: {error}"))
            }
            (
                MultiChartLoadKind::Detail { window, .. },
                MultiChartLoadSource::CompoundLeaf {
                    dataset,
                    meta,
                    selection,
                },
            ) => {
                let detail_selection = selection_with_window(&selection, window.start, window.end);
                plot_projected_with_cap(
                    &dataset,
                    meta.as_ref(),
                    &detail_selection,
                    window.sample_cap,
                )
                .map(|preview| MultiChartLoadResult::Success {
                    item_id: request.item_id,
                    kind: request.kind,
                    points: offset_points(preview.data, window.start),
                    source_len: 0,
                })
                .map_err(|error| format!("Failed loading viewport detail: {error}"))
            }
        };
        let _ = tx_events.send(AppEvent::MultiChartLoad(match result {
            Ok(result) => result,
            Err(message) => MultiChartLoadResult::Failure {
                item_id: request.item_id,
                kind: request.kind,
                message,
            },
        }));
    });
    tx_load
}

fn selection_with_window(
    selection: &PreviewSelection,
    start: usize,
    end: usize,
) -> PreviewSelection {
    let base_start = match selection.slice {
        SliceSelection::All => 0,
        SliceSelection::FromTo(base_start, _) => base_start,
    };
    PreviewSelection {
        index: selection.index.clone(),
        x: selection.x,
        slice: SliceSelection::FromTo(base_start + start, base_start + end),
    }
}

fn offset_points(points: Vec<Point>, start: usize) -> Vec<Point> {
    points
        .into_iter()
        .map(|(x, y)| (x + start as f64, y))
        .collect()
}

pub struct MultiChartState {
    items: Vec<ChartItem>,
    pub modified: bool,
    pub height: u32,
    pub width: u32,
    pub plot_buffer: Vec<u8>,
    pub picker: Picker,
    pub idx: usize,
    viewport: Option<ChartViewport>,
    tx_load: Sender<MultiChartLoadRequest>,
    stateful_protocol: Option<StatefulProtocol>,
    next_id: u64,
    next_color_slot: usize,
    x_axis_policy: ChartXAxisPolicy,
    expression_prompt: Option<ExpressionPromptState>,
    last_chart_area: Option<Rect>,
    drag_state: Option<ChartDragState>,
}

impl MultiChartState {
    pub fn new(picker: Picker, tx_load: Sender<MultiChartLoadRequest>) -> Self {
        Self {
            items: Vec::new(),
            modified: false,
            idx: 0,
            height: 0,
            width: 0,
            plot_buffer: Vec::new(),
            picker,
            viewport: None,
            tx_load,
            stateful_protocol: None,
            next_id: 1,
            next_color_slot: 0,
            x_axis_policy: ChartXAxisPolicy::SampleIndex,
            expression_prompt: None,
            last_chart_area: None,
            drag_state: None,
        }
    }

    pub fn chart_items(&self) -> &[ChartItem] {
        &self.items
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

    fn item_by_id(&self, id: ChartItemId) -> Option<&ChartItem> {
        self.items.iter().find(|item| item.id == id)
    }

    fn raw_dataset_reference(
        expression: &str,
    ) -> Result<Option<expression::ExpressionSeriesRef>, String> {
        let tokens = tokenize_expression(expression)?;
        let Some(ExpressionToken::SeriesRef(series_ref)) = tokens.first() else {
            return Ok(None);
        };
        if tokens.len() != 1 {
            return Ok(None);
        }
        if !matches!(series_ref.target, ExpressionObjectTarget::AbsolutePath(_))
            || series_ref.attr_name.is_some()
        {
            return Ok(None);
        }
        Ok(Some(series_ref.clone()))
    }

    #[cfg(test)]
    fn create_expression_derived(&mut self, expression: String) -> Result<ChartItemId, String> {
        self.create_expression_derived_with_file(expression, None)
    }

    pub fn evaluate_expression_preview(
        &self,
        expression: &str,
        file: Option<&File>,
    ) -> Result<DatasetPlotingData, String> {
        let evaluated = self.evaluate_expression_with_file(expression, file)?;
        dataset_ploting_data_from_points(evaluated.points)
    }

    pub fn capture_expression_chart_item(
        &self,
        expression: &str,
        file: Option<&File>,
    ) -> Result<(ChartSource, Vec<Point>), String> {
        let evaluated = self.evaluate_expression_with_file(expression, file)?;
        let points = sanitize_chart_points(evaluated.points);
        if points.is_empty() {
            return Err("Expression resolved to no finite points".to_string());
        }
        let len = points.len();
        let source = ChartSource::DerivedExpression {
            expression: expression.to_string(),
            input_ids: evaluated.input_ids,
            len,
            kind: evaluated.kind,
        };
        Ok((source, points))
    }

    fn create_expression_derived_with_file(
        &mut self,
        expression: String,
        file: Option<&File>,
    ) -> Result<ChartItemId, String> {
        if Self::raw_dataset_reference(&expression)?.is_some() {
            return self.add_dataset_reference_command(&expression, file);
        }
        let (source, series) = self.build_expression_chart_item(&expression, file)?;
        let points = series.points.clone();
        let id = self
            .add_chart_item(source, points)
            .ok_or_else(|| "Failed to create expression-derived chart".to_string())?;
        self.refresh_expression_detail_series(file)?;
        Ok(id)
    }

    fn build_expression_chart_item(
        &self,
        expression: &str,
        file: Option<&File>,
    ) -> Result<(ChartSource, ChartSeries), String> {
        let evaluated = self.evaluate_expression_with_file(expression, file)?;
        let points = sanitize_chart_points(evaluated.points);
        if points.is_empty() {
            return Err("Expression resolved to no finite points".to_string());
        }
        let len = points.len();
        let source = ChartSource::DerivedExpression {
            expression: expression.to_string(),
            input_ids: evaluated.input_ids,
            len,
            kind: evaluated.kind,
        };
        let series = ChartSeries::from_points(points)
            .ok_or_else(|| "Expression resolved to no finite points".to_string())?;
        Ok((source, series))
    }

    fn direct_expression_dependents_of(&self, id: ChartItemId) -> Vec<ChartItemId> {
        let mut dependents = self
            .items
            .iter()
            .filter_map(|item| item.source.input_ids().contains(&id).then_some(item.id))
            .collect::<Vec<_>>();
        dependents.sort_by_key(|id| id.0);
        dependents
    }

    fn transitive_expression_dependents_of(&self, id: ChartItemId) -> Vec<ChartItemId> {
        let mut pending = self.direct_expression_dependents_of(id);
        let mut seen = HashSet::new();
        let mut ordered = Vec::new();
        while let Some(next) = pending.pop() {
            if seen.insert(next) {
                ordered.push(next);
                pending.extend(self.direct_expression_dependents_of(next));
            }
        }
        ordered.sort_by_key(|dep_id| dep_id.0);
        ordered
    }

    fn validate_expression_rewire(
        &self,
        id: ChartItemId,
        previous_dependents: &[ChartItemId],
        new_input_ids: &[ChartItemId],
    ) -> Result<(), String> {
        if new_input_ids.contains(&id) {
            return Err(format!("Series ${} cannot depend on itself", id.0));
        }
        let blocked = previous_dependents
            .iter()
            .copied()
            .find(|dependent_id| new_input_ids.contains(dependent_id));
        if let Some(blocked) = blocked {
            return Err(format!(
                "Updating ${} would create a dependency cycle through ${}",
                id.0, blocked.0
            ));
        }
        Ok(())
    }

    fn expression_recompute_order(
        &self,
        affected_ids: &[ChartItemId],
    ) -> Result<Vec<ChartItemId>, String> {
        if affected_ids.is_empty() {
            return Ok(Vec::new());
        }

        let affected = affected_ids.iter().copied().collect::<HashSet<_>>();
        let mut indegree = HashMap::<ChartItemId, usize>::new();
        let mut edges = HashMap::<ChartItemId, Vec<ChartItemId>>::new();
        for id in affected_ids {
            let item = self
                .item_by_id(*id)
                .ok_or_else(|| format!("Chart item ${} no longer exists", id.0))?;
            let mut local_indegree = 0usize;
            for dependency in item.source.input_ids() {
                if affected.contains(dependency) {
                    local_indegree += 1;
                    edges.entry(*dependency).or_default().push(*id);
                }
            }
            indegree.insert(*id, local_indegree);
        }

        let mut ready = indegree
            .iter()
            .filter_map(|(id, degree)| (*degree == 0).then_some(*id))
            .collect::<BTreeSet<_>>();
        let mut ordered = Vec::with_capacity(affected_ids.len());

        while let Some(id) = ready.iter().next().copied() {
            ready.remove(&id);
            ordered.push(id);

            if let Some(dependents) = edges.get(&id) {
                for dependent in dependents {
                    if let Some(entry) = indegree.get_mut(dependent) {
                        *entry = entry.saturating_sub(1);
                        if *entry == 0 {
                            ready.insert(*dependent);
                        }
                    }
                }
            }
        }

        if ordered.len() != affected_ids.len() {
            return Err(
                "Expression dependency cycle blocked recomputing dependent series".to_string(),
            );
        }
        Ok(ordered)
    }

    fn recompute_expression_dependents(
        &mut self,
        affected_ids: Vec<ChartItemId>,
        file: Option<&File>,
    ) -> Result<(), String> {
        for id in self.expression_recompute_order(&affected_ids)? {
            let expression = match self.item_by_id(id).map(|item| item.source.clone()) {
                Some(ChartSource::DerivedExpression { expression, .. }) => expression,
                Some(_) => continue,
                None => return Err(format!("Chart item ${} no longer exists", id.0)),
            };
            let (source, series) = self
                .build_expression_chart_item(&expression, file)
                .map_err(|error| format!("Failed to recompute ${}: {}", id.0, error))?;
            let index = self
                .items
                .iter()
                .position(|item| item.id == id)
                .ok_or_else(|| format!("Chart item ${} no longer exists", id.0))?;
            self.items[index].label = source.label();
            self.items[index].source = source;
            self.items[index].series = series;
        }
        Ok(())
    }

    fn update_expression_item_with_file(
        &mut self,
        id: ChartItemId,
        expression: String,
        file: Option<&File>,
    ) -> Result<(), String> {
        if Self::raw_dataset_reference(&expression)?.is_some() {
            let item_id = self.add_dataset_reference_command(&expression, file)?;
            if item_id != id {
                let index = self
                    .items
                    .iter()
                    .position(|item| item.id == id)
                    .ok_or_else(|| format!("Chart item ${} no longer exists", id.0))?;
                self.items.remove(index);
                self.idx = self.idx.clamp(0, self.items.len().saturating_sub(1));
            }
            return Ok(());
        }
        let original_items = self.items.clone();
        let original_idx = self.idx;
        let original_modified = self.modified;
        let previous_dependents = self.transitive_expression_dependents_of(id);
        let (source, series) = self.build_expression_chart_item(&expression, file)?;
        self.validate_expression_rewire(id, &previous_dependents, source.input_ids())?;
        let index = self
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| format!("Chart item ${} no longer exists", id.0))?;
        self.items[index].label = source.label();
        self.items[index].source = source;
        self.items[index].series = series;
        self.idx = index;
        let result = self.recompute_expression_dependents(previous_dependents, file);
        match result {
            Ok(()) => {
                self.refresh_expression_detail_series(file)?;
                self.modified = true;
                Ok(())
            }
            Err(error) => {
                self.items = original_items;
                self.idx = original_idx;
                self.modified = original_modified;
                Err(error)
            }
        }
    }

    fn evaluate_expression_with_file(
        &self,
        expression: &str,
        file: Option<&File>,
    ) -> Result<EvaluatedExpression, String> {
        self.evaluate_expression_with_resolution(
            expression,
            file,
            ExpressionSeriesResolution::Overview,
            true,
        )
    }

    fn evaluate_expression_with_resolution(
        &self,
        expression: &str,
        file: Option<&File>,
        resolution: ExpressionSeriesResolution,
        allow_external_series: bool,
    ) -> Result<EvaluatedExpression, String> {
        let tokens = tokenize_expression(expression)?;
        let parsed = parse_derived_expression(&tokens)?;
        let mut refs = ExpressionRefs::default();
        collect_parsed_expression_refs(&parsed, &mut refs);
        refs.item_refs.sort_by(|lhs, rhs| {
            lhs.id
                .0
                .cmp(&rhs.id.0)
                .then_with(|| match (&lhs.slice, &rhs.slice) {
                    (None, None) => std::cmp::Ordering::Equal,
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (Some(lhs), Some(rhs)) => lhs.start.cmp(&rhs.start).then(lhs.end.cmp(&rhs.end)),
                })
        });
        refs.item_refs.dedup();
        refs.series_refs
            .sort_by_key(|series_ref| series_ref.render());
        refs.series_refs.dedup();
        refs.scalar_refs
            .sort_by_key(|scalar_ref| scalar_ref.render());
        refs.scalar_refs.dedup();
        if refs.item_refs.is_empty() && refs.series_refs.is_empty() {
            return Err(
                "Expression must reference at least one series such as $3, !/group/ds[..,0], or !$3:ATTR"
                    .to_string(),
            );
        }

        let item_values = refs
            .item_refs
            .iter()
            .map(|item_ref| {
                resolve_expression_item_value(self, item_ref, resolution)
                    .map(|points| (item_ref.clone(), points))
            })
            .collect::<Result<std::collections::HashMap<_, _>, _>>()?;

        let external_series = resolve_expression_series_values(
            self,
            file,
            &refs.series_refs,
            resolution,
            allow_external_series,
        )?;
        let mut series_inputs = item_values
            .iter()
            .map(|(item_ref, points)| ExpressionSeriesInput {
                label: item_ref.render(),
                points: points.clone(),
            })
            .collect::<Vec<_>>();
        for series_ref in &refs.series_refs {
            let points = external_series.get(series_ref).cloned().ok_or_else(|| {
                format!("Series reference {} was not resolved", series_ref.render())
            })?;
            series_inputs.push(ExpressionSeriesInput {
                label: series_ref.render(),
                points,
            });
        }

        let first = series_inputs.first().ok_or_else(|| {
            "Expression must reference at least one chart item or dataset".to_string()
        })?;
        let expected_len = first.points.len();
        if expected_len == 0 {
            return Err("Cannot build an expression from empty series".to_string());
        }
        let require_matching_x = matches!(parsed, ParsedExpression::YSeries(_));
        validate_expression_series_compatibility(&series_inputs, expected_len, require_matching_x)?;

        let scalar_values = resolve_expression_scalar_values(self, file, &refs.scalar_refs)?;

        let mut points = Vec::with_capacity(expected_len);
        let kind = match &parsed {
            ParsedExpression::YSeries(ast) => {
                for idx in 0..expected_len {
                    let y = eval_expression_at(
                        ast,
                        idx,
                        &item_values,
                        &external_series,
                        &scalar_values,
                    )?;
                    points.push((first.points[idx].0, y));
                }
                DerivedExpressionKind::YSeries
            }
            ParsedExpression::XySeries(x_ast, y_ast) => {
                for idx in 0..expected_len {
                    let x = eval_expression_at(
                        x_ast,
                        idx,
                        &item_values,
                        &external_series,
                        &scalar_values,
                    )?;
                    let y = eval_expression_at(
                        y_ast,
                        idx,
                        &item_values,
                        &external_series,
                        &scalar_values,
                    )?;
                    points.push((x, y));
                }
                DerivedExpressionKind::XySeries
            }
        };

        let input_ids = collect_expression_input_ids(&refs);

        Ok(EvaluatedExpression {
            points,
            kind,
            input_ids,
        })
    }

    fn expression_detail_window(&self, expression: &str) -> Result<Option<ChartLodWindow>, String> {
        let tokens = tokenize_expression(expression)?;
        let parsed = parse_derived_expression(&tokens)?;
        let mut refs = ExpressionRefs::default();
        collect_parsed_expression_refs(&parsed, &mut refs);
        let mut expected = None::<ChartLodWindow>;
        for item_ref in &refs.item_refs {
            let item = self
                .item_by_id(item_ref.id)
                .ok_or_else(|| format!("Unknown chart item reference ${}", item_ref.id.0))?;
            let Some(window) = item.detail_window else {
                return Ok(None);
            };
            if expected.is_some_and(|existing| existing != window) {
                return Ok(None);
            }
            expected = Some(window);
        }
        for series_ref in &refs.series_refs {
            if let ExpressionObjectTarget::ItemRef(id) = &series_ref.target {
                let item = self
                    .item_by_id(*id)
                    .ok_or_else(|| format!("Unknown chart item reference ${}", id.0))?;
                let Some(window) = item.detail_window else {
                    return Ok(None);
                };
                if expected.is_some_and(|existing| existing != window) {
                    return Ok(None);
                }
                expected = Some(window);
            } else {
                return Ok(None);
            }
        }
        Ok(expected)
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

    pub fn add_chart_item(
        &mut self,
        source: ChartSource,
        points: Vec<Point>,
    ) -> Option<ChartItemId> {
        self.add_chart_item_with_status(source, Some(points), 0, MultiChartLoadState::Ready, false)
    }

    pub fn add_chart_item_with_status(
        &mut self,
        source: ChartSource,
        points: Option<Vec<Point>>,
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
            source,
            series,
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
        let prefixed = if normalized.starts_with('!') {
            normalized.to_string()
        } else {
            format!("!{normalized}")
        };
        let Some(series_ref) = Self::raw_dataset_reference(&prefixed)? else {
            return Err(format!(
                "Dataset reference '{}' must look like !/path or !/path[..,0]",
                dataset_spec
            ));
        };
        let file = file.ok_or_else(|| {
            "Adding a dataset by path requires an open file handle, but no file is loaded"
                .to_string()
        })?;
        let ExpressionObjectTarget::AbsolutePath(path) = &series_ref.target else {
            unreachable!();
        };
        let dataset = file.dataset(path).map_err(|error| {
            format!(
                "Dataset reference {} could not be opened: {}",
                series_ref.render(),
                error
            )
        })?;
        let shape = dataset.shape();
        let selection = series_ref.to_preview_selection(&shape)?;
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
mod tests;
