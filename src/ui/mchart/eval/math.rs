use super::*;
use crate::{
    configure,
    ui::mchart::functions::{
        find_builtin_mchart_function, find_registered_mchart_function, MchartFunctionExecutionKind,
        MchartReducerKind, MchartRollingKind, MchartScalarCompareKind, MchartUnaryMathKind,
    },
};

struct ExpressionEvalContext<'a> {
    item_series_values: &'a std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    item_scalar_values: &'a std::collections::HashMap<ExpressionItemRef, f64>,
    series_values: &'a std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
    scalar_values: &'a std::collections::HashMap<ExpressionLoadRef, f64>,
    series_sample_count: usize,
}

impl<'a> ExpressionEvalContext<'a> {
    fn new(
        item_series_values: &'a std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
        item_scalar_values: &'a std::collections::HashMap<ExpressionItemRef, f64>,
        series_values: &'a std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
        scalar_values: &'a std::collections::HashMap<ExpressionLoadRef, f64>,
        series_sample_count: usize,
    ) -> Self {
        Self {
            item_series_values,
            item_scalar_values,
            series_values,
            scalar_values,
            series_sample_count,
        }
    }

    fn eval_at(&self, expr: &ExpressionAst, idx: usize) -> Result<f64, String> {
        eval_expression_at(
            expr,
            idx,
            self.item_series_values,
            self.item_scalar_values,
            self.series_values,
            self.scalar_values,
            self.series_sample_count,
        )
    }

    fn eval_scalar(&self, expr: &ExpressionAst) -> Result<f64, String> {
        eval_scalar_expression(
            expr,
            self.item_series_values,
            self.item_scalar_values,
            self.series_values,
            self.scalar_values,
            self.series_sample_count,
        )
    }

    fn eval_window_size(
        &self,
        function_name: &str,
        window_expr: &ExpressionAst,
    ) -> Result<usize, String> {
        let window = self.eval_scalar(window_expr)?;
        if window < 1.0 || window.fract() != 0.0 {
            return Err(format!(
                "{function_name}() window must be a positive integer"
            ));
        }
        Ok(window as usize)
    }

    fn trailing_series_window(
        &self,
        series_expr: &ExpressionAst,
        idx: usize,
        window: usize,
    ) -> Result<Vec<f64>, String> {
        let start = idx.saturating_add(1).saturating_sub(window);
        let mut values = Vec::with_capacity(idx - start + 1);
        for sample_idx in start..=idx {
            values.push(self.eval_at(series_expr, sample_idx)?);
        }
        Ok(values)
    }
}

pub(crate) fn eval_expression_at(
    expr: &ExpressionAst,
    idx: usize,
    item_series_values: &std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    item_scalar_values: &std::collections::HashMap<ExpressionItemRef, f64>,
    series_values: &std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionLoadRef, f64>,
    series_sample_count: usize,
) -> Result<f64, String> {
    match expr {
        ExpressionAst::Number(value) => Ok(*value),
        ExpressionAst::ItemRef(item_ref) => item_scalar_values
            .get(item_ref)
            .copied()
            .or_else(|| {
                item_series_values
                    .get(item_ref)
                    .and_then(|points: &Vec<Point>| points.get(idx).map(|(_, y)| *y))
            })
            .ok_or_else(|| {
                format!(
                    "Chart item {} is unavailable at sample index {}",
                    item_ref.render(),
                    idx
                )
            }),
        ExpressionAst::LoadRef(load_ref) => scalar_values
            .get(load_ref)
            .copied()
            .or_else(|| {
                series_values
                    .get(load_ref)
                    .and_then(|points: &Vec<Point>| points.get(idx).map(|(_, y)| *y))
            })
            .ok_or_else(|| format!("Reference {} is unavailable", load_ref.render())),
        ExpressionAst::UnaryMinus(inner) => Ok(-eval_expression_at(
            inner,
            idx,
            item_series_values,
            item_scalar_values,
            series_values,
            scalar_values,
            series_sample_count,
        )?),
        ExpressionAst::Binary { op, lhs, rhs } => {
            let lhs = eval_expression_at(
                lhs,
                idx,
                item_series_values,
                item_scalar_values,
                series_values,
                scalar_values,
                series_sample_count,
            )?;
            let rhs = eval_expression_at(
                rhs,
                idx,
                item_series_values,
                item_scalar_values,
                series_values,
                scalar_values,
                series_sample_count,
            )?;
            match op {
                ExprBinaryOp::Add => Ok(lhs + rhs),
                ExprBinaryOp::Sub => Ok(lhs - rhs),
                ExprBinaryOp::Mul => Ok(lhs * rhs),
                ExprBinaryOp::Div => {
                    if rhs == 0.0 {
                        Err("Expression division by zero".to_string())
                    } else {
                        Ok(lhs / rhs)
                    }
                }
            }
        }
        ExpressionAst::FunctionCall { name, args } => {
            let ctx = ExpressionEvalContext::new(
                item_series_values,
                item_scalar_values,
                series_values,
                scalar_values,
                series_sample_count,
            );
            let function = find_registered_mchart_function(name)
                .ok_or_else(|| format!("Unsupported function '{name}'"))?;
            let Some(execution) =
                find_builtin_mchart_function(&function.name).map(|builtin| builtin.execution)
            else {
                return match eval_custom_function_result(
                    &function,
                    args,
                    item_series_values,
                    item_scalar_values,
                    series_values,
                    scalar_values,
                    series_sample_count,
                )? {
                    configure::LuaMchartReturnValue::Scalar(value) => Ok(value),
                    configure::LuaMchartReturnValue::Series(values) => {
                        values.get(idx).copied().ok_or_else(|| {
                            format!(
                                "{}() returned {} samples, missing sample index {}",
                                function.name,
                                values.len(),
                                idx
                            )
                        })
                    }
                };
            };
            match execution {
                MchartFunctionExecutionKind::Power => {
                    if args.len() != 2 {
                        return Err("exp() expects exactly 2 arguments".to_string());
                    }
                    let lhs = eval_expression_at(
                        &args[0],
                        idx,
                        item_series_values,
                        item_scalar_values,
                        series_values,
                        scalar_values,
                        series_sample_count,
                    )?;
                    let rhs = eval_expression_at(
                        &args[1],
                        idx,
                        item_series_values,
                        item_scalar_values,
                        series_values,
                        scalar_values,
                        series_sample_count,
                    )?;
                    Ok(lhs.powf(rhs))
                }
                MchartFunctionExecutionKind::UnaryMath(op) => {
                    if args.len() != 1 {
                        return Err(format!("{name}() expects exactly 1 argument"));
                    }
                    apply_unary_math_function(
                        op,
                        name,
                        eval_expression_at(
                            &args[0],
                            idx,
                            item_series_values,
                            item_scalar_values,
                            series_values,
                            scalar_values,
                            series_sample_count,
                        )?,
                    )
                }
                MchartFunctionExecutionKind::Rolling(kind) => {
                    eval_rolling_series_function(kind, args, idx, &ctx)
                }
                MchartFunctionExecutionKind::RollingQuantile => {
                    eval_rolling_quantile_function(args, idx, &ctx)
                }
                MchartFunctionExecutionKind::Threshold => {
                    if args.len() != 2 {
                        return Err("threshold() expects exactly 2 arguments".to_string());
                    }
                    let value = eval_expression_at(
                        &args[0],
                        idx,
                        item_series_values,
                        item_scalar_values,
                        series_values,
                        scalar_values,
                        series_sample_count,
                    )?;
                    let threshold = eval_scalar_expression(
                        &args[1],
                        item_series_values,
                        item_scalar_values,
                        series_values,
                        scalar_values,
                        series_sample_count,
                    )?;
                    Ok(if value >= threshold { 1.0 } else { 0.0 })
                }
                MchartFunctionExecutionKind::Diff => eval_diff_series_function(args, idx, &ctx),
                MchartFunctionExecutionKind::Reducer(_)
                | MchartFunctionExecutionKind::ScalarCompare(_) => eval_scalar_expression(
                    expr,
                    item_series_values,
                    item_scalar_values,
                    series_values,
                    scalar_values,
                    series_sample_count,
                ),
                MchartFunctionExecutionKind::Interp | MchartFunctionExecutionKind::Slice => {
                    Err(format!("Unsupported function '{name}'"))
                }
            }
        }
    }
}

pub(crate) fn eval_scalar_expression(
    expr: &ExpressionAst,
    item_series_values: &std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    item_scalar_values: &std::collections::HashMap<ExpressionItemRef, f64>,
    series_values: &std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionLoadRef, f64>,
    series_sample_count: usize,
) -> Result<f64, String> {
    match expr {
        ExpressionAst::Number(value) => Ok(*value),
        ExpressionAst::ItemRef(item_ref) => item_scalar_values
            .get(item_ref)
            .copied()
            .ok_or_else(|| format!("{} resolves to a series, not a scalar", item_ref.render())),
        ExpressionAst::LoadRef(load_ref) => scalar_values
            .get(load_ref)
            .copied()
            .ok_or_else(|| format!("{} resolves to a series, not a scalar", load_ref.render())),
        ExpressionAst::UnaryMinus(inner) => Ok(-eval_scalar_expression(
            inner,
            item_series_values,
            item_scalar_values,
            series_values,
            scalar_values,
            series_sample_count,
        )?),
        ExpressionAst::Binary { op, lhs, rhs } => {
            let lhs = eval_scalar_expression(
                lhs,
                item_series_values,
                item_scalar_values,
                series_values,
                scalar_values,
                series_sample_count,
            )?;
            let rhs = eval_scalar_expression(
                rhs,
                item_series_values,
                item_scalar_values,
                series_values,
                scalar_values,
                series_sample_count,
            )?;
            match op {
                ExprBinaryOp::Add => Ok(lhs + rhs),
                ExprBinaryOp::Sub => Ok(lhs - rhs),
                ExprBinaryOp::Mul => Ok(lhs * rhs),
                ExprBinaryOp::Div => {
                    if rhs == 0.0 {
                        Err("Expression division by zero".to_string())
                    } else {
                        Ok(lhs / rhs)
                    }
                }
            }
        }
        ExpressionAst::FunctionCall { name, args } => {
            let ctx = ExpressionEvalContext::new(
                item_series_values,
                item_scalar_values,
                series_values,
                scalar_values,
                series_sample_count,
            );
            let function = find_registered_mchart_function(name)
                .ok_or_else(|| format!("Unsupported function '{name}'"))?;
            let Some(execution) =
                find_builtin_mchart_function(&function.name).map(|builtin| builtin.execution)
            else {
                return match eval_custom_function_result(
                    &function,
                    args,
                    item_series_values,
                    item_scalar_values,
                    series_values,
                    scalar_values,
                    series_sample_count,
                )? {
                    configure::LuaMchartReturnValue::Scalar(value) => Ok(value),
                    configure::LuaMchartReturnValue::Series(_) => Err(format!(
                        "{}() resolves to a series, not a scalar",
                        function.name
                    )),
                };
            };
            match execution {
                MchartFunctionExecutionKind::Power => {
                    if args.len() != 2 {
                        return Err("exp() expects exactly 2 arguments".to_string());
                    }
                    let lhs = eval_scalar_expression(
                        &args[0],
                        item_series_values,
                        item_scalar_values,
                        series_values,
                        scalar_values,
                        series_sample_count,
                    )?;
                    let rhs = eval_scalar_expression(
                        &args[1],
                        item_series_values,
                        item_scalar_values,
                        series_values,
                        scalar_values,
                        series_sample_count,
                    )?;
                    Ok(lhs.powf(rhs))
                }
                MchartFunctionExecutionKind::UnaryMath(op) => {
                    if args.len() != 1 {
                        return Err(format!("{name}() expects exactly 1 argument"));
                    }
                    apply_unary_math_function(
                        op,
                        name,
                        eval_scalar_expression(
                            &args[0],
                            item_series_values,
                            item_scalar_values,
                            series_values,
                            scalar_values,
                            series_sample_count,
                        )?,
                    )
                }
                MchartFunctionExecutionKind::Threshold => {
                    if args.len() != 2 {
                        return Err("threshold() expects exactly 2 arguments".to_string());
                    }
                    let value = eval_scalar_expression(
                        &args[0],
                        item_series_values,
                        item_scalar_values,
                        series_values,
                        scalar_values,
                        series_sample_count,
                    )?;
                    let threshold = eval_scalar_expression(
                        &args[1],
                        item_series_values,
                        item_scalar_values,
                        series_values,
                        scalar_values,
                        series_sample_count,
                    )?;
                    Ok(if value >= threshold { 1.0 } else { 0.0 })
                }
                MchartFunctionExecutionKind::Reducer(kind) => {
                    reduce_series_function(name, args, &ctx, reducer_for_kind(kind))
                }
                MchartFunctionExecutionKind::ScalarCompare(kind) => {
                    if args.len() != 2 {
                        return Err(format!("{name}() expects exactly 2 arguments"));
                    }
                    let lhs = eval_scalar_expression(
                        &args[0],
                        item_series_values,
                        item_scalar_values,
                        series_values,
                        scalar_values,
                        series_sample_count,
                    )?;
                    let rhs = eval_scalar_expression(
                        &args[1],
                        item_series_values,
                        item_scalar_values,
                        series_values,
                        scalar_values,
                        series_sample_count,
                    )?;
                    Ok(match kind {
                        MchartScalarCompareKind::Max => lhs.max(rhs),
                        MchartScalarCompareKind::Min => lhs.min(rhs),
                    })
                }
                MchartFunctionExecutionKind::Rolling(_)
                | MchartFunctionExecutionKind::RollingQuantile
                | MchartFunctionExecutionKind::Diff
                | MchartFunctionExecutionKind::Interp
                | MchartFunctionExecutionKind::Slice => {
                    Err(format!("Unsupported function '{name}'"))
                }
            }
        }
    }
}

fn reduce_series_function<F>(
    name: &str,
    args: &[ExpressionAst],
    ctx: &ExpressionEvalContext<'_>,
    reducer: F,
) -> Result<f64, String>
where
    F: FnOnce(&[f64]) -> Result<f64, String>,
{
    if args.len() != 1 {
        return Err(format!("{name}() expects exactly 1 argument"));
    }
    if ctx.series_sample_count == 0 {
        return Err(format!("{name}() requires at least one series input"));
    }
    let mut values = Vec::with_capacity(ctx.series_sample_count);
    for idx in 0..ctx.series_sample_count {
        values.push(ctx.eval_at(&args[0], idx)?);
    }
    reducer(&values)
}

fn eval_custom_function_result(
    function: &crate::configure::registry::MchartFunctionMetadata,
    args: &[ExpressionAst],
    item_series_values: &std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    item_scalar_values: &std::collections::HashMap<ExpressionItemRef, f64>,
    series_values: &std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionLoadRef, f64>,
    series_sample_count: usize,
) -> Result<configure::LuaMchartReturnValue, String> {
    let callback_id = function
        .callback_id
        .as_deref()
        .ok_or_else(|| format!("Unsupported function '{}'", function.name))?;
    if args.len() != function.params.len() {
        return Err(format!(
            "{}() expects exactly {} arguments",
            function.name,
            function.params.len()
        ));
    }
    let resolved_args = function
        .params
        .iter()
        .zip(args.iter())
        .map(|(param, arg)| match param.value_kind {
            crate::configure::registry::RegistryValueKind::Scalar => eval_scalar_expression(
                arg,
                item_series_values,
                item_scalar_values,
                series_values,
                scalar_values,
                series_sample_count,
            )
            .map(configure::LuaMchartArgValue::Scalar),
            crate::configure::registry::RegistryValueKind::Series => eval_series_expression_points(
                arg,
                item_series_values,
                item_scalar_values,
                series_values,
                scalar_values,
                series_sample_count,
            )
            .map(configure::LuaMchartArgValue::Series),
            _ => Err(format!(
                "{}() param '{}' has an unsupported value kind",
                function.name, param.name
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    configure::run_registered_mchart_function(callback_id, &resolved_args, function.return_kind)
}

fn eval_series_expression_points(
    expr: &ExpressionAst,
    item_series_values: &std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    item_scalar_values: &std::collections::HashMap<ExpressionItemRef, f64>,
    series_values: &std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionLoadRef, f64>,
    series_sample_count: usize,
) -> Result<Vec<Point>, String> {
    let x_points = first_series_points(item_series_values, series_values)
        .ok_or_else(|| "Series arguments must reference at least one loaded series".to_string())?;
    if x_points.len() != series_sample_count {
        return Err(
            "Series argument sample count is inconsistent with the expression context".to_string(),
        );
    }
    let mut points = Vec::with_capacity(series_sample_count);
    for idx in 0..series_sample_count {
        let y = eval_expression_at(
            expr,
            idx,
            item_series_values,
            item_scalar_values,
            series_values,
            scalar_values,
            series_sample_count,
        )?;
        points.push((x_points[idx].0, y));
    }
    Ok(points)
}

fn first_series_points<'a>(
    item_series_values: &'a std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    series_values: &'a std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
) -> Option<&'a [Point]> {
    item_series_values
        .values()
        .next()
        .map(Vec::as_slice)
        .or_else(|| series_values.values().next().map(Vec::as_slice))
}

fn eval_rolling_series_function(
    kind: MchartRollingKind,
    args: &[ExpressionAst],
    idx: usize,
    ctx: &ExpressionEvalContext<'_>,
) -> Result<f64, String> {
    if args.len() != 2 {
        return Err(format!(
            "{}() expects exactly 2 arguments",
            rolling_function_name(kind)
        ));
    }
    let window = ctx.eval_window_size(rolling_function_name(kind), &args[1])?;
    let values = ctx.trailing_series_window(&args[0], idx, window)?;
    let mut values = values;
    match kind {
        MchartRollingKind::Mean => Ok(values.iter().sum::<f64>() / values.len() as f64),
        MchartRollingKind::Median => rolling_quantile_from_sorted(&mut values, 0.5),
        MchartRollingKind::Stddev => {
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            let variance = if values.len() <= 1 {
                0.0
            } else {
                values
                    .iter()
                    .map(|value| {
                        let delta = *value - mean;
                        delta * delta
                    })
                    .sum::<f64>()
                    / values.len() as f64
            };
            Ok(variance.sqrt())
        }
        MchartRollingKind::Min => Ok(values.iter().copied().fold(f64::INFINITY, f64::min)),
        MchartRollingKind::Max => Ok(values.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
    }
}

fn eval_rolling_quantile_function(
    args: &[ExpressionAst],
    idx: usize,
    ctx: &ExpressionEvalContext<'_>,
) -> Result<f64, String> {
    if args.len() != 3 {
        return Err("rolling_quantile() expects exactly 3 arguments".to_string());
    }
    let window = ctx.eval_window_size("rolling_quantile", &args[1])?;
    let quantile = ctx.eval_scalar(&args[2])?;
    if !(0.0..=1.0).contains(&quantile) {
        return Err("rolling_quantile() quantile must be between 0 and 1".to_string());
    }
    let values = ctx.trailing_series_window(&args[0], idx, window)?;
    let mut values = values;
    rolling_quantile_from_sorted(&mut values, quantile)
}

fn eval_diff_series_function(
    args: &[ExpressionAst],
    idx: usize,
    ctx: &ExpressionEvalContext<'_>,
) -> Result<f64, String> {
    if args.len() != 1 {
        return Err("diff() expects exactly 1 argument".to_string());
    }
    if idx == 0 {
        return Ok(0.0);
    }
    let current = ctx.eval_at(&args[0], idx)?;
    let previous = ctx.eval_at(&args[0], idx - 1)?;
    Ok(current - previous)
}

fn rolling_quantile_from_sorted(values: &mut [f64], quantile: f64) -> Result<f64, String> {
    if values.is_empty() {
        return Err("rolling quantile requires at least one sample".to_string());
    }
    values.sort_by(f64::total_cmp);
    if values.len() == 1 {
        return Ok(values[0]);
    }
    let position = quantile * (values.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        Ok(values[lower])
    } else {
        let weight = position - lower as f64;
        Ok(values[lower] * (1.0 - weight) + values[upper] * weight)
    }
}

fn apply_unary_math_function(
    kind: MchartUnaryMathKind,
    name: &str,
    value: f64,
) -> Result<f64, String> {
    let output = match kind {
        MchartUnaryMathKind::Abs => value.abs(),
        MchartUnaryMathKind::Sqrt => value.sqrt(),
        MchartUnaryMathKind::Ln => value.ln(),
        MchartUnaryMathKind::Log10 => value.log10(),
        MchartUnaryMathKind::Sin => value.sin(),
        MchartUnaryMathKind::Cos => value.cos(),
        MchartUnaryMathKind::Tan => value.tan(),
        MchartUnaryMathKind::Floor => value.floor(),
        MchartUnaryMathKind::Ceil => value.ceil(),
        MchartUnaryMathKind::Round => value.round(),
    };
    if output.is_finite() {
        Ok(output)
    } else {
        Err(format!("{name}() produced a non-finite value"))
    }
}

fn reducer_for_kind(kind: MchartReducerKind) -> impl FnOnce(&[f64]) -> Result<f64, String> {
    move |values| match kind {
        MchartReducerKind::Mean => Ok(values.iter().sum::<f64>() / values.len() as f64),
        MchartReducerKind::Min => Ok(values.iter().copied().fold(f64::INFINITY, f64::min)),
        MchartReducerKind::Max => Ok(values.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
        MchartReducerKind::Stddev => {
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            let variance = if values.len() <= 1 {
                0.0
            } else {
                values
                    .iter()
                    .map(|value| {
                        let delta = *value - mean;
                        delta * delta
                    })
                    .sum::<f64>()
                    / values.len() as f64
            };
            Ok(variance.sqrt())
        }
        MchartReducerKind::Len => Ok(values.len() as f64),
    }
}

fn rolling_function_name(kind: MchartRollingKind) -> &'static str {
    match kind {
        MchartRollingKind::Mean => "rolling_mean",
        MchartRollingKind::Median => "rolling_median",
        MchartRollingKind::Stddev => "rolling_stddev",
        MchartRollingKind::Min => "rolling_min",
        MchartRollingKind::Max => "rolling_max",
    }
}
