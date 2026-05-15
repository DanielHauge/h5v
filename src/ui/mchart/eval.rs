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
        ExpressionLoadRef, ExpressionObjectTarget, ExpressionScalarRef, ExpressionSeriesRef,
    },
    sanitize_chart_points, ChartItemId, DerivedExpressionKind, MultiChartState, Point,
};

#[derive(Debug, Clone)]
pub(super) struct ExpressionSeriesInput {
    pub(super) label: String,
    pub(super) points: Vec<Point>,
}

pub(super) struct EvaluatedExpression {
    pub(super) points: Vec<Point>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ValidatedExpressionLoad {
    Scalar,
    Series { len: usize },
}

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
) -> Result<Vec<Point>, String> {
    let item = state
        .item_by_id(item_ref.id)
        .ok_or_else(|| format!("Unknown chart item reference ${}", item_ref.id.0))?;
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
    Ok(points)
}

pub(super) fn validate_expression_load_ref(
    state: &MultiChartState,
    file: &File,
    load_ref: &ExpressionLoadRef,
    resolution: ExpressionSeriesResolution,
    allow_external_series: bool,
) -> Result<ValidatedExpressionLoad, String> {
    match (&load_ref.target, &load_ref.attr_name) {
        (ExpressionObjectTarget::ItemRef(id), None) => {
            let points = resolve_expression_item_points_by_selector(
                state,
                *id,
                load_ref.selectors.as_deref(),
                resolution,
                &load_ref.render(),
            )?;
            Ok(match points {
                ResolvedExpressionLoad::Scalar(_) => ValidatedExpressionLoad::Scalar,
                ResolvedExpressionLoad::Series(points) => {
                    ValidatedExpressionLoad::Series { len: points.len() }
                }
            })
        }
        (target, Some(attr_name)) => {
            let object_path = resolve_expression_target_path(state, target, &load_ref.render())?;
            let attr = open_expression_attribute(file, &object_path, attr_name)?;
            validate_expression_attribute_load(&attr, load_ref)
        }
        (ExpressionObjectTarget::AbsolutePath(path), None) => {
            if !allow_external_series && load_ref.selectors.is_some() {
                return Err(format!(
                    "Reference {} cannot refine from viewport detail yet",
                    load_ref.render()
                ));
            }
            let object_path = normalize_absolute_object_path(&path)?;
            let dataset = file.dataset(&object_path).map_err(|error| {
                format!(
                    "Reference {} could not open dataset '{}': {}",
                    load_ref.render(),
                    object_path,
                    error
                )
            })?;
            validate_expression_dataset_load(&dataset, load_ref)
        }
    }
}

pub(super) fn resolve_expression_load_value(
    state: &MultiChartState,
    file: &File,
    load_ref: &ExpressionLoadRef,
    resolution: ExpressionSeriesResolution,
    allow_external_series: bool,
) -> Result<ResolvedExpressionLoad, String> {
    match (&load_ref.target, &load_ref.attr_name) {
        (ExpressionObjectTarget::ItemRef(id), None) => resolve_expression_item_points_by_selector(
            state,
            *id,
            load_ref.selectors.as_deref(),
            resolution,
            &load_ref.render(),
        ),
        (target, Some(attr_name)) => {
            let object_path = resolve_expression_target_path(state, target, &load_ref.render())?;
            let attr = open_expression_attribute(file, &object_path, attr_name)?;
            resolve_expression_attribute_load(&attr, load_ref)
        }
        (ExpressionObjectTarget::AbsolutePath(path), None) => {
            if !allow_external_series && load_ref.selectors.is_some() {
                return Err(format!(
                    "Reference {} cannot refine from viewport detail yet",
                    load_ref.render()
                ));
            }
            let object_path = normalize_absolute_object_path(&path)?;
            let dataset = file.dataset(&object_path).map_err(|error| {
                format!(
                    "Reference {} could not open dataset '{}': {}",
                    load_ref.render(),
                    object_path,
                    error
                )
            })?;
            resolve_expression_dataset_load(&dataset, load_ref)
        }
    }
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

pub(super) fn eval_expression_at(
    expr: &ExpressionAst,
    idx: usize,
    item_values: &std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    series_values: &std::collections::HashMap<ExpressionLoadRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionLoadRef, f64>,
) -> Result<f64, String> {
    match expr {
        ExpressionAst::Number(value) => Ok(*value),
        ExpressionAst::ItemRef(item_ref) => item_values
            .get(item_ref)
            .and_then(|points: &Vec<Point>| points.get(idx).map(|(_, y)| *y))
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
            item_values,
            series_values,
            scalar_values,
        )?),
        ExpressionAst::Binary { op, lhs, rhs } => {
            let lhs = eval_expression_at(lhs, idx, item_values, series_values, scalar_values)?;
            let rhs = eval_expression_at(rhs, idx, item_values, series_values, scalar_values)?;
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
    }
}

pub(super) fn resolve_expression_scalar_values(
    state: &MultiChartState,
    file: Option<&File>,
    refs: &[ExpressionScalarRef],
) -> Result<std::collections::HashMap<ExpressionScalarRef, f64>, String> {
    if refs.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let file = file.ok_or_else(|| {
        "Scalar references require an open file handle, but no file is loaded".to_string()
    })?;
    let mut values = std::collections::HashMap::with_capacity(refs.len());
    for scalar_ref in refs {
        let value = resolve_expression_scalar_value(state, file, scalar_ref)?;
        values.insert(scalar_ref.clone(), value);
    }
    Ok(values)
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

pub(super) fn resolve_expression_series_values(
    state: &MultiChartState,
    file: Option<&File>,
    refs: &[ExpressionSeriesRef],
    resolution: ExpressionSeriesResolution,
    allow_external_series: bool,
) -> Result<std::collections::HashMap<ExpressionSeriesRef, Vec<Point>>, String> {
    if refs.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let file = file.ok_or_else(|| {
        "Series references require an open file handle, but no file is loaded".to_string()
    })?;
    let mut series = std::collections::HashMap::with_capacity(refs.len());
    for series_ref in refs {
        let points = resolve_expression_series_value(
            state,
            file,
            series_ref,
            resolution,
            allow_external_series,
        )?;
        series.insert(series_ref.clone(), points);
    }
    Ok(series)
}

pub(super) fn resolve_expression_series_value(
    state: &MultiChartState,
    file: &File,
    series_ref: &ExpressionSeriesRef,
    resolution: ExpressionSeriesResolution,
    allow_external_series: bool,
) -> Result<Vec<Point>, String> {
    match resolve_expression_load_value(state, file, series_ref, resolution, allow_external_series)?
    {
        ResolvedExpressionLoad::Series(points) => Ok(points),
        ResolvedExpressionLoad::Scalar(_) => Err(format!(
            "Reference {} resolved to a scalar, but a series is required",
            series_ref.render()
        )),
    }
}

pub(super) fn validate_expression_series_ref(
    state: &MultiChartState,
    file: &File,
    series_ref: &ExpressionSeriesRef,
    allow_external_series: bool,
) -> Result<usize, String> {
    match validate_expression_load_ref(
        state,
        file,
        series_ref,
        ExpressionSeriesResolution::Overview,
        allow_external_series,
    )? {
        ValidatedExpressionLoad::Series { len } => Ok(len),
        ValidatedExpressionLoad::Scalar => Err(format!(
            "Reference {} resolved to a scalar, but a series is required",
            series_ref.render()
        )),
    }
}

pub(super) fn validate_expression_scalar_ref(
    state: &MultiChartState,
    file: &File,
    scalar_ref: &ExpressionScalarRef,
) -> Result<(), String> {
    match validate_expression_load_ref(
        state,
        file,
        scalar_ref,
        ExpressionSeriesResolution::Overview,
        true,
    )? {
        ValidatedExpressionLoad::Scalar => Ok(()),
        ValidatedExpressionLoad::Series { .. } => Err(format!(
            "Reference {} resolved to a series, but a scalar is required",
            scalar_ref.render()
        )),
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

fn resolve_expression_item_points_by_selector(
    state: &MultiChartState,
    id: ChartItemId,
    selectors: Option<&[ExpressionDatasetSelector]>,
    resolution: ExpressionSeriesResolution,
    reference: &str,
) -> Result<ResolvedExpressionLoad, String> {
    let item = state
        .item_by_id(id)
        .ok_or_else(|| format!("Unknown chart item reference ${}", id.0))?;
    if !item.has_loaded_series() {
        return Err(format!("Chart item reference ${} is still loading", id.0));
    }
    let points = sanitize_chart_points(match resolution {
        ExpressionSeriesResolution::Overview => item.overview_series().points.clone(),
        ExpressionSeriesResolution::Active => item.active_series().points.clone(),
    });
    let shape = [points.len()];
    match infer_expression_array_selection(&shape, selectors, reference)? {
        ExpressionArraySelection::Series(selection) => {
            let len = preview_selection_len(&selection, &shape)?;
            let start = match selection.slice {
                SliceSelection::All => 0,
                SliceSelection::FromTo(start, _) => start,
            };
            let series = points.into_iter().skip(start).take(len).collect::<Vec<_>>();
            if series.is_empty() {
                return Err(format!(
                    "Reference {reference} resolved to no finite points"
                ));
            }
            Ok(ResolvedExpressionLoad::Series(series))
        }
        ExpressionArraySelection::Scalar(indexes) => {
            let index = indexes.first().copied().unwrap_or_default();
            let (_, value) = points.get(index).copied().ok_or_else(|| {
                format!(
                    "Reference {reference} selects index {} out of bounds",
                    index
                )
            })?;
            require_finite_scalar_value(value, reference).map(ResolvedExpressionLoad::Scalar)
        }
    }
}

fn validate_expression_dataset_load(
    dataset: &Dataset,
    load_ref: &ExpressionLoadRef,
) -> Result<ValidatedExpressionLoad, String> {
    let shape = dataset.shape();
    let selection = infer_expression_array_selection(
        &shape,
        load_ref.selectors.as_deref(),
        &load_ref.render(),
    )?;
    let dtype = dataset.dtype().map_err(|error| {
        format!(
            "Failed to inspect dataset type for {}: {}",
            load_ref.render(),
            error
        )
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!(
            "Failed to inspect dataset type for {}: {}",
            load_ref.render(),
            error
        )
    })?;
    if !is_numeric_type_descriptor(&type_desc) {
        return Err(format!(
            "Reference {} must be numeric; got {}",
            load_ref.render(),
            type_desc
        ));
    }
    Ok(match selection {
        ExpressionArraySelection::Scalar(_) => ValidatedExpressionLoad::Scalar,
        ExpressionArraySelection::Series(preview_selection) => ValidatedExpressionLoad::Series {
            len: preview_selection_len(&preview_selection, &shape)?,
        },
    })
}

fn resolve_expression_dataset_load(
    dataset: &Dataset,
    load_ref: &ExpressionLoadRef,
) -> Result<ResolvedExpressionLoad, String> {
    let shape = dataset.shape();
    match infer_expression_array_selection(
        &shape,
        load_ref.selectors.as_deref(),
        &load_ref.render(),
    )? {
        ExpressionArraySelection::Series(preview_selection) => {
            read_expression_dataset_points(dataset, load_ref, &preview_selection)
                .map(ResolvedExpressionLoad::Series)
        }
        ExpressionArraySelection::Scalar(indexes) => read_expression_numeric_scalar_dataset_value(
            dataset,
            &load_ref.render(),
            Some(&indexes),
        )
        .and_then(|value| require_finite_scalar_value(value, &load_ref.render()))
        .map(ResolvedExpressionLoad::Scalar),
    }
}

fn validate_expression_attribute_load(
    attr: &Attribute,
    load_ref: &ExpressionLoadRef,
) -> Result<ValidatedExpressionLoad, String> {
    let shape = attr.shape();
    let selection = infer_expression_array_selection(
        &shape,
        load_ref.selectors.as_deref(),
        &load_ref.render(),
    )?;
    let dtype = attr.dtype().map_err(|error| {
        format!(
            "Failed to inspect attribute type for {}: {}",
            load_ref.render(),
            error
        )
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!(
            "Failed to inspect attribute type for {}: {}",
            load_ref.render(),
            error
        )
    })?;
    if !is_numeric_type_descriptor(&type_desc) {
        return Err(format!(
            "Reference {} must be numeric; got {}",
            load_ref.render(),
            type_desc
        ));
    }
    Ok(match selection {
        ExpressionArraySelection::Scalar(_) => ValidatedExpressionLoad::Scalar,
        ExpressionArraySelection::Series(_) => {
            if shape.len() != 1 {
                return Err(format!(
                    "Reference {} currently supports only rank-1 series attributes",
                    load_ref.render()
                ));
            }
            ValidatedExpressionLoad::Series { len: shape[0] }
        }
    })
}

fn resolve_expression_attribute_load(
    attr: &Attribute,
    load_ref: &ExpressionLoadRef,
) -> Result<ResolvedExpressionLoad, String> {
    let shape = attr.shape();
    match infer_expression_array_selection(
        &shape,
        load_ref.selectors.as_deref(),
        &load_ref.render(),
    )? {
        ExpressionArraySelection::Series(_) => {
            if shape.len() != 1 {
                return Err(format!(
                    "Reference {} currently supports only rank-1 series attributes",
                    load_ref.render()
                ));
            }
            read_expression_numeric_series_attr(attr, &load_ref.render())
                .map(ResolvedExpressionLoad::Series)
        }
        ExpressionArraySelection::Scalar(indexes) => {
            read_expression_numeric_scalar_attr_value(attr, &load_ref.render(), Some(&indexes))
                .and_then(|value| require_finite_scalar_value(value, &load_ref.render()))
                .map(ResolvedExpressionLoad::Scalar)
        }
    }
}

pub(super) fn read_expression_dataset_points(
    dataset: &Dataset,
    load_ref: &ExpressionLoadRef,
    preview_selection: &PreviewSelection,
) -> Result<Vec<Point>, String> {
    let shape = dataset.shape();
    let selection = preview_selection_to_hyperslab(&shape, preview_selection)?;
    let dtype = dataset.dtype().map_err(|error| {
        format!(
            "Failed to inspect dataset type for {}: {}",
            load_ref.render(),
            error
        )
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!(
            "Failed to inspect dataset type for {}: {}",
            load_ref.render(),
            error
        )
    })?;

    let values = match type_desc {
        TypeDescriptor::Integer(IntSize::U1) => dataset
            .read_slice_1d::<i8, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Integer(IntSize::U2) => dataset
            .read_slice_1d::<i16, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Integer(IntSize::U4) => dataset
            .read_slice_1d::<i32, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Integer(IntSize::U8) => dataset
            .read_slice_1d::<i64, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Unsigned(IntSize::U1) => dataset
            .read_slice_1d::<u8, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Unsigned(IntSize::U2) => dataset
            .read_slice_1d::<u16, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Unsigned(IntSize::U4) => dataset
            .read_slice_1d::<u32, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Unsigned(IntSize::U8) => dataset
            .read_slice_1d::<u64, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Float(FloatSize::U4) => dataset
            .read_slice_1d::<f32, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Float(FloatSize::U8) => dataset
            .read_slice_1d::<f64, _>(selection.clone())
            .map(|values| values.into_iter().collect::<Vec<_>>()),
        TypeDescriptor::Boolean => dataset.read_slice_1d::<bool, _>(selection).map(|values| {
            values
                .into_iter()
                .map(|value| if value { 1.0 } else { 0.0 })
                .collect::<Vec<_>>()
        }),
        other => {
            return Err(format!(
                "Series reference {} must be numeric; got {}",
                load_ref.render(),
                other
            ))
        }
    }
    .map_err(|error| format!("Failed reading {}: {}", load_ref.render(), error))?;

    let points = sanitize_chart_points(
        values
            .into_iter()
            .enumerate()
            .map(|(idx, value)| (idx as f64, value))
            .collect::<Vec<_>>(),
    );
    if points.is_empty() {
        return Err(format!(
            "Series reference {} resolved to no finite points",
            load_ref.render()
        ));
    }
    Ok(points)
}

fn preview_selection_to_hyperslab(
    shape: &[usize],
    selection: &PreviewSelection,
) -> Result<Selection, String> {
    preview_selection_len(selection, shape)?;
    let slice = match selection.slice {
        SliceSelection::All => 0..shape[selection.x],
        SliceSelection::FromTo(a, b) => a..b,
    };

    let mut slice_selections = Vec::new();
    for idx in 0..shape.len() {
        if idx == selection.x {
            slice_selections.push(SliceOrIndex::SliceTo {
                start: slice.start,
                step: 1,
                end: slice.end,
                block: 1,
            });
        } else {
            slice_selections.push(SliceOrIndex::Index(selection.index[idx]));
        }
    }

    Ok(Selection::Hyperslab(Hyperslab::from(slice_selections)))
}

pub(super) fn resolve_expression_scalar_value(
    state: &MultiChartState,
    file: &File,
    scalar_ref: &ExpressionScalarRef,
) -> Result<f64, String> {
    match resolve_expression_load_value(
        state,
        file,
        scalar_ref,
        ExpressionSeriesResolution::Overview,
        true,
    )? {
        ResolvedExpressionLoad::Scalar(value) => Ok(value),
        ResolvedExpressionLoad::Series(_) => Err(format!(
            "Reference {} resolved to a series, but a scalar is required",
            scalar_ref.render()
        )),
    }
}

pub(super) fn normalize_absolute_object_path(path: &str) -> Result<String, String> {
    if !path.starts_with('/') {
        return Err(format!("Absolute path '{path}' must start with '/'"));
    }
    let mut components = Vec::new();
    for segment in path.split('/') {
        if segment.is_empty() {
            continue;
        }
        match segment {
            "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(format!("Absolute path '{path}' escapes above root"));
                }
            }
            other => components.push(other),
        }
    }
    if components.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", components.join("/")))
    }
}

fn resolve_expression_target_path(
    state: &MultiChartState,
    target: &ExpressionObjectTarget,
    reference: &str,
) -> Result<String, String> {
    match target {
        ExpressionObjectTarget::AbsolutePath(path) => normalize_absolute_object_path(path),
        ExpressionObjectTarget::ItemRef(id) => state
            .item_by_id(*id)
            .and_then(|item| item.source.dataset_source())
            .map(|dataset_source| dataset_source.dataset_path.clone())
            .ok_or_else(|| {
                format!(
                    "Reference {} requires chart item ${} to be dataset-backed",
                    reference, id.0
                )
            }),
    }
}

fn open_expression_attribute(
    file: &File,
    object_path: &str,
    attr_name: &str,
) -> Result<Attribute, String> {
    if object_path == "/" {
        return file.attr(attr_name).map_err(|error| {
            format!(
                "Failed to read attribute 'load(/:{})': {}",
                attr_name, error
            )
        });
    }

    if let Ok(group) = file.group(object_path) {
        return group.attr(attr_name).map_err(|error| {
            format!(
                "Failed to read attribute '#{}:{}': {}",
                object_path, attr_name, error
            )
        });
    }

    if let Ok(dataset) = file.dataset(object_path) {
        return dataset.attr(attr_name).map_err(|error| {
            format!(
                "Failed to read attribute '#{}:{}': {}",
                object_path, attr_name, error
            )
        });
    }

    Err(format!(
        "Attribute path '{}' does not resolve to a dataset or group in the file",
        object_path
    ))
}

fn read_expression_numeric_scalar_attr(attr: &Attribute, reference: &str) -> Result<f64, String> {
    let dtype = attr.dtype().map_err(|error| {
        format!("Failed to inspect scalar attribute type for {reference}: {error}")
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!("Failed to inspect scalar attribute type for {reference}: {error}")
    })?;
    validate_scalar_type_descriptor(attr.is_scalar(), &type_desc, reference, "Attribute")?;
    match type_desc {
        TypeDescriptor::Integer(IntSize::U1) => attr
            .read_scalar::<i8>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Integer(IntSize::U2) => attr
            .read_scalar::<i16>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Integer(IntSize::U4) => attr
            .read_scalar::<i32>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Integer(IntSize::U8) => attr
            .read_scalar::<i64>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U1) => attr
            .read_scalar::<u8>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U2) => attr
            .read_scalar::<u16>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U4) => attr
            .read_scalar::<u32>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U8) => attr
            .read_scalar::<u64>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Float(FloatSize::U4) => attr
            .read_scalar::<f32>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Float(FloatSize::U8) => attr
            .read_scalar::<f64>()
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        other => Err(format!(
            "Attribute reference {reference} must be numeric; got {other}"
        )),
    }
}

fn validate_expression_numeric_scalar_attr(
    attr: &Attribute,
    reference: &str,
) -> Result<(), String> {
    let dtype = attr.dtype().map_err(|error| {
        format!("Failed to inspect scalar attribute type for {reference}: {error}")
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!("Failed to inspect scalar attribute type for {reference}: {error}")
    })?;
    validate_scalar_type_descriptor(attr.is_scalar(), &type_desc, reference, "Attribute")
}

fn read_expression_numeric_series_attr(
    attr: &Attribute,
    reference: &str,
) -> Result<Vec<Point>, String> {
    let dtype = attr.dtype().map_err(|error| {
        format!("Failed to inspect series attribute type for {reference}: {error}")
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!("Failed to inspect series attribute type for {reference}: {error}")
    })?;
    validate_series_type_descriptor(
        attr.is_scalar(),
        attr.shape().len(),
        &type_desc,
        reference,
        "Attribute",
    )?;
    let values = match type_desc {
        TypeDescriptor::Integer(IntSize::U1) => attr.read_1d::<i8>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Integer(IntSize::U2) => attr.read_1d::<i16>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Integer(IntSize::U4) => attr.read_1d::<i32>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Integer(IntSize::U8) => attr.read_1d::<i64>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Unsigned(IntSize::U1) => attr.read_1d::<u8>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Unsigned(IntSize::U2) => attr.read_1d::<u16>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Unsigned(IntSize::U4) => attr.read_1d::<u32>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Unsigned(IntSize::U8) => attr.read_1d::<u64>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Float(FloatSize::U4) => attr.read_1d::<f32>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Float(FloatSize::U8) => attr
            .read_1d::<f64>()
            .map(|values| values.into_iter().collect::<Vec<_>>()),
        other => {
            return Err(format!(
                "Series reference {reference} must be numeric; got {other}"
            ))
        }
    }
    .map_err(|error| format!("Failed reading {reference}: {error}"))?;

    let points = sanitize_chart_points(
        values
            .into_iter()
            .enumerate()
            .map(|(idx, value)| (idx as f64, value))
            .collect(),
    );
    if points.is_empty() {
        return Err(format!(
            "Series reference {reference} resolved to no finite points"
        ));
    }
    Ok(points)
}

fn validate_expression_numeric_series_attr(
    attr: &Attribute,
    reference: &str,
) -> Result<usize, String> {
    let dtype = attr.dtype().map_err(|error| {
        format!("Failed to inspect series attribute type for {reference}: {error}")
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!("Failed to inspect series attribute type for {reference}: {error}")
    })?;
    validate_series_type_descriptor(
        attr.is_scalar(),
        attr.shape().len(),
        &type_desc,
        reference,
        "Attribute",
    )?;
    Ok(attr.shape()[0])
}

fn read_expression_numeric_scalar_dataset(
    dataset: &Dataset,
    reference: &str,
) -> Result<f64, String> {
    let dtype = dataset.dtype().map_err(|error| {
        format!("Failed to inspect scalar dataset type for {reference}: {error}")
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!("Failed to inspect scalar dataset type for {reference}: {error}")
    })?;
    validate_scalar_type_descriptor(dataset.is_scalar(), &type_desc, reference, "Dataset")?;
    match type_desc {
        TypeDescriptor::Integer(IntSize::U1) => dataset
            .read_scalar::<i8>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Integer(IntSize::U2) => dataset
            .read_scalar::<i16>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Integer(IntSize::U4) => dataset
            .read_scalar::<i32>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Integer(IntSize::U8) => dataset
            .read_scalar::<i64>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U1) => dataset
            .read_scalar::<u8>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U2) => dataset
            .read_scalar::<u16>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U4) => dataset
            .read_scalar::<u32>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U8) => dataset
            .read_scalar::<u64>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Float(FloatSize::U4) => dataset
            .read_scalar::<f32>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Float(FloatSize::U8) => dataset
            .read_scalar::<f64>()
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        other => Err(format!(
            "Scalar reference {reference} must be numeric; got {other}"
        )),
    }
}

fn read_expression_numeric_scalar_dataset_value(
    dataset: &Dataset,
    reference: &str,
    indexes: Option<&[usize]>,
) -> Result<f64, String> {
    match indexes {
        None | Some([]) => read_expression_numeric_scalar_dataset(dataset, reference),
        Some(indexes) => {
            let dtype = dataset.dtype().map_err(|error| {
                format!("Failed to inspect scalar dataset type for {reference}: {error}")
            })?;
            let type_desc = dtype.to_descriptor().map_err(|error| {
                format!("Failed to inspect scalar dataset type for {reference}: {error}")
            })?;
            match type_desc {
                TypeDescriptor::Integer(IntSize::U1) => read_indexed_numeric_value(
                    dataset
                        .read_dyn::<i8>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Integer(IntSize::U2) => read_indexed_numeric_value(
                    dataset
                        .read_dyn::<i16>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Integer(IntSize::U4) => read_indexed_numeric_value(
                    dataset
                        .read_dyn::<i32>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Integer(IntSize::U8) => read_indexed_numeric_value(
                    dataset
                        .read_dyn::<i64>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Unsigned(IntSize::U1) => read_indexed_numeric_value(
                    dataset
                        .read_dyn::<u8>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Unsigned(IntSize::U2) => read_indexed_numeric_value(
                    dataset
                        .read_dyn::<u16>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Unsigned(IntSize::U4) => read_indexed_numeric_value(
                    dataset
                        .read_dyn::<u32>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Unsigned(IntSize::U8) => read_indexed_numeric_value(
                    dataset
                        .read_dyn::<u64>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Float(FloatSize::U4) => read_indexed_numeric_value(
                    dataset
                        .read_dyn::<f32>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Float(FloatSize::U8) => {
                    read_indexed_numeric_value(dataset.read_dyn::<f64>(), indexes, reference)
                }
                TypeDescriptor::Boolean => read_indexed_numeric_value(
                    dataset
                        .read_dyn::<bool>()
                        .map(|values| values.mapv(|value| if value { 1.0 } else { 0.0 })),
                    indexes,
                    reference,
                ),
                other => Err(format!(
                    "Scalar reference {reference} must be numeric; got {other}"
                )),
            }
        }
    }
}

fn read_expression_numeric_scalar_attr_value(
    attr: &Attribute,
    reference: &str,
    indexes: Option<&[usize]>,
) -> Result<f64, String> {
    match indexes {
        None | Some([]) => read_expression_numeric_scalar_attr(attr, reference),
        Some(indexes) => {
            let dtype = attr.dtype().map_err(|error| {
                format!("Failed to inspect scalar attribute type for {reference}: {error}")
            })?;
            let type_desc = dtype.to_descriptor().map_err(|error| {
                format!("Failed to inspect scalar attribute type for {reference}: {error}")
            })?;
            match type_desc {
                TypeDescriptor::Integer(IntSize::U1) => read_indexed_numeric_value(
                    attr.read_dyn::<i8>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Integer(IntSize::U2) => read_indexed_numeric_value(
                    attr.read_dyn::<i16>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Integer(IntSize::U4) => read_indexed_numeric_value(
                    attr.read_dyn::<i32>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Integer(IntSize::U8) => read_indexed_numeric_value(
                    attr.read_dyn::<i64>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Unsigned(IntSize::U1) => read_indexed_numeric_value(
                    attr.read_dyn::<u8>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Unsigned(IntSize::U2) => read_indexed_numeric_value(
                    attr.read_dyn::<u16>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Unsigned(IntSize::U4) => read_indexed_numeric_value(
                    attr.read_dyn::<u32>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Unsigned(IntSize::U8) => read_indexed_numeric_value(
                    attr.read_dyn::<u64>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Float(FloatSize::U4) => read_indexed_numeric_value(
                    attr.read_dyn::<f32>()
                        .map(|values| values.mapv(|value| value as f64)),
                    indexes,
                    reference,
                ),
                TypeDescriptor::Float(FloatSize::U8) => {
                    read_indexed_numeric_value(attr.read_dyn::<f64>(), indexes, reference)
                }
                TypeDescriptor::Boolean => read_indexed_numeric_value(
                    attr.read_dyn::<bool>()
                        .map(|values| values.mapv(|value| if value { 1.0 } else { 0.0 })),
                    indexes,
                    reference,
                ),
                other => Err(format!(
                    "Scalar reference {reference} must be numeric; got {other}"
                )),
            }
        }
    }
}

fn read_indexed_numeric_value(
    result: Result<ndarray::ArrayD<f64>, hdf5_metno::Error>,
    indexes: &[usize],
    reference: &str,
) -> Result<f64, String> {
    let values = result.map_err(|error| format!("Failed reading {reference}: {error}"))?;
    values
        .get(IxDyn(indexes))
        .copied()
        .ok_or_else(|| format!("Reference {reference} index {:?} is out of bounds", indexes))
}

fn validate_expression_numeric_scalar_dataset(
    dataset: &Dataset,
    reference: &str,
) -> Result<(), String> {
    let dtype = dataset.dtype().map_err(|error| {
        format!("Failed to inspect scalar dataset type for {reference}: {error}")
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!("Failed to inspect scalar dataset type for {reference}: {error}")
    })?;
    validate_scalar_type_descriptor(dataset.is_scalar(), &type_desc, reference, "Dataset")
}

fn validate_expression_numeric_series_dataset(
    dataset: &Dataset,
    series_ref: &ExpressionSeriesRef,
) -> Result<usize, String> {
    let shape = dataset.shape();
    let preview_selection = series_ref.to_series_preview_selection(&shape)?;
    let dtype = dataset.dtype().map_err(|error| {
        format!(
            "Failed to inspect dataset type for {}: {}",
            series_ref.render(),
            error
        )
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!(
            "Failed to inspect dataset type for {}: {}",
            series_ref.render(),
            error
        )
    })?;
    validate_series_type_descriptor(
        dataset.is_scalar(),
        1,
        &type_desc,
        &series_ref.render(),
        "Dataset",
    )?;
    preview_selection_len(&preview_selection, &shape)
}

fn validate_scalar_type_descriptor(
    is_scalar: bool,
    type_desc: &TypeDescriptor,
    reference: &str,
    object_kind: &str,
) -> Result<(), String> {
    if !is_scalar {
        return Err(format!(
            "{object_kind} reference {reference} must resolve to a scalar numeric {}",
            object_kind.to_ascii_lowercase()
        ));
    }
    if is_numeric_type_descriptor(type_desc) {
        Ok(())
    } else {
        Err(format!(
            "{object_kind} reference {reference} must be numeric; got {type_desc}"
        ))
    }
}

fn validate_series_type_descriptor(
    is_scalar: bool,
    rank: usize,
    type_desc: &TypeDescriptor,
    reference: &str,
    object_kind: &str,
) -> Result<(), String> {
    if is_scalar {
        return Err(format!(
            "Series reference {reference} must resolve to a non-scalar numeric {}",
            object_kind.to_ascii_lowercase()
        ));
    }
    if rank != 1 {
        return Err(format!(
            "Series reference {reference} currently supports only rank-1 numeric {}s",
            object_kind.to_ascii_lowercase()
        ));
    }
    if is_numeric_type_descriptor(type_desc) {
        Ok(())
    } else {
        Err(format!(
            "Series reference {reference} must be numeric; got {type_desc}"
        ))
    }
}

fn is_numeric_type_descriptor(type_desc: &TypeDescriptor) -> bool {
    matches!(
        type_desc,
        TypeDescriptor::Integer(_)
            | TypeDescriptor::Unsigned(_)
            | TypeDescriptor::Float(_)
            | TypeDescriptor::Boolean
    )
}
