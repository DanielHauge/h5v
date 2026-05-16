use super::*;

pub(crate) fn validate_expression_load_ref(
    file: &File,
    load_ref: &ExpressionLoadRef,
    _resolution: ExpressionSeriesResolution,
    allow_external_series: bool,
) -> Result<ValidatedExpressionLoad, String> {
    match (&load_ref.target, &load_ref.attr_name) {
        (target, Some(attr_name)) => {
            let object_path = resolve_expression_target_path(target)?;
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

pub(crate) fn resolve_expression_load_value(
    file: &File,
    load_ref: &ExpressionLoadRef,
    _resolution: ExpressionSeriesResolution,
    allow_external_series: bool,
) -> Result<ResolvedExpressionLoad, String> {
    match (&load_ref.target, &load_ref.attr_name) {
        (target, Some(attr_name)) => {
            let object_path = resolve_expression_target_path(target)?;
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

pub(crate) fn read_expression_dataset_points(
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

pub(crate) fn normalize_absolute_object_path(path: &str) -> Result<String, String> {
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

fn resolve_expression_target_path(target: &ExpressionObjectTarget) -> Result<String, String> {
    match target {
        ExpressionObjectTarget::AbsolutePath(path) => normalize_absolute_object_path(path),
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
