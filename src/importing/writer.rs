use std::{collections::HashSet, path::Path, str::FromStr};

use hdf5_metno::{types::VarLenUnicode, Dataset, File, Group};

use crate::error::AppError;

use super::{
    ColumnKind, ImportedTable, TabularImport, COLUMN_COUNT_ATTR, COLUMN_ORDER_ATTR, DELIMITER_ATTR,
    IMPORT_SCHEMA_ATTR, IMPORT_SCHEMA_VERSION, INFERRED_TYPE_ATTR, ORIGINAL_NAME_ATTR,
    ROW_COUNT_ATTR, SOURCE_FORMAT_ATTR, SOURCE_PATH_ATTR, TABLE_COUNT_ATTR, TABLE_ORDER_ATTR,
};

pub(super) fn write_tabular_hdf5(
    artifact_path: &Path,
    source_path: &Path,
    imported: &TabularImport,
) -> Result<(), AppError> {
    let file = File::create(artifact_path)?;
    write_string_attr_file(&file, SOURCE_FORMAT_ATTR, imported.format_label)?;
    write_string_attr_file(&file, SOURCE_PATH_ATTR, &source_path.to_string_lossy())?;
    write_string_attr_file(&file, IMPORT_SCHEMA_ATTR, IMPORT_SCHEMA_VERSION)?;
    if let Some(delimiter) = imported.delimiter {
        write_string_attr_file(
            &file,
            DELIMITER_ATTR,
            if delimiter == b'\t' { "\\t" } else { "," },
        )?;
    }
    write_u64_attr_file(&file, TABLE_COUNT_ATTR, imported.tables.len() as u64)?;

    if imported.tables.len() == 1 && !imported.force_table_groups {
        write_root_table_hdf5(&file, &imported.tables[0])?;
    } else {
        let table_order = imported
            .tables
            .iter()
            .map(|table| table.name.clone())
            .collect::<Vec<_>>();
        write_string_array_attr_file(&file, TABLE_ORDER_ATTR, &table_order)?;
        let mut used_names = HashSet::new();
        for table in &imported.tables {
            let base_name = sanitize_dataset_name(&table.name, used_names.len());
            let group_name = unique_name(&mut used_names, &base_name);
            let group = file.create_group(&group_name)?;
            write_string_attr_group(&group, ORIGINAL_NAME_ATTR, &table.name)?;
            write_group_table_hdf5(&group, table)?;
        }
    }

    Ok(())
}

fn write_root_table_hdf5(file: &File, table: &ImportedTable) -> Result<(), AppError> {
    write_u64_attr_file(
        file,
        ROW_COUNT_ATTR,
        table.columns.first().map_or(0, Vec::len) as u64,
    )?;
    write_u64_attr_file(file, COLUMN_COUNT_ATTR, table.columns.len() as u64)?;
    let columns_group = file.create_group("columns")?;
    write_columns_into_group(&columns_group, table)
}

fn write_group_table_hdf5(group: &Group, table: &ImportedTable) -> Result<(), AppError> {
    write_u64_attr_group(
        group,
        ROW_COUNT_ATTR,
        table.columns.first().map_or(0, Vec::len) as u64,
    )?;
    write_u64_attr_group(group, COLUMN_COUNT_ATTR, table.columns.len() as u64)?;
    let columns_group = group.create_group("columns")?;
    write_columns_into_group(&columns_group, table)
}

fn write_columns_into_group(columns_group: &Group, table: &ImportedTable) -> Result<(), AppError> {
    write_string_array_attr_group(columns_group, COLUMN_ORDER_ATTR, &table.headers)?;
    let dataset_names = unique_dataset_names(&table.headers);
    for ((dataset_name, original_name), values) in dataset_names
        .iter()
        .zip(table.headers.iter())
        .zip(table.columns.iter())
    {
        let kind = infer_column_kind(values);
        let dataset = write_column_dataset(columns_group, dataset_name, values, kind)?;
        write_string_attr_dataset(&dataset, ORIGINAL_NAME_ATTR, original_name)?;
        write_string_attr_dataset(&dataset, INFERRED_TYPE_ATTR, kind.label())?;
    }
    Ok(())
}

fn write_column_dataset(
    columns_group: &Group,
    dataset_name: &str,
    values: &[String],
    kind: ColumnKind,
) -> Result<Dataset, AppError> {
    match kind {
        ColumnKind::Bool => {
            let data = values
                .iter()
                .map(|value| match parse_bool_word(value) {
                    Some(true) => 1_u8,
                    Some(false) => 0_u8,
                    None => 0_u8,
                })
                .collect::<Vec<_>>();
            columns_group
                .new_dataset_builder()
                .with_data(&data)
                .create(dataset_name)
                .map_err(AppError::from)
        }
        ColumnKind::I64 => {
            let data = values
                .iter()
                .map(|value| value.parse::<i64>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| {
                    AppError::FileError(format!(
                        "Failed converting column '{dataset_name}' to i64: {error}"
                    ))
                })?;
            columns_group
                .new_dataset_builder()
                .with_data(&data)
                .create(dataset_name)
                .map_err(AppError::from)
        }
        ColumnKind::U64 => {
            let data = values
                .iter()
                .map(|value| value.parse::<u64>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| {
                    AppError::FileError(format!(
                        "Failed converting column '{dataset_name}' to u64: {error}"
                    ))
                })?;
            columns_group
                .new_dataset_builder()
                .with_data(&data)
                .create(dataset_name)
                .map_err(AppError::from)
        }
        ColumnKind::F64 => {
            let data = values
                .iter()
                .map(|value| {
                    if value.trim().is_empty() {
                        Ok(f64::NAN)
                    } else {
                        value.parse::<f64>()
                    }
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| {
                    AppError::FileError(format!(
                        "Failed converting column '{dataset_name}' to f64: {error}"
                    ))
                })?;
            columns_group
                .new_dataset_builder()
                .with_data(&data)
                .create(dataset_name)
                .map_err(AppError::from)
        }
        ColumnKind::String => {
            let data = values
                .iter()
                .map(|value| VarLenUnicode::from_str(value))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| {
                    AppError::FileError(format!(
                        "Failed converting column '{dataset_name}' to unicode strings: {error}"
                    ))
                })?;
            columns_group
                .new_dataset_builder()
                .with_data(&data)
                .create(dataset_name)
                .map_err(AppError::from)
        }
    }
}

fn infer_column_kind(values: &[String]) -> ColumnKind {
    let non_empty = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if non_empty.is_empty() {
        return ColumnKind::String;
    }

    if values
        .iter()
        .all(|value| value.trim().is_empty() || parse_bool_word(value).is_some())
        && values.iter().all(|value| !value.trim().is_empty())
    {
        return ColumnKind::Bool;
    }

    if values.iter().all(|value| !value.trim().is_empty())
        && values.iter().all(|value| value.parse::<i64>().is_ok())
    {
        return ColumnKind::I64;
    }

    if values.iter().all(|value| !value.trim().is_empty())
        && values.iter().all(|value| value.parse::<u64>().is_ok())
    {
        return ColumnKind::U64;
    }

    if values
        .iter()
        .all(|value| value.trim().is_empty() || value.parse::<f64>().is_ok())
    {
        return ColumnKind::F64;
    }

    ColumnKind::String
}

fn parse_bool_word(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" => Some(true),
        "false" | "no" => Some(false),
        _ => None,
    }
}

fn unique_dataset_names(headers: &[String]) -> Vec<String> {
    let mut used = HashSet::new();
    let mut names = Vec::with_capacity(headers.len());
    for (idx, header) in headers.iter().enumerate() {
        names.push(unique_name(&mut used, &sanitize_dataset_name(header, idx)));
    }
    names
}

fn unique_name(used: &mut HashSet<String>, base: &str) -> String {
    if used.insert(base.to_string()) {
        return base.to_string();
    }
    let mut suffix = 2_usize;
    loop {
        let candidate = format!("{base}_{suffix}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
}

fn sanitize_dataset_name(header: &str, idx: usize) -> String {
    let mut name = String::new();
    let mut last_was_separator = false;
    for ch in header.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };
        if mapped == '_' {
            if last_was_separator {
                continue;
            }
            last_was_separator = true;
        } else {
            last_was_separator = false;
        }
        name.push(mapped);
    }
    let name = name.trim_matches('_').to_string();
    if name.is_empty() {
        format!("column_{}", idx + 1)
    } else {
        name
    }
}

fn write_string_attr_file(file: &File, name: &str, value: &str) -> Result<(), AppError> {
    let attr = file
        .new_attr_builder()
        .empty::<VarLenUnicode>()
        .create(name)?;
    let value = VarLenUnicode::from_str(value).map_err(|error| {
        AppError::FileError(format!("Failed encoding attribute '{name}': {error}"))
    })?;
    attr.write_scalar(&value)?;
    Ok(())
}

fn write_u64_attr_file(file: &File, name: &str, value: u64) -> Result<(), AppError> {
    let attr = file.new_attr_builder().empty::<u64>().create(name)?;
    attr.write_scalar(&value)?;
    Ok(())
}

fn write_string_array_attr_file(
    file: &File,
    name: &str,
    values: &[String],
) -> Result<(), AppError> {
    let encoded = values
        .iter()
        .map(|value| VarLenUnicode::from_str(value))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            AppError::FileError(format!("Failed encoding attribute array '{name}': {error}"))
        })?;
    file.new_attr_builder().with_data(&encoded).create(name)?;
    Ok(())
}

fn write_string_array_attr_group(
    group: &Group,
    name: &str,
    values: &[String],
) -> Result<(), AppError> {
    let encoded = values
        .iter()
        .map(|value| VarLenUnicode::from_str(value))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            AppError::FileError(format!("Failed encoding attribute array '{name}': {error}"))
        })?;
    group.new_attr_builder().with_data(&encoded).create(name)?;
    Ok(())
}

fn write_u64_attr_group(group: &Group, name: &str, value: u64) -> Result<(), AppError> {
    let attr = group.new_attr_builder().empty::<u64>().create(name)?;
    attr.write_scalar(&value)?;
    Ok(())
}

fn write_string_attr_group(group: &Group, name: &str, value: &str) -> Result<(), AppError> {
    let attr = group
        .new_attr_builder()
        .empty::<VarLenUnicode>()
        .create(name)?;
    let value = VarLenUnicode::from_str(value).map_err(|error| {
        AppError::FileError(format!("Failed encoding attribute '{name}': {error}"))
    })?;
    attr.write_scalar(&value)?;
    Ok(())
}

fn write_string_attr_dataset(dataset: &Dataset, name: &str, value: &str) -> Result<(), AppError> {
    let attr = dataset
        .new_attr_builder()
        .empty::<VarLenUnicode>()
        .create(name)?;
    let value = VarLenUnicode::from_str(value).map_err(|error| {
        AppError::FileError(format!("Failed encoding attribute '{name}': {error}"))
    })?;
    attr.write_scalar(&value)?;
    Ok(())
}
