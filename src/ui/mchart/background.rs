use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::mpsc::{channel, Sender},
    thread,
};

use hdf5_metno::File;

use crate::{
    data::DatasetPlotingData,
    ui::{app::AppEvent, perf},
};

use super::{
    eval::{
        dataset_ploting_data_from_points, eval_expression_at, eval_scalar_expression,
        resolve_expression_item_value, resolve_expression_load_value,
        validate_expression_series_compatibility, EvaluatedExpression, ExpressionItemLookup,
        ExpressionSeriesInput, ExpressionSeriesResolution, ResolvedExpressionItemValue,
    },
    expression::{
        self, collect_parsed_expression_refs, parse_derived_expression, tokenize_expression,
        ExpressionAst, ExpressionRefs, ParsedExpression,
    },
    ChartItem, ChartItemId, ChartLodWindow, ChartSeries, ChartSource, DerivedExpressionKind,
    MultiChartDerivedDetailUpdate, MultiChartExpressionRefreshRequest,
    MultiChartExpressionRefreshResult, MultiChartLoadState, Point,
};

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
            Ok(if arg_kinds.contains(&ExpressionValueKind::Series) {
                ExpressionValueKind::Series
            } else {
                ExpressionValueKind::Scalar
            })
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

#[derive(Debug, Clone)]
struct ExpressionEvalSnapshot {
    items: Vec<ChartItem>,
}

impl ExpressionItemLookup for ExpressionEvalSnapshot {
    fn item_by_id(&self, id: ChartItemId) -> Option<&ChartItem> {
        self.item_by_id(id)
    }

    fn item_by_name(&self, name: &str) -> Option<&ChartItem> {
        self.item_by_name(name)
    }
}

impl ExpressionEvalSnapshot {
    fn new(items: &[ChartItem]) -> Self {
        Self {
            items: items.to_vec(),
        }
    }

    fn item_by_id(&self, id: ChartItemId) -> Option<&ChartItem> {
        self.items.iter().find(|item| item.id == id)
    }

    fn item_by_name(&self, name: &str) -> Option<&ChartItem> {
        self.items
            .iter()
            .find(|item| item.name.as_deref() == Some(name))
    }

    fn item_by_id_mut(&mut self, id: ChartItemId) -> Option<&mut ChartItem> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    fn collect_expression_input_ids(&self, refs: &ExpressionRefs) -> Vec<ChartItemId> {
        let mut input_ids = refs
            .item_refs
            .iter()
            .filter_map(|item_ref| match &item_ref.target {
                expression::ExpressionItemTarget::Id(id) => Some(*id),
                expression::ExpressionItemTarget::Name(name) => {
                    self.item_by_name(name).map(|item| item.id)
                }
            })
            .collect::<Vec<_>>();
        input_ids.sort_by_key(|id| id.0);
        input_ids.dedup();
        input_ids
    }

    fn series_transform_input(
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

    fn expression_detail_window(&self, expression: &str) -> Result<Option<ChartLodWindow>, String> {
        let tokens = tokenize_expression(expression)?;
        let parsed = parse_derived_expression(&tokens)?;
        let mut refs = ExpressionRefs::default();
        collect_parsed_expression_refs(&parsed, &mut refs);
        let mut expected = None::<ChartLodWindow>;
        for item_ref in &refs.item_refs {
            let item = match &item_ref.target {
                expression::ExpressionItemTarget::Id(id) => self
                    .item_by_id(*id)
                    .ok_or_else(|| format!("Unknown chart item reference ${}", id.0))?,
                expression::ExpressionItemTarget::Name(name) => self
                    .item_by_name(name)
                    .ok_or_else(|| format!("Unknown chart item reference ${name}"))?,
            };
            let Some(window) = item.detail_window else {
                return Ok(None);
            };
            if expected.is_some_and(|existing| existing != window) {
                return Ok(None);
            }
            expected = Some(window);
        }
        if !refs.load_refs.is_empty() {
            return Ok(None);
        }
        Ok(expected)
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

    fn set_detail_state(
        &mut self,
        item_id: ChartItemId,
        detail_series: Option<ChartSeries>,
        detail_window: Option<ChartLodWindow>,
    ) {
        let Some(item) = self.item_by_id_mut(item_id) else {
            return;
        };
        item.detail_series = detail_series;
        item.detail_window = detail_window;
        item.pending_detail_window = None;
        item.load_state = MultiChartLoadState::Ready;
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

    fn evaluate_interp_expression_with_resolution(
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

    fn evaluate_slice_expression_with_resolution(
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

    fn evaluate_expression_with_resolution(
        &self,
        expression: &str,
        file: Option<&File>,
        resolution: ExpressionSeriesResolution,
        allow_external_series: bool,
    ) -> Result<EvaluatedExpression, String> {
        let _eval_timer = perf::metrics().mchart.expression_eval.start();
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
        let mut external_series = HashMap::new();
        let mut scalar_values = HashMap::new();
        let mut load_kinds = HashMap::new();
        if let Some(file) = file {
            for load_ref in &refs.load_refs {
                match resolve_expression_load_value(
                    file,
                    load_ref,
                    resolution,
                    allow_external_series,
                )? {
                    super::eval::ResolvedExpressionLoad::Series(points) => {
                        load_kinds.insert(load_ref.clone(), ExpressionValueKind::Series);
                        external_series.insert(load_ref.clone(), points);
                    }
                    super::eval::ResolvedExpressionLoad::Scalar(value) => {
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

fn open_worker_file(file_path: Option<&str>) -> Result<Option<File>, String> {
    match file_path {
        Some(file_path) => File::with_options()
            .with_fapl(|fapl| {
                fapl.fclose_degree(hdf5_metno::plist::file_access::FileCloseDegree::Strong)
            })
            .open(file_path)
            .map(Some)
            .map_err(|error| format!("Failed to open HDF5 file '{file_path}': {error}")),
        None => Ok(None),
    }
}

pub(crate) fn evaluate_preview_expression(
    items: &[ChartItem],
    expression: &str,
    file_path: Option<&str>,
) -> Result<DatasetPlotingData, String> {
    let file = open_worker_file(file_path)?;
    let snapshot = ExpressionEvalSnapshot::new(items);
    let evaluated = snapshot.evaluate_expression_with_file(expression, file.as_ref())?;
    dataset_ploting_data_from_points(evaluated.points)
}

fn compute_expression_refresh(
    request: MultiChartExpressionRefreshRequest,
) -> MultiChartExpressionRefreshResult {
    let _refresh_timer = perf::metrics().mchart.detail_refresh.start();
    let mut snapshot = ExpressionEvalSnapshot::new(&request.items);
    let file = match open_worker_file(request.file_path.as_deref()) {
        Ok(file) => file,
        Err(message) => {
            return MultiChartExpressionRefreshResult::Failure {
                revision: request.revision,
                message,
            };
        }
    };
    let derived_ids = snapshot
        .items
        .iter()
        .filter_map(|item| {
            matches!(item.source, ChartSource::DerivedExpression { .. }).then_some(item.id)
        })
        .collect::<Vec<_>>();
    perf::metrics()
        .mchart
        .detail_items_seen
        .add(derived_ids.len() as u64);
    let ordered_ids = match snapshot.expression_recompute_order(&derived_ids) {
        Ok(ordered_ids) => ordered_ids,
        Err(message) => {
            return MultiChartExpressionRefreshResult::Failure {
                revision: request.revision,
                message,
            };
        }
    };
    let mut updates = Vec::with_capacity(ordered_ids.len());
    for id in ordered_ids {
        let expression = match snapshot.item_by_id(id).map(|item| item.source.clone()) {
            Some(ChartSource::DerivedExpression { expression, .. }) => expression,
            _ => continue,
        };
        let Some(window) = (match snapshot.expression_detail_window(&expression) {
            Ok(window) => window,
            Err(_) => {
                snapshot.set_detail_state(id, None, None);
                updates.push(MultiChartDerivedDetailUpdate {
                    item_id: id,
                    detail_series: None,
                    detail_window: None,
                });
                continue;
            }
        }) else {
            snapshot.set_detail_state(id, None, None);
            updates.push(MultiChartDerivedDetailUpdate {
                item_id: id,
                detail_series: None,
                detail_window: None,
            });
            continue;
        };
        match snapshot.evaluate_expression_with_resolution(
            &expression,
            file.as_ref(),
            ExpressionSeriesResolution::Active,
            false,
        ) {
            Ok(evaluated) => {
                let detail_series = ChartSeries::from_points(evaluated.points);
                snapshot.set_detail_state(id, detail_series.clone(), Some(window));
                updates.push(MultiChartDerivedDetailUpdate {
                    item_id: id,
                    detail_series,
                    detail_window: Some(window),
                });
            }
            Err(_) => {
                snapshot.set_detail_state(id, None, None);
                updates.push(MultiChartDerivedDetailUpdate {
                    item_id: id,
                    detail_series: None,
                    detail_window: None,
                });
            }
        }
    }
    MultiChartExpressionRefreshResult::Success {
        revision: request.revision,
        updates,
    }
}

pub fn handle_mchart_expression_refresh(
    tx_events: Sender<AppEvent>,
) -> Sender<MultiChartExpressionRefreshRequest> {
    let (tx_refresh, rx_refresh) = channel::<MultiChartExpressionRefreshRequest>();
    thread::spawn(move || loop {
        let Ok(mut request) = rx_refresh.recv() else {
            return;
        };
        while let Ok(next_request) = rx_refresh.try_recv() {
            request = next_request;
        }
        let _ = tx_events.send(AppEvent::MultiChartExpressionRefresh(
            compute_expression_refresh(request),
        ));
    });
    tx_refresh
}
