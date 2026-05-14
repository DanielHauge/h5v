use hdf5_metno::File;
use ratatui::layout::Rect;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::collections::{BTreeSet, HashMap, HashSet};

use crate::data::DatasetPlotingData;
#[cfg(test)]
use crate::data::{PreviewSelection, SliceSelection};

mod eval;
mod expression;
mod interaction;
mod model;
mod prompt;
mod render;
use eval::{
    dataset_ploting_data_from_points, eval_expression_at, read_expression_dataset_points,
    resolve_expression_item_value, resolve_expression_scalar_values,
    resolve_expression_series_values, validate_expression_series_compatibility,
    EvaluatedExpression, ExpressionSeriesInput,
};
use expression::{
    collect_expression_input_ids, collect_parsed_expression_refs, parse_derived_expression,
    tokenize_expression, ExpressionObjectTarget, ExpressionRefs, ExpressionToken, ParsedExpression,
};
use model::sanitize_chart_points;
#[allow(unused_imports)]
pub use model::{
    ChartItem, ChartItemId, ChartItemStats, ChartSeries, ChartSource, ChartXAxisPolicy,
    DatasetChartKind, DatasetChartSource, DerivedExpressionKind, Point,
};
use prompt::{
    ExpressionPromptInputKind, ExpressionPromptMessageKind, ExpressionPromptMode,
    ExpressionPromptState, ExpressionPromptSuggestion, ExpressionPromptSuggestionKind,
    EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS,
};
#[allow(unused_imports)]
use render::chart_plot_area_in_rect;

#[derive(Debug, Clone)]
struct ChartDragState {
    anchor_column: u16,
    viewport_from: usize,
    viewport_to: usize,
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

pub struct MultiChartState {
    items: Vec<ChartItem>,
    pub modified: bool,
    pub height: u32,
    pub width: u32,
    pub plot_buffer: Vec<u8>,
    pub picker: Picker,
    pub idx: usize,
    pub aoi_from: Option<usize>,
    pub aoi_to: Option<usize>,
    stateful_protocol: Option<StatefulProtocol>,
    next_id: u64,
    next_color_slot: usize,
    x_axis_policy: ChartXAxisPolicy,
    expression_prompt: Option<ExpressionPromptState>,
    last_chart_area: Option<Rect>,
    drag_state: Option<ChartDragState>,
}

impl MultiChartState {
    pub fn new(picker: Picker) -> Self {
        Self {
            items: Vec::new(),
            modified: false,
            idx: 0,
            height: 0,
            width: 0,
            plot_buffer: Vec::new(),
            picker,
            aoi_from: None,
            aoi_to: None,
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

    pub fn selected_item(&self) -> Option<&ChartItem> {
        self.items.get(self.idx)
    }

    pub fn is_expression_prompt_active(&self) -> bool {
        self.expression_prompt.is_some()
    }

    fn item_by_id(&self, id: ChartItemId) -> Option<&ChartItem> {
        self.items.iter().find(|item| item.id == id)
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
        let (source, series) = self.build_expression_chart_item(&expression, file)?;
        let points = series.points.clone();
        self.add_chart_item(source, points)
            .ok_or_else(|| "Failed to create expression-derived chart".to_string())
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

    fn recompute_expression_dependents(
        &mut self,
        affected_ids: Vec<ChartItemId>,
        file: Option<&File>,
    ) -> Result<(), String> {
        if affected_ids.is_empty() {
            return Ok(());
        }

        let affected = affected_ids.iter().copied().collect::<HashSet<_>>();
        let mut indegree = HashMap::<ChartItemId, usize>::new();
        let mut edges = HashMap::<ChartItemId, Vec<ChartItemId>>::new();
        for id in &affected_ids {
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
        let mut processed = 0usize;

        while let Some(id) = ready.iter().next().copied() {
            ready.remove(&id);
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
            processed += 1;

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

        if processed != affected_ids.len() {
            return Err(
                "Expression dependency cycle blocked recomputing dependent series".to_string(),
            );
        }
        Ok(())
    }

    fn update_expression_item_with_file(
        &mut self,
        id: ChartItemId,
        expression: String,
        file: Option<&File>,
    ) -> Result<(), String> {
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
                resolve_expression_item_value(self, item_ref)
                    .map(|points| (item_ref.clone(), points))
            })
            .collect::<Result<std::collections::HashMap<_, _>, _>>()?;

        let external_series = resolve_expression_series_values(self, file, &refs.series_refs)?;
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

    pub fn add_chart_item(
        &mut self,
        source: ChartSource,
        points: Vec<Point>,
    ) -> Option<ChartItemId> {
        let series = ChartSeries::from_points(points)?;
        if let Some((idx, item)) = self
            .items
            .iter_mut()
            .enumerate()
            .find(|(_, item)| item.source == source)
        {
            item.series = series;
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
            visible: true,
        });
        self.idx = self.items.len().saturating_sub(1);
        self.modified = true;
        Some(id)
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
        let tokens = tokenize_expression(&prefixed)?;
        let Some(ExpressionToken::SeriesRef(series_ref)) = tokens.first() else {
            return Err(format!(
                "Dataset reference '{}' must look like !/path or !/path[..,0]",
                dataset_spec
            ));
        };
        if tokens.len() != 1 {
            return Err(format!(
                "Dataset reference '{}' must contain only a single dataset selector",
                dataset_spec
            ));
        }
        let file = file.ok_or_else(|| {
            "Adding a dataset by path requires an open file handle, but no file is loaded"
                .to_string()
        })?;
        if !matches!(series_ref.target, ExpressionObjectTarget::AbsolutePath(_))
            || series_ref.attr_name.is_some()
        {
            return Err(format!(
                "Dataset reference '{}' must look like !/path or !/path[..,0]",
                dataset_spec
            ));
        }
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
        let points = read_expression_dataset_points(&dataset, series_ref)?;
        let source = ChartSource::DatasetSelection(DatasetChartSource {
            dataset_path: dataset.name(),
            display_path: dataset.name(),
            selection,
            shape,
            kind: DatasetChartKind::Dataset,
        });
        self.add_chart_item(source, points)
            .ok_or_else(|| "Failed to add dataset to multichart".to_string())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod tests;
