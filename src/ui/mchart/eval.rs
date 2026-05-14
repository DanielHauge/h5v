use hdf5_metno::{
    types::{FloatSize, IntSize, TypeDescriptor},
    Attribute, Dataset, File, Hyperslab, Selection, SliceOrIndex,
};

use crate::data::{
    validate_preview_selection_shape, DatasetPlotingData, PreviewSelection, SliceSelection,
};

use super::{
    expression::{
        ExprBinaryOp, ExpressionAst, ExpressionItemRef, ExpressionObjectTarget,
        ExpressionScalarRef, ExpressionSeriesRef,
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

pub(super) fn resolve_expression_item_value(
    state: &MultiChartState,
    item_ref: &ExpressionItemRef,
) -> Result<Vec<Point>, String> {
    let item = state
        .item_by_id(item_ref.id)
        .ok_or_else(|| format!("Unknown chart item reference ${}", item_ref.id.0))?;
    let points = sanitize_chart_points(item.series.points.clone());
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
    series_values: &std::collections::HashMap<ExpressionSeriesRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionScalarRef, f64>,
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
        ExpressionAst::SeriesRef(series_ref) => series_values
            .get(series_ref)
            .and_then(|points: &Vec<Point>| points.get(idx).map(|(_, y)| *y))
            .ok_or_else(|| {
                format!(
                    "Series reference {} is unavailable at sample index {}",
                    series_ref.render(),
                    idx
                )
            }),
        ExpressionAst::ScalarRef(scalar_ref) => scalar_values
            .get(scalar_ref)
            .copied()
            .ok_or_else(|| format!("Scalar reference {} is unavailable", scalar_ref.render())),
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
) -> Result<std::collections::HashMap<ExpressionSeriesRef, Vec<Point>>, String> {
    if refs.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let file = file.ok_or_else(|| {
        "Series references require an open file handle, but no file is loaded".to_string()
    })?;
    let mut series = std::collections::HashMap::with_capacity(refs.len());
    for series_ref in refs {
        let points = resolve_expression_series_value(state, file, series_ref)?;
        series.insert(series_ref.clone(), points);
    }
    Ok(series)
}

pub(super) fn resolve_expression_series_value(
    state: &MultiChartState,
    file: &File,
    series_ref: &ExpressionSeriesRef,
) -> Result<Vec<Point>, String> {
    match (&series_ref.target, &series_ref.attr_name) {
        (ExpressionObjectTarget::ItemRef(id), None) => {
            let points = state
                .item_by_id(*id)
                .map(|item| sanitize_chart_points(item.series.points.clone()))
                .ok_or_else(|| format!("Unknown chart item reference ${}", id.0))?;
            if points.is_empty() {
                Err(format!(
                    "Series reference {} resolved to no finite points",
                    series_ref.render()
                ))
            } else {
                Ok(points)
            }
        }
        (target, Some(attr_name)) => {
            let object_path = resolve_expression_target_path(state, target, &series_ref.render())?;
            let attr = open_expression_attribute(file, &object_path, attr_name)?;
            read_expression_numeric_series_attr(&attr, &series_ref.render())
        }
        (ExpressionObjectTarget::AbsolutePath(path), None) => {
            let object_path = normalize_absolute_object_path(&path)?;
            let dataset = file.dataset(&object_path).map_err(|error| {
                format!(
                    "Series reference {} could not open dataset '{}': {}",
                    series_ref.render(),
                    object_path,
                    error
                )
            })?;
            read_expression_dataset_points(&dataset, series_ref)
        }
    }
}

pub(super) fn read_expression_dataset_points(
    dataset: &Dataset,
    series_ref: &ExpressionSeriesRef,
) -> Result<Vec<Point>, String> {
    let shape = dataset.shape();
    let preview_selection = series_ref.to_preview_selection(&shape)?;
    let selection = preview_selection_to_hyperslab(&shape, &preview_selection)?;
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
                series_ref.render(),
                other
            ))
        }
    }
    .map_err(|error| format!("Failed reading {}: {}", series_ref.render(), error))?;

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
            series_ref.render()
        ));
    }
    Ok(points)
}

fn preview_selection_to_hyperslab(
    shape: &[usize],
    selection: &PreviewSelection,
) -> Result<Selection, String> {
    validate_preview_selection_shape(shape, selection).map_err(|error| error.to_string())?;
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
    let object_path =
        resolve_expression_target_path(state, &scalar_ref.target, &scalar_ref.render())?;
    match &scalar_ref.attr_name {
        Some(attr_name) => {
            let attr = open_expression_attribute(file, &object_path, attr_name)?;
            require_finite_scalar_value(
                read_expression_numeric_scalar_attr(&attr, &scalar_ref.render())?,
                &scalar_ref.render(),
            )
        }
        None => {
            let dataset = file.dataset(&object_path).map_err(|error| {
                format!(
                    "Scalar reference {} could not open dataset '{}': {}",
                    scalar_ref.render(),
                    object_path,
                    error
                )
            })?;
            require_finite_scalar_value(
                read_expression_numeric_scalar_dataset(&dataset, &scalar_ref.render())?,
                &scalar_ref.render(),
            )
        }
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
        return file
            .attr(attr_name)
            .map_err(|error| format!("Failed to read attribute '#/:{}': {}", attr_name, error));
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
    if !attr.is_scalar() {
        return Err(format!(
            "Attribute reference {reference} must resolve to a scalar numeric attribute"
        ));
    }
    let dtype = attr.dtype().map_err(|error| {
        format!("Failed to inspect scalar attribute type for {reference}: {error}")
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!("Failed to inspect scalar attribute type for {reference}: {error}")
    })?;
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

fn read_expression_numeric_series_attr(
    attr: &Attribute,
    reference: &str,
) -> Result<Vec<Point>, String> {
    if attr.is_scalar() {
        return Err(format!(
            "Series reference {reference} must resolve to a non-scalar numeric attribute"
        ));
    }
    if attr.shape().len() != 1 {
        return Err(format!(
            "Series reference {reference} currently supports only rank-1 numeric attributes"
        ));
    }

    let dtype = attr.dtype().map_err(|error| {
        format!("Failed to inspect series attribute type for {reference}: {error}")
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!("Failed to inspect series attribute type for {reference}: {error}")
    })?;
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

fn read_expression_numeric_scalar_dataset(
    dataset: &Dataset,
    reference: &str,
) -> Result<f64, String> {
    if !dataset.is_scalar() {
        return Err(format!(
            "Scalar reference {reference} must resolve to a scalar numeric dataset"
        ));
    }

    let dtype = dataset.dtype().map_err(|error| {
        format!("Failed to inspect scalar dataset type for {reference}: {error}")
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!("Failed to inspect scalar dataset type for {reference}: {error}")
    })?;
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
