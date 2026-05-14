use hdf5_metno::File;
use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea},
    style::{Color as _, IntoFont, RGBColor, ShapeStyle},
};
use ratatui::layout::Rect;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    ops::Range,
};

#[cfg(test)]
use crate::data::{PreviewSelection, SliceSelection};
use crate::{configure, data::DatasetPlotingData, error::log_error};

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
    current_expression_completion, expression_prompt_input_segments, expression_prompt_messages,
    expression_prompt_suggestions, ExpressionPromptInputKind, ExpressionPromptMessage,
    ExpressionPromptMessageKind, ExpressionPromptMode, ExpressionPromptState,
    ExpressionPromptSuggestion, ExpressionPromptSuggestionKind,
    EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS,
};

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

    pub fn open_expression_prompt(&mut self) {
        let buffer = String::new();
        let cursor = buffer.len();
        self.expression_prompt = Some(ExpressionPromptState::new(
            buffer,
            cursor,
            ExpressionPromptMode::New,
        ));
        self.modified = true;
    }

    pub fn open_selected_item_for_edit(&mut self) -> Result<(), String> {
        let selected = self
            .selected_item()
            .ok_or_else(|| "No chart item selected".to_string())?;
        let buffer = selected.editable_expression().ok_or_else(|| {
            format!(
                "Selected series ${} cannot be edited as an expression",
                selected.id.0
            )
        })?;
        let cursor = buffer.len();
        self.expression_prompt = Some(ExpressionPromptState::new(
            buffer,
            cursor,
            ExpressionPromptMode::EditExisting(selected.id),
        ));
        self.modified = true;
        Ok(())
    }

    pub fn close_expression_prompt(&mut self) {
        self.expression_prompt = None;
        self.modified = true;
    }

    pub fn expression_insert_char(&mut self, ch: char) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.buffer.insert(prompt.cursor, ch);
            prompt.cursor += 1;
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_backspace(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor > 0 {
                prompt.cursor -= 1;
                prompt.buffer.remove(prompt.cursor);
                prompt.selected_suggestion = None;
            }
        }
    }

    pub fn expression_delete(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor < prompt.buffer.len() {
                prompt.buffer.remove(prompt.cursor);
                prompt.selected_suggestion = None;
            }
        }
    }

    pub fn expression_move_left(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor > 0 {
                prompt.cursor -= 1;
            }
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_move_right(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor < prompt.buffer.len() {
                prompt.cursor += 1;
            }
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_move_to_start(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.cursor = 0;
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_move_to_end(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.cursor = prompt.buffer.len();
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_clear(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.buffer.clear();
            prompt.cursor = 0;
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_select_next_suggestion(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if !prompt.suggestions.is_empty() {
                let visible = prompt
                    .suggestions
                    .len()
                    .min(EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS);
                prompt.selected_suggestion = Some(match prompt.selected_suggestion {
                    Some(selected) => (selected + 1) % visible,
                    None => 0,
                });
            }
        }
    }

    pub fn expression_select_prev_suggestion(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if !prompt.suggestions.is_empty() {
                let visible = prompt
                    .suggestions
                    .len()
                    .min(EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS);
                prompt.selected_suggestion = Some(match prompt.selected_suggestion {
                    Some(0) | None => visible - 1,
                    Some(selected) => selected - 1,
                });
            }
        }
    }

    pub fn expression_deselect_suggestion(&mut self) -> bool {
        let Some(prompt) = self.expression_prompt.as_mut() else {
            return false;
        };
        prompt.selected_suggestion.take().is_some()
    }

    pub fn expression_has_selected_suggestion(&self) -> bool {
        self.expression_prompt
            .as_ref()
            .and_then(|prompt| prompt.selected_suggestion)
            .is_some()
    }

    pub fn expression_apply_selected_suggestion(&mut self) -> bool {
        let Some(prompt) = self.expression_prompt.as_mut() else {
            return false;
        };
        let Some((start, end, suggestion)) = current_expression_completion(prompt)
            .map(|(start, end, _, suggestion)| (start, end, suggestion.clone()))
        else {
            return false;
        };
        prompt
            .buffer
            .replace_range(start..end, &suggestion.insert_text);
        prompt.cursor = start + suggestion.insert_text.len();
        prompt.selected_suggestion = None;
        true
    }

    pub fn refresh_expression_prompt(&mut self, file: Option<&File>) {
        let Some((buffer, cursor, selected_suggestion)) =
            self.expression_prompt.as_ref().map(|prompt| {
                (
                    prompt.buffer.clone(),
                    prompt.cursor,
                    prompt.selected_suggestion,
                )
            })
        else {
            return;
        };
        let messages = expression_prompt_messages(self, file, &buffer);
        let suggestions = expression_prompt_suggestions(self, file, &buffer, cursor);
        let input_segments = expression_prompt_input_segments(self, file, &buffer);
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.messages = messages;
            prompt.suggestions = suggestions;
            prompt.selected_suggestion =
                selected_suggestion.filter(|selected| *selected < prompt.suggestions.len());
            prompt.input_segments = input_segments;
        }
    }

    pub fn submit_expression_prompt(&mut self, file: Option<&File>) -> Result<(), String> {
        let (expression, mode) = self
            .expression_prompt
            .as_ref()
            .map(|prompt| (prompt.buffer.trim().to_string(), prompt.mode.clone()))
            .ok_or_else(|| "Expression prompt is not active".to_string())?;
        if expression.is_empty() {
            self.set_expression_messages(vec![ExpressionPromptMessage {
                kind: ExpressionPromptMessageKind::Error,
                text: "Enter an expression before submitting".to_string(),
            }]);
            return Ok(());
        }

        let result = match mode {
            ExpressionPromptMode::New => self
                .create_expression_derived_with_file(expression.clone(), file)
                .map(|_| ()),
            ExpressionPromptMode::EditExisting(id) => {
                self.update_expression_item_with_file(id, expression.clone(), file)
            }
        };

        match result {
            Ok(_) => {
                self.close_expression_prompt();
                Ok(())
            }
            Err(error) => {
                self.set_expression_messages(vec![ExpressionPromptMessage {
                    kind: ExpressionPromptMessageKind::Error,
                    text: error,
                }]);
                Ok(())
            }
        }
    }

    fn set_expression_messages(&mut self, messages: Vec<ExpressionPromptMessage>) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.messages = messages;
        }
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

    fn prepared_chart_data(&self) -> Option<PreparedChartData> {
        let visible_items = self
            .items
            .iter()
            .filter(|item| item.visible)
            .collect::<Vec<_>>();
        if visible_items.is_empty() {
            return None;
        }

        let (global_x_min, global_x_max) = self.global_x_bounds().unwrap_or((0, 1));
        let (x_min, x_max) = match (self.aoi_from, self.aoi_to) {
            (None, None) => (global_x_min, global_x_max),
            (Some(from), None) => (from, global_x_max.max(from)),
            (None, Some(to)) => (global_x_min.min(to), to),
            (Some(from), Some(to)) if from < to => (from, to),
            _ => return None,
        };

        let selected_item_id = self.selected_item().map(|item| item.id);
        let mut global_y_max = f64::MIN;
        let mut global_y_min = f64::MAX;
        let mut plot_x_min = f64::MAX;
        let mut plot_x_max = f64::MIN;
        let mut series = Vec::new();

        for item in visible_items {
            if x_max <= item.series.sample_min || x_min >= item.series.sample_max {
                continue;
            }
            let local_x_min = item
                .series
                .sample_min
                .max(x_min)
                .clamp(item.series.sample_min, item.series.sample_max);
            let local_x_max = item
                .series
                .sample_max
                .min(x_max)
                .clamp(item.series.sample_min, item.series.sample_max);
            let points = sanitize_chart_points(
                item.series.points[local_x_min..local_x_max]
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
            );
            if points.is_empty() {
                continue;
            }

            for &(x, y) in &points {
                global_y_max = global_y_max.max(y);
                global_y_min = global_y_min.min(y);
                plot_x_min = plot_x_min.min(x);
                plot_x_max = plot_x_max.max(x);
            }

            series.push(PreparedChartSeries {
                label: item.label.clone(),
                color_slot: item.color_slot,
                points,
                is_selected: selected_item_id == Some(item.id),
            });
        }

        if series.is_empty() || !global_y_min.is_finite() || !global_y_max.is_finite() {
            return None;
        }
        let (y_min, y_max) = if (global_y_max - global_y_min).abs() < f64::EPSILON {
            let pad = if global_y_min == 0.0 {
                1.0
            } else {
                global_y_min.abs() * 0.05
            };
            (global_y_min - pad, global_y_max + pad)
        } else {
            (global_y_min, global_y_max)
        };
        if !plot_x_min.is_finite() || !plot_x_max.is_finite() {
            return None;
        }
        let (plot_x_min, plot_x_max) = if (plot_x_max - plot_x_min).abs() < f64::EPSILON {
            let pad = if plot_x_min == 0.0 {
                1.0
            } else {
                plot_x_min.abs() * 0.05
            };
            (plot_x_min - pad, plot_x_max + pad)
        } else {
            (plot_x_min, plot_x_max)
        };

        Some(PreparedChartData {
            plot_x_min,
            plot_x_max,
            y_min,
            y_max,
            series,
        })
    }

    fn render_chart_with_area(&mut self, chart_area: Option<Rect>) -> bool {
        if !self.modified {
            return false;
        }
        self.idx = self.idx.clamp(0, self.items.len().saturating_sub(1));
        self.modified = false;
        let Some(prepared) = self.prepared_chart_data() else {
            return false;
        };

        let width = self.width;
        let height = self.height;
        self.plot_buffer = vec![0; (width * height * 3) as usize];
        let root =
            BitMapBackend::with_buffer(&mut self.plot_buffer, (width, height)).into_drawing_area();
        let (bg_r, bg_g, bg_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.plot_bg));
        let (grid_r, grid_g, grid_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.grid));
        let (axis_r, axis_g, axis_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.axis));
        let plot_bg = RGBColor(bg_r, bg_g, bg_b);
        let grid = RGBColor(grid_r, grid_g, grid_b);
        let axis = RGBColor(axis_r, axis_g, axis_b);
        if let Err(e) = root.fill(&plot_bg) {
            log_error(e);
            return false;
        }
        let y_label_area_size = format!("{:.4}", prepared.y_max).len() as u32 * 3 + 30;
        let chart = plotters::prelude::ChartBuilder::on(&root)
            .margin(10)
            .x_label_area_size(30)
            .y_label_area_size(y_label_area_size)
            .build_cartesian_2d(
                prepared.plot_x_min..prepared.plot_x_max,
                prepared.y_min..prepared.y_max,
            );

        let mut chart = match chart {
            Ok(chart) => chart,
            Err(e) => {
                log_error(e);
                return false;
            }
        };
        if let Some(chart_area) = chart_area {
            let (plot_x_range, plot_y_range) = chart.plotting_area().get_pixel_range();
            self.last_chart_area =
                chart_plot_area_in_rect(chart_area, width, height, plot_x_range, plot_y_range);
        }

        if let Err(e) = chart
            .configure_mesh()
            .x_desc(self.x_axis_policy.label())
            .y_label_style(("sans-serif", 18).into_font().color(&axis))
            .x_label_style(("sans-serif", 18).into_font().color(&axis))
            .axis_style(ShapeStyle::from(&axis).stroke_width(2))
            .light_line_style(grid.mix(0.35))
            .bold_line_style(grid.mix(0.55))
            .draw()
        {
            log_error(e);
        }

        for series in prepared.series {
            let (r, g, b) = configure::rgb_channels(configure::themed_color(|colors| {
                colors.chart.series[series.color_slot % colors.chart.series.len()]
            }));
            let color = RGBColor(r, g, b);
            let stroke_width = if series.is_selected { 4 } else { 3 };
            let line_series = plotters::prelude::LineSeries::new(
                series.points.iter().copied(),
                ShapeStyle::from(&color).stroke_width(stroke_width),
            );
            let series_label = series.label.clone();
            let drawn_series = match chart.draw_series(line_series) {
                Ok(series) => series,
                Err(e) => {
                    log_error(e);
                    continue;
                }
            };
            drawn_series.label(series_label).legend(move |(x, y)| {
                plotters::prelude::PathElement::new(
                    vec![(x, y), (x + 20, y)],
                    plotters::prelude::ShapeStyle {
                        filled: true,
                        stroke_width,
                        color: plotters::style::Color::to_rgba(&color),
                    },
                )
            });
        }

        if let Err(e) = root.present() {
            log_error(e);
        }

        true
    }
}

fn chart_plot_area_in_rect(
    outer_area: Rect,
    width_px: u32,
    height_px: u32,
    plot_x_range: Range<i32>,
    plot_y_range: Range<i32>,
) -> Option<Rect> {
    if outer_area.width == 0 || outer_area.height == 0 || width_px == 0 || height_px == 0 {
        return None;
    }
    let x_start = plot_x_range.start.max(0) as u32;
    let x_end = plot_x_range.end.max(plot_x_range.start).max(0) as u32;
    let y_start = plot_y_range.start.max(0) as u32;
    let y_end = plot_y_range.end.max(plot_y_range.start).max(0) as u32;
    if x_end <= x_start || y_end <= y_start {
        return None;
    }

    let left = x_start
        .saturating_mul(outer_area.width as u32)
        .checked_div(width_px)
        .unwrap_or(0);
    let right = ((x_end.saturating_mul(outer_area.width as u32)) + width_px.saturating_sub(1))
        .checked_div(width_px)
        .unwrap_or(outer_area.width as u32)
        .min(outer_area.width as u32);
    let top = y_start
        .saturating_mul(outer_area.height as u32)
        .checked_div(height_px)
        .unwrap_or(0);
    let bottom = ((y_end.saturating_mul(outer_area.height as u32)) + height_px.saturating_sub(1))
        .checked_div(height_px)
        .unwrap_or(outer_area.height as u32)
        .min(outer_area.height as u32);

    let width = right.saturating_sub(left).max(1) as u16;
    let height = bottom.saturating_sub(top).max(1) as u16;
    Some(Rect::new(
        outer_area.x.saturating_add(left as u16),
        outer_area.y.saturating_add(top as u16),
        width.min(outer_area.width.saturating_sub(left as u16)),
        height.min(outer_area.height.saturating_sub(top as u16)),
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod tests;
