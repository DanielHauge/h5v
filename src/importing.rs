use std::{
    collections::hash_map::DefaultHasher,
    collections::HashSet,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    str::FromStr,
    time::UNIX_EPOCH,
};

use calamine::{open_workbook_auto, Data, Reader};
use csv::{ReaderBuilder, StringRecord};
use hdf5_metno::{types::VarLenUnicode, Dataset, File, Group};
use parquet::{
    file::reader::{FileReader, SerializedFileReader},
    record::Row,
};

use crate::error::AppError;

const IMPORT_SCHEMA_VERSION: &str = "tabular-v1";
const SOURCE_FORMAT_ATTR: &str = "H5V_SOURCE_FORMAT";
const SOURCE_PATH_ATTR: &str = "H5V_SOURCE_PATH";
const IMPORT_SCHEMA_ATTR: &str = "H5V_IMPORT_SCHEMA";
const ROW_COUNT_ATTR: &str = "H5V_ROW_COUNT";
const COLUMN_COUNT_ATTR: &str = "H5V_COLUMN_COUNT";
const COLUMN_ORDER_ATTR: &str = "H5V_COLUMN_ORDER";
const ORIGINAL_NAME_ATTR: &str = "H5V_ORIGINAL_NAME";
const INFERRED_TYPE_ATTR: &str = "H5V_INFERRED_TYPE";
const DELIMITER_ATTR: &str = "H5V_DELIMITER";
const TABLE_ORDER_ATTR: &str = "H5V_TABLE_ORDER";
const TABLE_COUNT_ATTR: &str = "H5V_TABLE_COUNT";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedHdf5Input {
    pub(crate) original_path: PathBuf,
    pub(crate) hdf5_path: String,
    pub(crate) link_name: String,
    pub(crate) imported: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceFormat {
    Csv { delimiter: u8, label: &'static str },
    Xlsx,
    Parquet,
}

impl SourceFormat {
    fn label(self) -> &'static str {
        match self {
            Self::Csv { label, .. } => label,
            Self::Xlsx => "xlsx",
            Self::Parquet => "parquet",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColumnKind {
    Bool,
    I64,
    U64,
    F64,
    String,
}

impl ColumnKind {
    fn label(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::F64 => "f64",
            Self::String => "string",
        }
    }
}

struct ImportedTable {
    name: String,
    headers: Vec<String>,
    columns: Vec<Vec<String>>,
}

struct TabularImport {
    tables: Vec<ImportedTable>,
    delimiter: Option<u8>,
    format_label: &'static str,
    force_table_groups: bool,
}

pub(crate) fn resolve_cli_inputs(paths: &[String]) -> Result<Vec<ResolvedHdf5Input>, AppError> {
    let mut resolved = Vec::with_capacity(paths.len());
    let mut errors = Vec::new();

    for raw_path in paths {
        let path = PathBuf::from(raw_path);
        match resolve_cli_input(&path) {
            Ok(input) => resolved.push(input),
            Err(error) => errors.push(format!("- {}: {}", path.display(), error)),
        }
    }

    if !errors.is_empty() {
        return Err(AppError::FileError(format!(
            "Failed to open input files:\n{}",
            errors.join("\n")
        )));
    }

    Ok(resolved)
}

fn resolve_cli_input(path: &Path) -> Result<ResolvedHdf5Input, AppError> {
    if !path.exists() {
        return Err(AppError::FileError(format!(
            "Path '{}' does not exist",
            path.display()
        )));
    }
    if !path.is_file() {
        return Err(AppError::FileError(format!(
            "Path '{}' is not a file",
            path.display()
        )));
    }

    if File::open(path).is_ok() {
        return Ok(ResolvedHdf5Input {
            original_path: path.to_path_buf(),
            hdf5_path: path.to_string_lossy().into_owned(),
            link_name: display_name_for_path(path),
            imported: false,
        });
    }

    match detect_non_hdf5_format(path) {
        Some(format) => {
            let imported_path = import_tabular_file(path, format)?;
            Ok(ResolvedHdf5Input {
                original_path: path.to_path_buf(),
                hdf5_path: imported_path,
                link_name: display_name_for_path(path),
                imported: true,
            })
        }
        None => Err(AppError::FileError(format!(
            "Unsupported file format for '{}'. Supported non-HDF5 imports currently: .csv, .tsv, .tab, .xlsx, .parquet",
            path.display()
        ))),
    }
}

fn detect_non_hdf5_format(path: &Path) -> Option<SourceFormat> {
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match extension.as_str() {
        "csv" => Some(SourceFormat::Csv {
            delimiter: b',',
            label: "csv",
        }),
        "tsv" | "tab" => Some(SourceFormat::Csv {
            delimiter: b'\t',
            label: "tsv",
        }),
        "xlsx" => Some(SourceFormat::Xlsx),
        "parquet" => Some(SourceFormat::Parquet),
        _ => None,
    }
}

fn import_tabular_file(source_path: &Path, format: SourceFormat) -> Result<String, AppError> {
    let cache_root = import_cache_root()?;
    let metadata = fs::metadata(source_path)?;
    let cache_key = import_cache_key(source_path, &metadata, format.label());
    let stem = sanitized_file_stem(source_path);
    let artifact_path = cache_root.join(format!("{stem}-{cache_key}.h5"));
    if artifact_path.exists() && File::open(&artifact_path).is_ok() {
        return Ok(artifact_path.to_string_lossy().into_owned());
    }

    let imported = match format {
        SourceFormat::Csv { delimiter, label } => {
            read_delimited_file(source_path, delimiter, label)?
        }
        SourceFormat::Xlsx => read_xlsx_file(source_path)?,
        SourceFormat::Parquet => read_parquet_file(source_path)?,
    };
    let temp_path = cache_root.join(format!("{stem}-{cache_key}.tmp.h5"));
    if temp_path.exists() {
        fs::remove_file(&temp_path)?;
    }
    if artifact_path.exists() {
        fs::remove_file(&artifact_path)?;
    }

    write_tabular_hdf5(&temp_path, source_path, &imported)?;
    fs::rename(&temp_path, &artifact_path)?;
    Ok(artifact_path.to_string_lossy().into_owned())
}

fn import_cache_root() -> Result<PathBuf, AppError> {
    let root = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("h5v")
        .join("imports");
    fs::create_dir_all(&root)?;
    Ok(root)
}

fn import_cache_key(source_path: &Path, metadata: &fs::Metadata, format_label: &str) -> String {
    let mut hasher = DefaultHasher::new();
    source_path.to_string_lossy().hash(&mut hasher);
    metadata.len().hash(&mut hasher);
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .hash(&mut hasher);
    IMPORT_SCHEMA_VERSION.hash(&mut hasher);
    format_label.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn read_delimited_file(
    source_path: &Path,
    delimiter: u8,
    format_label: &'static str,
) -> Result<TabularImport, AppError> {
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .from_path(source_path)
        .map_err(|error| {
            AppError::FileError(format!(
                "Failed reading {} file '{}': {}",
                format_label,
                source_path.display(),
                error
            ))
        })?;

    let headers_record = reader.headers().map_err(|error| {
        AppError::FileError(format!(
            "Failed reading header row from '{}': {}",
            source_path.display(),
            error
        ))
    })?;
    if headers_record.is_empty() {
        return Err(AppError::FileError(format!(
            "File '{}' does not contain any columns",
            source_path.display()
        )));
    }

    let mut headers = headers_record
        .iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let mut columns = vec![Vec::new(); headers.len()];
    let mut row_count = 0_usize;

    for record in reader.records() {
        let record = record.map_err(|error| {
            AppError::FileError(format!(
                "Failed parsing '{}' as {}: {}",
                source_path.display(),
                format_label,
                error
            ))
        })?;
        ensure_column_width(&record, &mut headers, &mut columns, row_count);
        for (idx, column) in columns.iter_mut().enumerate() {
            column.push(record.get(idx).unwrap_or_default().to_string());
        }
        row_count += 1;
    }

    Ok(TabularImport {
        tables: vec![ImportedTable {
            name: "table".to_string(),
            headers,
            columns,
        }],
        delimiter: Some(delimiter),
        format_label,
        force_table_groups: false,
    })
}

fn read_xlsx_file(source_path: &Path) -> Result<TabularImport, AppError> {
    let mut workbook = open_workbook_auto(source_path).map_err(|error| {
        AppError::FileError(format!(
            "Failed opening XLSX file '{}': {}",
            source_path.display(),
            error
        ))
    })?;

    let mut tables = Vec::new();
    for sheet_name in workbook.sheet_names().to_owned() {
        let range = workbook.worksheet_range(&sheet_name).map_err(|error| {
            AppError::FileError(format!(
                "Failed reading sheet '{}' from '{}': {}",
                sheet_name,
                source_path.display(),
                error
            ))
        })?;

        let mut rows = range.rows();
        let Some(header_row) = rows.next() else {
            continue;
        };
        if header_row.is_empty() {
            continue;
        }

        let mut headers = header_row
            .iter()
            .map(excel_cell_to_string)
            .collect::<Vec<_>>();
        let mut columns = vec![Vec::new(); headers.len()];
        let mut row_count = 0_usize;

        for row in rows {
            ensure_sequence_width(row.len(), &mut headers, &mut columns, row_count);
            for (idx, column) in columns.iter_mut().enumerate() {
                column.push(row.get(idx).map(excel_cell_to_string).unwrap_or_default());
            }
            row_count += 1;
        }

        tables.push(ImportedTable {
            name: sheet_name,
            headers,
            columns,
        });
    }

    if tables.is_empty() {
        return Err(AppError::FileError(format!(
            "XLSX file '{}' does not contain any non-empty sheets",
            source_path.display()
        )));
    }

    Ok(TabularImport {
        tables,
        delimiter: None,
        format_label: "xlsx",
        force_table_groups: true,
    })
}

fn read_parquet_file(source_path: &Path) -> Result<TabularImport, AppError> {
    let reader = SerializedFileReader::new(fs::File::open(source_path)?).map_err(|error| {
        AppError::FileError(format!(
            "Failed opening Parquet file '{}': {}",
            source_path.display(),
            error
        ))
    })?;

    let mut headers = reader
        .metadata()
        .file_metadata()
        .schema_descr()
        .columns()
        .iter()
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    let mut columns = vec![Vec::new(); headers.len()];
    let mut row_count = 0_usize;

    let row_iter = reader.get_row_iter(None).map_err(|error| {
        AppError::FileError(format!(
            "Failed reading rows from Parquet file '{}': {}",
            source_path.display(),
            error
        ))
    })?;
    for row in row_iter {
        let row = row.map_err(|error| {
            AppError::FileError(format!(
                "Failed decoding a row from Parquet file '{}': {}",
                source_path.display(),
                error
            ))
        })?;
        append_parquet_row(&row, &mut headers, &mut columns, row_count);
        row_count += 1;
    }

    Ok(TabularImport {
        tables: vec![ImportedTable {
            name: "table".to_string(),
            headers,
            columns,
        }],
        delimiter: None,
        format_label: "parquet",
        force_table_groups: false,
    })
}

fn append_parquet_row(
    row: &Row,
    headers: &mut Vec<String>,
    columns: &mut Vec<Vec<String>>,
    row_count: usize,
) {
    let fields = row.get_column_iter().collect::<Vec<_>>();
    if !fields.is_empty() {
        if headers.is_empty() {
            headers.extend(fields.iter().map(|(name, _)| (*name).clone()));
        }
        ensure_sequence_width(fields.len(), headers, columns, row_count);
        for (idx, column) in columns.iter_mut().enumerate() {
            let value = fields
                .get(idx)
                .map(|(_, field)| parquet_field_to_string(field))
                .unwrap_or_default();
            column.push(value);
        }
    }
}

fn ensure_sequence_width(
    width: usize,
    headers: &mut Vec<String>,
    columns: &mut Vec<Vec<String>>,
    row_count: usize,
) {
    if width <= columns.len() {
        return;
    }

    for idx in columns.len()..width {
        headers.push(format!("column_{}", idx + 1));
        columns.push(vec![String::new(); row_count]);
    }
}

fn ensure_column_width(
    record: &StringRecord,
    headers: &mut Vec<String>,
    columns: &mut Vec<Vec<String>>,
    row_count: usize,
) {
    ensure_sequence_width(record.len(), headers, columns, row_count);
}

fn write_tabular_hdf5(
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

fn display_name_for_path(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn sanitized_file_stem(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("imported");
    let mut sanitized = String::new();
    for ch in stem.chars() {
        sanitized.push(if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        });
    }
    sanitized
        .trim_matches('_')
        .to_string()
        .chars()
        .take(40)
        .collect::<String>()
}

fn excel_cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        _ => cell.to_string(),
    }
}

fn parquet_field_to_string(field: &parquet::record::Field) -> String {
    let rendered = field.to_string();
    if rendered.eq_ignore_ascii_case("null") {
        String::new()
    } else if rendered.starts_with('"') && rendered.ends_with('"') && rendered.len() >= 2 {
        rendered[1..rendered.len() - 1].to_string()
    } else {
        rendered
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{io::Write, str::FromStr};

    use hdf5_metno::types::VarLenUnicode;
    use parquet::{
        data_type::ByteArray, file::properties::WriterProperties,
        file::writer::SerializedFileWriter, schema::parser::parse_message_type,
    };
    use rust_xlsxwriter::Workbook;
    use tempfile::tempdir;

    use super::{
        resolve_cli_inputs, COLUMN_ORDER_ATTR, IMPORT_SCHEMA_ATTR, ORIGINAL_NAME_ATTR,
        TABLE_ORDER_ATTR,
    };

    #[test]
    fn resolves_csv_input_into_generated_hdf5_columns() {
        let _guard = crate::test_support::hdf5_test_guard();
        let temp = tempdir().expect("tempdir");
        let csv_path = temp.path().join("sample.csv");
        let mut file = std::fs::File::create(&csv_path).expect("create csv");
        writeln!(file, "time,value,label").expect("header");
        writeln!(file, "0,1.5,alpha").expect("row1");
        writeln!(file, "1,,beta").expect("row2");
        writeln!(file, "2,3.0,gamma").expect("row3");

        let resolved =
            resolve_cli_inputs(&[csv_path.to_string_lossy().into_owned()]).expect("resolve csv");
        assert_eq!(resolved.len(), 1);
        assert!(resolved[0].imported);
        assert_eq!(resolved[0].link_name, "sample.csv");

        let imported = hdf5_metno::File::open(&resolved[0].hdf5_path).expect("open imported hdf5");
        let columns = imported.group("columns").expect("columns group");
        let order = columns
            .attr(COLUMN_ORDER_ATTR)
            .expect("column order attr")
            .read_1d::<VarLenUnicode>()
            .expect("read column order");
        assert_eq!(
            order.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["time", "value", "label"]
        );
        let schema = imported
            .attr(IMPORT_SCHEMA_ATTR)
            .expect("schema attr")
            .read_scalar::<VarLenUnicode>()
            .expect("read schema");
        assert_eq!(schema.to_string(), "tabular-v1");

        let time = columns.dataset("time").expect("time dataset");
        assert_eq!(
            time.attr(ORIGINAL_NAME_ATTR)
                .expect("original name")
                .read_scalar::<VarLenUnicode>()
                .expect("read original name")
                .to_string(),
            "time"
        );
        assert_eq!(
            time.read_1d::<i64>().expect("time data").to_vec(),
            vec![0, 1, 2]
        );

        let value = columns.dataset("value").expect("value dataset");
        let value_data = value.read_1d::<f64>().expect("value data");
        assert!(value_data[1].is_nan());
        assert_eq!(value_data[0], 1.5);
        assert_eq!(value_data[2], 3.0);

        let label = columns.dataset("label").expect("label dataset");
        assert_eq!(
            label
                .read_1d::<VarLenUnicode>()
                .expect("label data")
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["alpha", "beta", "gamma"]
        );
    }

    #[test]
    fn resolves_native_hdf5_without_importing() {
        let _guard = crate::test_support::hdf5_test_guard();
        let temp = tempdir().expect("tempdir");
        let h5_path = temp.path().join("native.h5");
        let file = hdf5_metno::File::create(&h5_path).expect("create hdf5");
        file.new_attr_builder()
            .empty::<VarLenUnicode>()
            .create("TITLE")
            .expect("create title attr")
            .write_scalar(&VarLenUnicode::from_str("native").expect("unicode"))
            .expect("write title attr");

        let resolved =
            resolve_cli_inputs(&[h5_path.to_string_lossy().into_owned()]).expect("resolve hdf5");
        assert_eq!(resolved.len(), 1);
        assert!(!resolved[0].imported);
        assert_eq!(resolved[0].hdf5_path, h5_path.to_string_lossy());
        assert_eq!(resolved[0].link_name, "native.h5");
    }

    #[test]
    fn resolves_xlsx_input_into_sheet_groups() {
        let _guard = crate::test_support::hdf5_test_guard();
        let temp = tempdir().expect("tempdir");
        let xlsx_path = temp.path().join("book.xlsx");
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.write_string(0, 0, "epoch").expect("header");
        sheet.write_string(0, 1, "reading").expect("header");
        sheet.write_number(1, 0, 1.0).expect("cell");
        sheet.write_number(1, 1, 2.5).expect("cell");
        sheet.write_number(2, 0, 2.0).expect("cell");
        sheet.write_number(2, 1, 4.5).expect("cell");
        workbook.save(&xlsx_path).expect("save workbook");

        let resolved =
            resolve_cli_inputs(&[xlsx_path.to_string_lossy().into_owned()]).expect("resolve xlsx");
        assert!(resolved[0].imported);

        let imported = hdf5_metno::File::open(&resolved[0].hdf5_path).expect("open imported hdf5");
        let order = imported
            .attr(TABLE_ORDER_ATTR)
            .expect("table order attr")
            .read_1d::<VarLenUnicode>()
            .expect("read table order");
        assert_eq!(
            order.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["Sheet1"]
        );

        let sheet_group = imported.group("sheet1").expect("sheet group");
        assert_eq!(
            sheet_group
                .attr(ORIGINAL_NAME_ATTR)
                .expect("sheet original name")
                .read_scalar::<VarLenUnicode>()
                .expect("read sheet name")
                .to_string(),
            "Sheet1"
        );
        let columns = sheet_group.group("columns").expect("columns");
        assert_eq!(
            columns
                .dataset("reading")
                .expect("reading dataset")
                .read_1d::<f64>()
                .expect("reading data")
                .to_vec(),
            vec![2.5, 4.5]
        );
    }

    #[test]
    fn resolves_parquet_input_into_generated_hdf5_columns() {
        let _guard = crate::test_support::hdf5_test_guard();
        let temp = tempdir().expect("tempdir");
        let parquet_path = temp.path().join("sample.parquet");
        write_test_parquet(&parquet_path);

        let resolved = resolve_cli_inputs(&[parquet_path.to_string_lossy().into_owned()])
            .expect("resolve parquet");
        assert!(resolved[0].imported);

        let imported = hdf5_metno::File::open(&resolved[0].hdf5_path).expect("open imported hdf5");
        let columns = imported.group("columns").expect("columns");
        assert_eq!(
            columns
                .attr(COLUMN_ORDER_ATTR)
                .expect("column order attr")
                .read_1d::<VarLenUnicode>()
                .expect("read column order")
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["id", "label"]
        );
        assert_eq!(
            columns
                .dataset("id")
                .expect("id dataset")
                .read_1d::<i64>()
                .expect("id values")
                .to_vec(),
            vec![1, 2]
        );
        assert_eq!(
            columns
                .dataset("label")
                .expect("label dataset")
                .read_1d::<VarLenUnicode>()
                .expect("label values")
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["alpha", "beta"]
        );
    }

    fn write_test_parquet(path: &std::path::Path) {
        use parquet::column::writer::ColumnWriter;

        let schema = parse_message_type(
            "message test_schema {
                REQUIRED INT64 id;
                REQUIRED BINARY label (STRING);
            }",
        )
        .expect("parse schema");
        let props = WriterProperties::builder().build();
        let file = std::fs::File::create(path).expect("create parquet file");
        let mut writer =
            SerializedFileWriter::new(file, schema.into(), props.into()).expect("parquet writer");
        let mut row_group = writer.next_row_group().expect("next row group");

        if let Some(mut column) = row_group.next_column().expect("id column") {
            match column.untyped() {
                ColumnWriter::Int64ColumnWriter(typed) => {
                    typed
                        .write_batch(&[1_i64, 2_i64], None, None)
                        .expect("write ids");
                }
                _ => panic!("unexpected parquet id column type"),
            }
            column.close().expect("close id column");
        }

        if let Some(mut column) = row_group.next_column().expect("label column") {
            match column.untyped() {
                ColumnWriter::ByteArrayColumnWriter(typed) => {
                    let values = vec![ByteArray::from("alpha"), ByteArray::from("beta")];
                    typed
                        .write_batch(&values, None, None)
                        .expect("write labels");
                }
                _ => panic!("unexpected parquet label column type"),
            }
            column.close().expect("close label column");
        }

        row_group.close().expect("close row group");
        writer.close().expect("close parquet writer");
    }
}
