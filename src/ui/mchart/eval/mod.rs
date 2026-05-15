use hdf5_metno::{
    types::{FloatSize, IntSize, TypeDescriptor},
    Attribute, Dataset, File, Hyperslab, Selection, SliceOrIndex,
};
use ndarray::IxDyn;

use crate::data::{
    validate_preview_selection_shape, DatasetPlotingData, PreviewSelection, SliceSelection,
};

use super::{
    expression::{
        ExprBinaryOp, ExpressionAst, ExpressionDatasetSelector, ExpressionItemRef,
        ExpressionItemTarget, ExpressionLoadRef, ExpressionObjectTarget,
    },
    sanitize_chart_points, ChartItemId, DerivedExpressionKind, MultiChartState, Point,
};
mod loads;
mod math;

#[derive(Debug, Clone)]
pub(super) struct ExpressionSeriesInput {
    pub(super) label: String,
    pub(super) points: Vec<Point>,
}

pub(super) struct EvaluatedExpression {
    pub(super) points: Vec<Point>,
    pub(super) scalar_value: Option<f64>,
    pub(super) kind: DerivedExpressionKind,
    pub(super) input_ids: Vec<ChartItemId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpressionSeriesResolution {
    Overview,
    Active,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ResolvedExpressionLoad {
    Scalar(f64),
    Series(Vec<Point>),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ResolvedExpressionItemValue {
    Scalar(f64),
    Series(Vec<Point>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ValidatedExpressionLoad {
    Scalar,
    Series { len: usize },
}

pub(super) use loads::{
    normalize_absolute_object_path, resolve_expression_load_value, validate_expression_load_ref,
};
pub(super) use math::{eval_expression_at, eval_scalar_expression};
enum ExpressionArraySelection {
    Scalar(Vec<usize>),
    Series(PreviewSelection),
}

pub(super) fn validate_expression_series_compatibility(
    referenced: &[ExpressionSeriesInput],
    expected_len: usize,
    require_matching_x: bool,
) -> Result<(), String> {
    let Some(first) = referenced.first() else {
        return Err("Expression must reference at least one chart item".to_string());
    };
    for item in &referenced[1..] {
        if item.points.len() != expected_len {
            return Err(format!(
                "Expression series lengths must match exactly: {} has len {}, but {} has len {}",
                first.label,
                expected_len,
                item.label,
                item.points.len()
            ));
        }
        if require_matching_x {
            for idx in 0..expected_len {
                if item.points[idx].0 != first.points[idx].0 {
                    return Err(format!(
                        "Expression x-values must match exactly across referenced items; mismatch at sample index {}",
                        idx
                    ));
                }
            }
        }
    }
    Ok(())
}

fn infer_expression_array_selection(
    shape: &[usize],
    selectors: Option<&[ExpressionDatasetSelector]>,
    reference: &str,
) -> Result<ExpressionArraySelection, String> {
    if shape.is_empty() {
        if selectors.is_some() {
            return Err(format!(
                "Reference {reference} points to a scalar and cannot use selectors"
            ));
        }
        return Ok(ExpressionArraySelection::Scalar(Vec::new()));
    }
    match selectors {
        None => {
            if shape.len() != 1 {
                return Err(format!(
                    "Reference {reference} needs an explicit selector like load(/path)[..,0] for rank-{} arrays",
                    shape.len()
                ));
            }
            Ok(ExpressionArraySelection::Series(PreviewSelection {
                x: 0,
                index: vec![0],
                slice: SliceSelection::All,
            }))
        }
        Some(selectors) => {
            if selectors.len() != shape.len() {
                return Err(format!(
                    "Reference {reference} must provide exactly {} selectors",
                    shape.len()
                ));
            }
            let mut x = None;
            let mut index = vec![0; shape.len()];
            let mut slice = SliceSelection::All;
            for (dim, selector) in selectors.iter().enumerate() {
                match selector {
                    ExpressionDatasetSelector::All => {
                        if x.replace(dim).is_some() {
                            return Err(format!(
                                "Reference {reference} must contain at most one slice axis selector"
                            ));
                        }
                    }
                    ExpressionDatasetSelector::Index(selected) => {
                        if *selected >= shape[dim] {
                            return Err(format!(
                                "Reference {reference} selects index {} out of bounds for dim {} with length {}",
                                selected, dim, shape[dim]
                            ));
                        }
                        index[dim] = *selected;
                    }
                    ExpressionDatasetSelector::Slice { start, end } => {
                        if x.replace(dim).is_some() {
                            return Err(format!(
                                "Reference {reference} must contain at most one slice axis selector"
                            ));
                        }
                        let start = start.unwrap_or(0);
                        let end = end.unwrap_or(shape[dim]);
                        if end <= start {
                            return Err(format!(
                                "Reference {reference} must use an increasing slice for dim {}",
                                dim
                            ));
                        }
                        if end > shape[dim] {
                            return Err(format!(
                                "Reference {reference} selects slice {}..{} out of bounds for dim {} with length {}",
                                start, end, dim, shape[dim]
                            ));
                        }
                        slice = SliceSelection::FromTo(start, end);
                    }
                }
            }
            Ok(match x {
                Some(x) => ExpressionArraySelection::Series(PreviewSelection { x, index, slice }),
                None => ExpressionArraySelection::Scalar(index),
            })
        }
    }
}

pub(super) fn resolve_expression_item_value(
    state: &MultiChartState,
    item_ref: &ExpressionItemRef,
    resolution: ExpressionSeriesResolution,
) -> Result<ResolvedExpressionItemValue, String> {
    let item = match &item_ref.target {
        ExpressionItemTarget::Id(id) => state
            .item_by_id(*id)
            .ok_or_else(|| format!("Unknown chart item reference ${}", id.0))?,
        ExpressionItemTarget::Name(name) => state
            .item_by_name(name)
            .ok_or_else(|| format!("Unknown chart item reference ${name}"))?,
    };
    if let Some(value) = item.scalar_value {
        if item_ref.slice.is_some() {
            return Err(format!(
                "Scalar chart item {} cannot use a series slice",
                item_ref.render()
            ));
        }
        return require_finite_scalar_value(value, &item_ref.render())
            .map(ResolvedExpressionItemValue::Scalar);
    }
    if !item.has_loaded_series() {
        return Err(format!(
            "Chart item reference {} is still loading",
            item_ref.render()
        ));
    }
    let points = sanitize_chart_points(match resolution {
        ExpressionSeriesResolution::Overview => item.overview_series().points.clone(),
        ExpressionSeriesResolution::Active => item.active_series().points.clone(),
    });
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
    if points.is_empty() {
        return Err(format!(
            "Chart item reference {} resolved to no finite points",
            item_ref.render()
        ));
    }
    Ok(ResolvedExpressionItemValue::Series(points))
}

pub(super) fn dataset_ploting_data_from_points(
    points: Vec<Point>,
) -> Result<DatasetPlotingData, String> {
    let points = sanitize_chart_points(points);
    let Some((_, first_y)) = points.first().copied() else {
        return Err("Expression preview has no finite points".to_string());
    };
    let mut min = first_y;
    let mut max = first_y;
    for (_, y) in &points {
        min = min.min(*y);
        max = max.max(*y);
    }
    Ok(DatasetPlotingData {
        length: points.len(),
        data: points,
        min,
        max,
    })
}

fn require_finite_scalar_value(value: f64, reference: &str) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!(
            "Scalar reference {reference} resolved to a non-finite value"
        ))
    }
}

pub(super) fn preview_selection_len(
    selection: &PreviewSelection,
    shape: &[usize],
) -> Result<usize, String> {
    validate_preview_selection_shape(shape, selection).map_err(|error| error.to_string())?;
    Ok(match selection.slice {
        SliceSelection::All => shape[selection.x],
        SliceSelection::FromTo(start, end) => end.saturating_sub(start),
    })
}
