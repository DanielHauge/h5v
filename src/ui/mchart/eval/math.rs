use super::*;
use crate::ui::mchart::functions::{
    find_mchart_function, MchartFunctionExecutionKind, MchartReducerKind, MchartRollingKind,
    MchartScalarCompareKind, MchartUnaryMathKind,
};

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
            let function = find_mchart_function(name)
                .ok_or_else(|| format!("Unsupported function '{name}'"))?;
            match function.execution {
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
                MchartFunctionExecutionKind::Rolling(kind) => eval_rolling_series_function(
                    kind,
                    args,
                    idx,
                    item_series_values,
                    item_scalar_values,
                    series_values,
                    scalar_values,
                    series_sample_count,
                ),
                MchartFunctionExecutionKind::RollingQuantile => eval_rolling_quantile_function(
                    args,
                    idx,
                    item_series_values,
                    item_scalar_values,
                    series_values,
                    scalar_values,
                    series_sample_count,
                ),
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
                MchartFunctionExecutionKind::Diff => eval_diff_series_function(
                    args,
                    idx,
                    item_series_values,
                    item_scalar_values,
                    series_values,
                    scalar_values,
                    series_sample_count,
                ),
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
            let function = find_mchart_function(name)
                .ok_or_else(|| format!("Unsupported function '{name}'"))?;
            match function.execution {
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
                MchartFunctionExecutionKind::Reducer(kind) => reduce_series_function(
                    name,
                    args,
                    item_series_values,
                    item_scalar_values,
                    series_values,
                    scalar_values,
                    series_sample_count,
                    reducer_for_kind(kind),
                ),
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
    item_series_values: &std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    item_scalar_values: &std::collections::HashMap<ExpressionItemRef, f64>,
    series_values: &std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionLoadRef, f64>,
    series_sample_count: usize,
    reducer: F,
) -> Result<f64, String>
where
    F: FnOnce(&[f64]) -> Result<f64, String>,
{
    if args.len() != 1 {
        return Err(format!("{name}() expects exactly 1 argument"));
    }
    if series_sample_count == 0 {
        return Err(format!("{name}() requires at least one series input"));
    }
    let mut values = Vec::with_capacity(series_sample_count);
    for idx in 0..series_sample_count {
        values.push(eval_expression_at(
            &args[0],
            idx,
            item_series_values,
            item_scalar_values,
            series_values,
            scalar_values,
            series_sample_count,
        )?);
    }
    reducer(&values)
}

fn eval_rolling_series_function(
    kind: MchartRollingKind,
    args: &[ExpressionAst],
    idx: usize,
    item_series_values: &std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    item_scalar_values: &std::collections::HashMap<ExpressionItemRef, f64>,
    series_values: &std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionLoadRef, f64>,
    series_sample_count: usize,
) -> Result<f64, String> {
    if args.len() != 2 {
        return Err(format!(
            "{}() expects exactly 2 arguments",
            rolling_function_name(kind)
        ));
    }
    let window = eval_window_size(
        rolling_function_name(kind),
        &args[1],
        item_series_values,
        item_scalar_values,
        series_values,
        scalar_values,
        series_sample_count,
    )?;
    let values = eval_trailing_series_window(
        &args[0],
        idx,
        window,
        item_series_values,
        item_scalar_values,
        series_values,
        scalar_values,
        series_sample_count,
    )?;
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
    item_series_values: &std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    item_scalar_values: &std::collections::HashMap<ExpressionItemRef, f64>,
    series_values: &std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionLoadRef, f64>,
    series_sample_count: usize,
) -> Result<f64, String> {
    if args.len() != 3 {
        return Err("rolling_quantile() expects exactly 3 arguments".to_string());
    }
    let window = eval_window_size(
        "rolling_quantile",
        &args[1],
        item_series_values,
        item_scalar_values,
        series_values,
        scalar_values,
        series_sample_count,
    )?;
    let quantile = eval_scalar_expression(
        &args[2],
        item_series_values,
        item_scalar_values,
        series_values,
        scalar_values,
        series_sample_count,
    )?;
    if !(0.0..=1.0).contains(&quantile) {
        return Err("rolling_quantile() quantile must be between 0 and 1".to_string());
    }
    let values = eval_trailing_series_window(
        &args[0],
        idx,
        window,
        item_series_values,
        item_scalar_values,
        series_values,
        scalar_values,
        series_sample_count,
    )?;
    let mut values = values;
    rolling_quantile_from_sorted(&mut values, quantile)
}

fn eval_diff_series_function(
    args: &[ExpressionAst],
    idx: usize,
    item_series_values: &std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    item_scalar_values: &std::collections::HashMap<ExpressionItemRef, f64>,
    series_values: &std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionLoadRef, f64>,
    series_sample_count: usize,
) -> Result<f64, String> {
    if args.len() != 1 {
        return Err("diff() expects exactly 1 argument".to_string());
    }
    if idx == 0 {
        return Ok(0.0);
    }
    let current = eval_expression_at(
        &args[0],
        idx,
        item_series_values,
        item_scalar_values,
        series_values,
        scalar_values,
        series_sample_count,
    )?;
    let previous = eval_expression_at(
        &args[0],
        idx - 1,
        item_series_values,
        item_scalar_values,
        series_values,
        scalar_values,
        series_sample_count,
    )?;
    Ok(current - previous)
}

fn eval_window_size(
    function_name: &str,
    window_expr: &ExpressionAst,
    item_series_values: &std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    item_scalar_values: &std::collections::HashMap<ExpressionItemRef, f64>,
    series_values: &std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionLoadRef, f64>,
    series_sample_count: usize,
) -> Result<usize, String> {
    let window = eval_scalar_expression(
        window_expr,
        item_series_values,
        item_scalar_values,
        series_values,
        scalar_values,
        series_sample_count,
    )?;
    if window < 1.0 || window.fract() != 0.0 {
        return Err(format!(
            "{function_name}() window must be a positive integer"
        ));
    }
    Ok(window as usize)
}

fn eval_trailing_series_window(
    series_expr: &ExpressionAst,
    idx: usize,
    window: usize,
    item_series_values: &std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    item_scalar_values: &std::collections::HashMap<ExpressionItemRef, f64>,
    series_values: &std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionLoadRef, f64>,
    series_sample_count: usize,
) -> Result<Vec<f64>, String> {
    let start = idx.saturating_add(1).saturating_sub(window);
    let mut values = Vec::with_capacity(idx - start + 1);
    for sample_idx in start..=idx {
        values.push(eval_expression_at(
            series_expr,
            sample_idx,
            item_series_values,
            item_scalar_values,
            series_values,
            scalar_values,
            series_sample_count,
        )?);
    }
    Ok(values)
}

fn rolling_quantile_from_sorted(values: &mut Vec<f64>, quantile: f64) -> Result<f64, String> {
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
