use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct ValidatedExpression {
    pub(super) kind: DerivedExpressionKind,
    pub(super) sample_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpressionValueKind {
    Scalar,
    Series,
}

fn classify_expression_value_kind(
    expr: &ExpressionAst,
    item_kinds: &HashMap<expression::ExpressionItemRef, ExpressionValueKind>,
    load_kinds: &HashMap<expression::ExpressionLoadRef, ExpressionValueKind>,
) -> Result<ExpressionValueKind, String> {
    match expr {
        ExpressionAst::Number(_) => Ok(ExpressionValueKind::Scalar),
        ExpressionAst::ItemRef(item_ref) => item_kinds
            .get(item_ref)
            .copied()
            .ok_or_else(|| format!("Unknown chart item reference {}", item_ref.render())),
        ExpressionAst::LoadRef(load_ref) => load_kinds
            .get(load_ref)
            .copied()
            .ok_or_else(|| format!("Unknown reference {}", load_ref.render())),
        ExpressionAst::UnaryMinus(inner) => {
            classify_expression_value_kind(inner, item_kinds, load_kinds)
        }
        ExpressionAst::Binary { lhs, rhs, .. } => {
            let lhs = classify_expression_value_kind(lhs, item_kinds, load_kinds)?;
            let rhs = classify_expression_value_kind(rhs, item_kinds, load_kinds)?;
            Ok(
                if lhs == ExpressionValueKind::Series || rhs == ExpressionValueKind::Series {
                    ExpressionValueKind::Series
                } else {
                    ExpressionValueKind::Scalar
                },
            )
        }
        ExpressionAst::FunctionCall { name, args } => {
            classify_function_value_kind(name, args, item_kinds, load_kinds)
        }
    }
}

fn classify_function_value_kind(
    name: &str,
    args: &[ExpressionAst],
    item_kinds: &HashMap<expression::ExpressionItemRef, ExpressionValueKind>,
    load_kinds: &HashMap<expression::ExpressionLoadRef, ExpressionValueKind>,
) -> Result<ExpressionValueKind, String> {
    let arg_kinds = args
        .iter()
        .map(|arg| classify_expression_value_kind(arg, item_kinds, load_kinds))
        .collect::<Result<Vec<_>, _>>()?;
    match name {
        "exp" => {
            if arg_kinds.len() != 2 {
                return Err("exp() expects exactly 2 arguments".to_string());
            }
            Ok(
                if arg_kinds
                    .iter()
                    .any(|kind| *kind == ExpressionValueKind::Series)
                {
                    ExpressionValueKind::Series
                } else {
                    ExpressionValueKind::Scalar
                },
            )
        }
        "avg" | "mean" | "min" | "max" | "stddev" | "len" => {
            if arg_kinds.len() != 1 {
                return Err(format!("{name}() expects exactly 1 argument"));
            }
            if arg_kinds[0] != ExpressionValueKind::Series {
                return Err(format!("{name}() requires a series argument"));
            }
            Ok(ExpressionValueKind::Scalar)
        }
        "abs" | "sqrt" | "ln" | "log10" | "sin" | "cos" | "tan" | "floor" | "ceil" | "round" => {
            if arg_kinds.len() != 1 {
                return Err(format!("{name}() expects exactly 1 argument"));
            }
            Ok(arg_kinds[0])
        }
        "rolling_mean" | "rolling_median" | "rolling_stddev" | "rolling_min" | "rolling_max" => {
            if arg_kinds.len() != 2 {
                return Err(format!("{name}() expects exactly 2 arguments"));
            }
            if arg_kinds[0] != ExpressionValueKind::Series {
                return Err(format!("{name}() requires a series as the first argument"));
            }
            if arg_kinds[1] != ExpressionValueKind::Scalar {
                return Err(format!("{name}() requires a scalar window argument"));
            }
            Ok(ExpressionValueKind::Series)
        }
        "rolling_quantile" => {
            if arg_kinds.len() != 3 {
                return Err("rolling_quantile() expects exactly 3 arguments".to_string());
            }
            if arg_kinds[0] != ExpressionValueKind::Series {
                return Err(
                    "rolling_quantile() requires a series as the first argument".to_string()
                );
            }
            if arg_kinds[1] != ExpressionValueKind::Scalar
                || arg_kinds[2] != ExpressionValueKind::Scalar
            {
                return Err(
                    "rolling_quantile() requires scalar window and quantile arguments".to_string(),
                );
            }
            Ok(ExpressionValueKind::Series)
        }
        "threshold" => {
            if arg_kinds.len() != 2 {
                return Err("threshold() expects exactly 2 arguments".to_string());
            }
            if arg_kinds[1] != ExpressionValueKind::Scalar {
                return Err("threshold() requires a scalar threshold argument".to_string());
            }
            Ok(arg_kinds[0])
        }
        "diff" => {
            if arg_kinds.len() != 1 {
                return Err("diff() expects exactly 1 argument".to_string());
            }
            if arg_kinds[0] != ExpressionValueKind::Series {
                return Err("diff() requires a series argument".to_string());
            }
            Ok(ExpressionValueKind::Series)
        }
        "max2" | "min2" => {
            if arg_kinds.len() != 2 {
                return Err(format!("{name}() expects exactly 2 arguments"));
            }
            if arg_kinds
                .iter()
                .any(|kind| *kind != ExpressionValueKind::Scalar)
            {
                return Err(format!("{name}() requires scalar arguments"));
            }
            Ok(ExpressionValueKind::Scalar)
        }
        _ => Err(format!("Unsupported function '{name}'")),
    }
}

fn interp_call_args(
    ast: &ExpressionAst,
) -> Option<(&expression::ExpressionItemRef, &ExpressionAst)> {
    let ExpressionAst::FunctionCall { name, args } = ast else {
        return None;
    };
    if name != "interp" || args.len() != 2 {
        return None;
    }
    let ExpressionAst::ItemRef(item_ref) = &args[0] else {
        return None;
    };
    Some((item_ref, &args[1]))
}

fn slice_call_args(
    ast: &ExpressionAst,
) -> Option<(
    &expression::ExpressionItemRef,
    &ExpressionAst,
    &ExpressionAst,
)> {
    let ExpressionAst::FunctionCall { name, args } = ast else {
        return None;
    };
    if name != "slice" || args.len() != 3 {
        return None;
    }
    let ExpressionAst::ItemRef(item_ref) = &args[0] else {
        return None;
    };
    Some((item_ref, &args[1], &args[2]))
}

fn ensure_top_level_transform_usage_is_supported(
    expr: &ExpressionAst,
    allow_top_level: bool,
) -> Result<(), String> {
    match expr {
        ExpressionAst::Number(_) | ExpressionAst::ItemRef(_) | ExpressionAst::LoadRef(_) => Ok(()),
        ExpressionAst::UnaryMinus(inner) => {
            ensure_top_level_transform_usage_is_supported(inner, false)
        }
        ExpressionAst::Binary { lhs, rhs, .. } => {
            ensure_top_level_transform_usage_is_supported(lhs, false)?;
            ensure_top_level_transform_usage_is_supported(rhs, false)
        }
        ExpressionAst::FunctionCall { name, args } => {
            if matches!(name.as_str(), "interp" | "slice") {
                if !allow_top_level {
                    return Err(format!("{name}() must be the top-level expression"));
                }
                let expected_arg_count = if name == "interp" { 2 } else { 3 };
                if args.len() != expected_arg_count {
                    return Err(format!(
                        "{name}() expects exactly {expected_arg_count} arguments"
                    ));
                }
                if !matches!(args.first(), Some(ExpressionAst::ItemRef(_))) {
                    return Err(format!(
                        "{name}() requires a direct chart item reference as the first argument"
                    ));
                }
            }
            for arg in args {
                ensure_top_level_transform_usage_is_supported(arg, false)?;
            }
            Ok(())
        }
    }
}

impl MultiChartState {
    pub(super) fn raw_dataset_reference(
        expression: &str,
    ) -> Result<Option<expression::ExpressionSeriesRef>, String> {
        let tokens = tokenize_expression(expression)?;
        let Some(ExpressionToken::LoadRef(series_ref)) = tokens.first() else {
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
    pub(super) fn create_expression_derived(
        &mut self,
        expression: String,
    ) -> Result<ChartItemId, String> {
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

    pub(super) fn create_expression_derived_with_file(
        &mut self,
        expression: String,
        file: Option<&File>,
    ) -> Result<ChartItemId, String> {
        let use_raw_dataset_reference =
            matches!(
                self.validate_expression_with_file(&expression, file),
                Ok(ValidatedExpression {
                    kind: DerivedExpressionKind::YSeries,
                    ..
                })
            ) && matches!(Self::raw_dataset_reference(&expression), Ok(Some(_)));
        if use_raw_dataset_reference {
            return self.add_dataset_reference_command(&expression, file);
        }
        match self.build_expression_chart_item(&expression, file) {
            Ok((source, series, scalar_value)) => {
                let points = series.points.clone();
                let id = self
                    .add_chart_item_with_scalar(source, points, scalar_value)
                    .ok_or_else(|| "Failed to create expression-derived chart".to_string())?;
                self.refresh_expression_detail_series(file)?;
                Ok(id)
            }
            Err(error) => self.apply_expression_error_state(None, expression, error),
        }
    }

    pub(super) fn build_expression_chart_item(
        &self,
        expression: &str,
        file: Option<&File>,
    ) -> Result<(ChartSource, ChartSeries, Option<f64>), String> {
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
        Ok((source, series, evaluated.scalar_value))
    }

    pub(super) fn apply_resolved_expression_item(
        &mut self,
        index: usize,
        source: ChartSource,
        series: ChartSeries,
        scalar_value: Option<f64>,
    ) {
        let item = &mut self.items[index];
        item.label = source.label();
        item.source = source;
        item.source_len = series.len();
        item.sampled = false;
        item.series = series;
        item.scalar_value = scalar_value;
        item.visible = true;
        item.clear_detail_state(true);
        item.load_state = MultiChartLoadState::Ready;
    }

    pub(super) fn invalid_expression_source(
        expression: String,
        previous_source: Option<&ChartSource>,
    ) -> ChartSource {
        match previous_source {
            Some(ChartSource::DerivedExpression {
                input_ids,
                len,
                kind,
                ..
            }) => ChartSource::DerivedExpression {
                expression,
                input_ids: input_ids.clone(),
                len: *len,
                kind: *kind,
            },
            _ => ChartSource::DerivedExpression {
                expression,
                input_ids: Vec::new(),
                len: 0,
                kind: DerivedExpressionKind::YSeries,
            },
        }
    }

    pub(super) fn apply_expression_error_state(
        &mut self,
        id: Option<ChartItemId>,
        expression: String,
        message: String,
    ) -> Result<ChartItemId, String> {
        match id {
            Some(id) => {
                let index = self
                    .items
                    .iter()
                    .position(|item| item.id == id)
                    .ok_or_else(|| format!("Chart item ${} no longer exists", id.0))?;
                let source =
                    Self::invalid_expression_source(expression, Some(&self.items[index].source));
                let item = &mut self.items[index];
                item.label = source.label();
                item.source = source;
                item.series = ChartSeries::from_points(vec![(0.0, 0.0)])
                    .ok_or_else(|| "Failed to create placeholder series".to_string())?;
                item.scalar_value = None;
                item.clear_detail_state(true);
                item.load_state = MultiChartLoadState::Error(message);
                item.visible = false;
                self.idx = index;
                self.modified = true;
                Ok(id)
            }
            None => {
                let source = Self::invalid_expression_source(expression, None);
                let id = self
                    .add_chart_item_with_status(
                        source,
                        Some(vec![(0.0, 0.0)]),
                        None,
                        0,
                        MultiChartLoadState::Error(message),
                        false,
                    )
                    .ok_or_else(|| "Failed to create expression-derived chart".to_string())?;
                if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
                    item.visible = false;
                }
                self.modified = true;
                Ok(id)
            }
        }
    }

    pub(super) fn direct_expression_dependents_of(&self, id: ChartItemId) -> Vec<ChartItemId> {
        let mut dependents = self
            .items
            .iter()
            .filter_map(|item| item.source.input_ids().contains(&id).then_some(item.id))
            .collect::<Vec<_>>();
        dependents.sort_by_key(|id| id.0);
        dependents
    }

    pub(super) fn transitive_expression_dependents_of(&self, id: ChartItemId) -> Vec<ChartItemId> {
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

    pub(super) fn validate_expression_rewire(
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

    pub(super) fn expression_recompute_order(
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

    pub(super) fn recompute_expression_dependents(
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
            let (source, series, scalar_value) = self
                .build_expression_chart_item(&expression, file)
                .map_err(|error| format!("Failed to recompute ${}: {}", id.0, error))?;
            let index = self
                .items
                .iter()
                .position(|item| item.id == id)
                .ok_or_else(|| format!("Chart item ${} no longer exists", id.0))?;
            self.apply_resolved_expression_item(index, source, series, scalar_value);
        }
        Ok(())
    }

    pub(super) fn update_expression_item_with_file(
        &mut self,
        id: ChartItemId,
        expression: String,
        file: Option<&File>,
    ) -> Result<(), String> {
        let use_raw_dataset_reference =
            matches!(
                self.validate_expression_with_file(&expression, file),
                Ok(ValidatedExpression {
                    kind: DerivedExpressionKind::YSeries,
                    ..
                })
            ) && matches!(Self::raw_dataset_reference(&expression), Ok(Some(_)));
        if use_raw_dataset_reference {
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
        let (source, series, scalar_value) =
            match self.build_expression_chart_item(&expression, file) {
                Ok(item) => item,
                Err(error) => {
                    self.apply_expression_error_state(Some(id), expression, error)?;
                    return Ok(());
                }
            };
        self.validate_expression_rewire(id, &previous_dependents, source.input_ids())?;
        let index = self
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| format!("Chart item ${} no longer exists", id.0))?;
        self.apply_resolved_expression_item(index, source, series, scalar_value);
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

    pub(super) fn evaluate_expression_with_file(
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

    pub(super) fn series_transform_input(
        &self,
        item_ref: &expression::ExpressionItemRef,
        resolution: ExpressionSeriesResolution,
    ) -> Result<(Vec<Point>, DerivedExpressionKind), String> {
        let item = match &item_ref.target {
            expression::ExpressionItemTarget::Id(id) => self
                .item_by_id(*id)
                .ok_or_else(|| format!("Unknown chart item reference ${}", id.0))?,
            expression::ExpressionItemTarget::Name(name) => self
                .item_by_name(name)
                .ok_or_else(|| format!("Unknown chart item reference ${name}"))?,
        };
        if !item.has_loaded_series() {
            return Err(format!(
                "Chart item reference {} is still loading",
                item_ref.render()
            ));
        }
        let kind = match item.source {
            ChartSource::DerivedExpression {
                kind: DerivedExpressionKind::XySeries,
                ..
            } => DerivedExpressionKind::XySeries,
            _ => DerivedExpressionKind::YSeries,
        };
        let points = match resolution {
            ExpressionSeriesResolution::Overview => item.overview_series().points.clone(),
            ExpressionSeriesResolution::Active => item.active_series().points.clone(),
        };
        let points = match &item_ref.slice {
            Some(slice) => {
                if slice.end > points.len() {
                    return Err(format!(
                        "Chart item reference {} is out of bounds for len {}",
                        item_ref.render(),
                        points.len()
                    ));
                }
                points[slice.start..slice.end].to_vec()
            }
            None => points,
        };
        if points.len() < 2 {
            return Err("Top-level series transforms require at least two samples".to_string());
        }
        for window in points.windows(2) {
            if window[1].0 <= window[0].0 {
                return Err(
                    "Top-level series transforms require strictly increasing x-values".to_string(),
                );
            }
        }
        Ok((points, kind))
    }

    pub(super) fn evaluate_interp_expression_with_resolution(
        &self,
        expr: &ExpressionAst,
        refs: &ExpressionRefs,
        resolution: ExpressionSeriesResolution,
        item_series_values: &HashMap<expression::ExpressionItemRef, Vec<Point>>,
        item_scalar_values: &HashMap<expression::ExpressionItemRef, f64>,
        external_series: &HashMap<expression::ExpressionLoadRef, Vec<Point>>,
        scalar_values: &HashMap<expression::ExpressionLoadRef, f64>,
    ) -> Result<EvaluatedExpression, String> {
        let Some((item_ref, sample_rate_expr)) = interp_call_args(expr) else {
            return Err("interp() must be the top-level expression".to_string());
        };
        let (input_points, kind) = self.series_transform_input(item_ref, resolution)?;
        if kind != DerivedExpressionKind::XySeries {
            return Err("interp() requires an x/y derived series such as ($1, $2)".to_string());
        }
        let sample_rate = eval_scalar_expression(
            sample_rate_expr,
            item_series_values,
            item_scalar_values,
            external_series,
            scalar_values,
            input_points.len(),
        )?;
        if !sample_rate.is_finite() || sample_rate <= 0.0 {
            return Err("interp() sample rate must be a positive finite scalar".to_string());
        }
        let first_x = input_points.first().map(|(x, _)| *x).unwrap_or_default();
        let last_x = input_points.last().map(|(x, _)| *x).unwrap_or_default();
        let mut next_x = ((first_x / sample_rate).floor() + 1.0) * sample_rate;
        let epsilon = sample_rate.abs() * 1e-9;
        let mut points = Vec::new();
        let mut segment_idx = 0usize;
        while next_x <= last_x + epsilon {
            while segment_idx + 1 < input_points.len() && input_points[segment_idx + 1].0 < next_x {
                segment_idx += 1;
            }
            if segment_idx + 1 >= input_points.len() {
                break;
            }
            let (x0, y0) = input_points[segment_idx];
            let (x1, y1) = input_points[segment_idx + 1];
            if next_x < x0 - epsilon || next_x > x1 + epsilon {
                next_x += sample_rate;
                continue;
            }
            let t = if (x1 - x0).abs() <= f64::EPSILON {
                0.0
            } else {
                (next_x - x0) / (x1 - x0)
            };
            points.push((next_x, y0 + (y1 - y0) * t));
            next_x += sample_rate;
        }
        if points.is_empty() {
            return Err("interp() sample rate produced no output samples".to_string());
        }
        Ok(EvaluatedExpression {
            points,
            scalar_value: None,
            kind: DerivedExpressionKind::XySeries,
            input_ids: self.collect_expression_input_ids(refs),
        })
    }

    pub(super) fn evaluate_slice_expression_with_resolution(
        &self,
        expr: &ExpressionAst,
        refs: &ExpressionRefs,
        resolution: ExpressionSeriesResolution,
        item_series_values: &HashMap<expression::ExpressionItemRef, Vec<Point>>,
        item_scalar_values: &HashMap<expression::ExpressionItemRef, f64>,
        external_series: &HashMap<expression::ExpressionLoadRef, Vec<Point>>,
        scalar_values: &HashMap<expression::ExpressionLoadRef, f64>,
    ) -> Result<EvaluatedExpression, String> {
        let Some((item_ref, start_expr, end_expr)) = slice_call_args(expr) else {
            return Err("slice() must be the top-level expression".to_string());
        };
        let (input_points, kind) = self.series_transform_input(item_ref, resolution)?;
        let start_x = eval_scalar_expression(
            start_expr,
            item_series_values,
            item_scalar_values,
            external_series,
            scalar_values,
            input_points.len(),
        )?;
        let end_x = eval_scalar_expression(
            end_expr,
            item_series_values,
            item_scalar_values,
            external_series,
            scalar_values,
            input_points.len(),
        )?;
        if !start_x.is_finite() || !end_x.is_finite() {
            return Err("slice() bounds must be finite scalars".to_string());
        }
        if start_x > end_x {
            return Err("slice() requires start <= end".to_string());
        }
        let points = input_points
            .into_iter()
            .filter(|(x, _)| *x >= start_x && *x <= end_x)
            .collect::<Vec<_>>();
        if points.is_empty() {
            return Err("slice() x-range produced no samples".to_string());
        }
        Ok(EvaluatedExpression {
            points,
            scalar_value: None,
            kind,
            input_ids: self.collect_expression_input_ids(refs),
        })
    }

    pub(super) fn validate_expression_with_file(
        &self,
        expression: &str,
        file: Option<&File>,
    ) -> Result<ValidatedExpression, String> {
        let tokens = tokenize_expression(expression)?;
        let parsed = parse_derived_expression(&tokens)?;
        match &parsed {
            ParsedExpression::YSeries(ast) => {
                ensure_top_level_transform_usage_is_supported(ast, true)?
            }
            ParsedExpression::XySeries(x_ast, y_ast) => {
                ensure_top_level_transform_usage_is_supported(x_ast, false)?;
                ensure_top_level_transform_usage_is_supported(y_ast, false)?;
            }
        }
        if matches!(
            &parsed,
            ParsedExpression::YSeries(ast)
                if interp_call_args(ast).is_some() || slice_call_args(ast).is_some()
        ) {
            let evaluated = self.evaluate_expression_with_resolution(
                expression,
                file,
                ExpressionSeriesResolution::Overview,
                true,
            )?;
            return Ok(ValidatedExpression {
                kind: evaluated.kind,
                sample_count: evaluated.points.len(),
            });
        }
        let mut refs = ExpressionRefs::default();
        collect_parsed_expression_refs(&parsed, &mut refs);
        refs.item_refs.sort_by_key(|item_ref| item_ref.render());
        refs.item_refs.dedup();
        refs.load_refs.sort_by_key(|load_ref| load_ref.render());
        refs.load_refs.dedup();
        let mut item_kinds = HashMap::new();
        let mut series_inputs = Vec::new();
        for item_ref in &refs.item_refs {
            match resolve_expression_item_value(
                self,
                item_ref,
                ExpressionSeriesResolution::Overview,
            )? {
                ResolvedExpressionItemValue::Scalar(_) => {
                    item_kinds.insert(item_ref.clone(), ExpressionValueKind::Scalar);
                }
                ResolvedExpressionItemValue::Series(points) => {
                    item_kinds.insert(item_ref.clone(), ExpressionValueKind::Series);
                    series_inputs.push(ExpressionSeriesInput {
                        label: item_ref.render(),
                        points,
                    });
                }
            }
        }

        let file = if refs.load_refs.is_empty() {
            None
        } else {
            Some(file.ok_or_else(|| {
                "load(...) references require an open file handle, but no file is loaded"
                    .to_string()
            })?)
        };

        let mut load_kinds = HashMap::new();
        if let Some(file) = file {
            for load_ref in &refs.load_refs {
                match validate_expression_load_ref(
                    self,
                    file,
                    load_ref,
                    ExpressionSeriesResolution::Overview,
                    true,
                )? {
                    ValidatedExpressionLoad::Series { len } => {
                        load_kinds.insert(load_ref.clone(), ExpressionValueKind::Series);
                        series_inputs.push(ExpressionSeriesInput {
                            label: load_ref.render(),
                            points: (0..len).map(|idx| (idx as f64, idx as f64)).collect(),
                        });
                    }
                    ValidatedExpressionLoad::Scalar => {
                        load_kinds.insert(load_ref.clone(), ExpressionValueKind::Scalar);
                    }
                }
            }
        }

        let (kind, expected_len) = match &parsed {
            ParsedExpression::YSeries(ast) => {
                let value_kind = classify_expression_value_kind(ast, &item_kinds, &load_kinds)?;
                match value_kind {
                    ExpressionValueKind::Scalar => (DerivedExpressionKind::Scalar, 1),
                    ExpressionValueKind::Series => {
                        let first = series_inputs.first().ok_or_else(|| {
                            "Expression must reference at least one series such as $3 or load(/group/ds)[..,0]"
                                .to_string()
                        })?;
                        let expected_len = first.points.len();
                        if expected_len == 0 {
                            return Err("Cannot build an expression from empty series".to_string());
                        }
                        validate_expression_series_compatibility(
                            &series_inputs,
                            expected_len,
                            true,
                        )?;
                        (DerivedExpressionKind::YSeries, expected_len)
                    }
                }
            }
            ParsedExpression::XySeries(x_ast, y_ast) => {
                if classify_expression_value_kind(x_ast, &item_kinds, &load_kinds)?
                    != ExpressionValueKind::Series
                    || classify_expression_value_kind(y_ast, &item_kinds, &load_kinds)?
                        != ExpressionValueKind::Series
                {
                    return Err("x/y expressions must resolve to series values".to_string());
                }
                let first = series_inputs.first().ok_or_else(|| {
                    "Expression must reference at least one series such as $3 or load(/group/ds)[..,0]"
                        .to_string()
                })?;
                let expected_len = first.points.len();
                if expected_len == 0 {
                    return Err("Cannot build an expression from empty series".to_string());
                }
                validate_expression_series_compatibility(&series_inputs, expected_len, false)?;
                (DerivedExpressionKind::XySeries, expected_len)
            }
        };

        Ok(ValidatedExpression {
            kind,
            sample_count: expected_len,
        })
    }

    pub(super) fn evaluate_expression_with_resolution(
        &self,
        expression: &str,
        file: Option<&File>,
        resolution: ExpressionSeriesResolution,
        allow_external_series: bool,
    ) -> Result<EvaluatedExpression, String> {
        let tokens = tokenize_expression(expression)?;
        let parsed = parse_derived_expression(&tokens)?;
        match &parsed {
            ParsedExpression::YSeries(ast) => {
                ensure_top_level_transform_usage_is_supported(ast, true)?
            }
            ParsedExpression::XySeries(x_ast, y_ast) => {
                ensure_top_level_transform_usage_is_supported(x_ast, false)?;
                ensure_top_level_transform_usage_is_supported(y_ast, false)?;
            }
        }
        let mut refs = ExpressionRefs::default();
        collect_parsed_expression_refs(&parsed, &mut refs);
        refs.item_refs.sort_by_key(|item_ref| item_ref.render());
        refs.item_refs.dedup();
        refs.load_refs.sort_by_key(|load_ref| load_ref.render());
        refs.load_refs.dedup();
        let mut item_series_values = HashMap::new();
        let mut item_scalar_values = HashMap::new();
        let mut item_kinds = HashMap::new();
        for item_ref in &refs.item_refs {
            match resolve_expression_item_value(self, item_ref, resolution)? {
                ResolvedExpressionItemValue::Scalar(value) => {
                    item_kinds.insert(item_ref.clone(), ExpressionValueKind::Scalar);
                    item_scalar_values.insert(item_ref.clone(), value);
                }
                ResolvedExpressionItemValue::Series(points) => {
                    item_kinds.insert(item_ref.clone(), ExpressionValueKind::Series);
                    item_series_values.insert(item_ref.clone(), points);
                }
            }
        }

        let file = if refs.load_refs.is_empty() {
            None
        } else {
            Some(file.ok_or_else(|| {
                "load(...) references require an open file handle, but no file is loaded"
                    .to_string()
            })?)
        };
        let mut external_series = std::collections::HashMap::new();
        let mut scalar_values = std::collections::HashMap::new();
        let mut load_kinds = HashMap::new();
        if let Some(file) = file {
            for load_ref in &refs.load_refs {
                match resolve_expression_load_value(
                    self,
                    file,
                    load_ref,
                    resolution,
                    allow_external_series,
                )? {
                    ResolvedExpressionLoad::Series(points) => {
                        load_kinds.insert(load_ref.clone(), ExpressionValueKind::Series);
                        external_series.insert(load_ref.clone(), points);
                    }
                    ResolvedExpressionLoad::Scalar(value) => {
                        load_kinds.insert(load_ref.clone(), ExpressionValueKind::Scalar);
                        scalar_values.insert(load_ref.clone(), value);
                    }
                }
            }
        }
        if let ParsedExpression::YSeries(ast) = &parsed {
            if interp_call_args(ast).is_some() {
                return self.evaluate_interp_expression_with_resolution(
                    ast,
                    &refs,
                    resolution,
                    &item_series_values,
                    &item_scalar_values,
                    &external_series,
                    &scalar_values,
                );
            }
            if slice_call_args(ast).is_some() {
                return self.evaluate_slice_expression_with_resolution(
                    ast,
                    &refs,
                    resolution,
                    &item_series_values,
                    &item_scalar_values,
                    &external_series,
                    &scalar_values,
                );
            }
        }
        let mut series_inputs = item_series_values
            .iter()
            .map(|(item_ref, points)| ExpressionSeriesInput {
                label: item_ref.render(),
                points: points.clone(),
            })
            .collect::<Vec<_>>();
        for load_ref in &refs.load_refs {
            let Some(points) = external_series.get(load_ref).cloned() else {
                continue;
            };
            series_inputs.push(ExpressionSeriesInput {
                label: load_ref.render(),
                points,
            });
        }

        let mut points = Vec::new();
        let mut scalar_output = None;
        let kind = match &parsed {
            ParsedExpression::YSeries(ast) => {
                match classify_expression_value_kind(ast, &item_kinds, &load_kinds)? {
                    ExpressionValueKind::Scalar => {
                        let value = eval_scalar_expression(
                            ast,
                            &item_series_values,
                            &item_scalar_values,
                            &external_series,
                            &scalar_values,
                            series_inputs
                                .first()
                                .map(|first| first.points.len())
                                .unwrap_or(1),
                        )?;
                        scalar_output = Some(value);
                        points.push((0.0, value));
                        DerivedExpressionKind::Scalar
                    }
                    ExpressionValueKind::Series => {
                        let first = series_inputs.first().ok_or_else(|| {
                            "Expression must reference at least one series such as $3 or load(/group/ds)[..,0]"
                                .to_string()
                        })?;
                        let expected_len = first.points.len();
                        if expected_len == 0 {
                            return Err("Cannot build an expression from empty series".to_string());
                        }
                        validate_expression_series_compatibility(
                            &series_inputs,
                            expected_len,
                            true,
                        )?;
                        points.reserve(expected_len);
                        for idx in 0..expected_len {
                            let y = eval_expression_at(
                                ast,
                                idx,
                                &item_series_values,
                                &item_scalar_values,
                                &external_series,
                                &scalar_values,
                                expected_len,
                            )?;
                            points.push((first.points[idx].0, y));
                        }
                        DerivedExpressionKind::YSeries
                    }
                }
            }
            ParsedExpression::XySeries(x_ast, y_ast) => {
                if classify_expression_value_kind(x_ast, &item_kinds, &load_kinds)?
                    != ExpressionValueKind::Series
                    || classify_expression_value_kind(y_ast, &item_kinds, &load_kinds)?
                        != ExpressionValueKind::Series
                {
                    return Err("x/y expressions must resolve to series values".to_string());
                }
                let first = series_inputs.first().ok_or_else(|| {
                    "Expression must reference at least one series such as $3 or load(/group/ds)[..,0]"
                        .to_string()
                })?;
                let expected_len = first.points.len();
                if expected_len == 0 {
                    return Err("Cannot build an expression from empty series".to_string());
                }
                validate_expression_series_compatibility(&series_inputs, expected_len, false)?;
                points.reserve(expected_len);
                for idx in 0..expected_len {
                    let x = eval_expression_at(
                        x_ast,
                        idx,
                        &item_series_values,
                        &item_scalar_values,
                        &external_series,
                        &scalar_values,
                        expected_len,
                    )?;
                    let y = eval_expression_at(
                        y_ast,
                        idx,
                        &item_series_values,
                        &item_scalar_values,
                        &external_series,
                        &scalar_values,
                        expected_len,
                    )?;
                    points.push((x, y));
                }
                DerivedExpressionKind::XySeries
            }
        };

        let input_ids = self.collect_expression_input_ids(&refs);

        Ok(EvaluatedExpression {
            points,
            scalar_value: scalar_output,
            kind,
            input_ids,
        })
    }
}
