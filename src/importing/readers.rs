use std::{fs, path::Path};

use calamine::{open_workbook_auto, Data, Reader};
use csv::{ReaderBuilder, StringRecord};
use parquet::{
    file::reader::{FileReader, SerializedFileReader},
    record::Row,
};

use crate::error::AppError;

use super::{ImportedTable, TabularImport};

pub(super) fn read_delimited_file(
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

pub(super) fn read_xlsx_file(source_path: &Path) -> Result<TabularImport, AppError> {
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

pub(super) fn read_parquet_file(source_path: &Path) -> Result<TabularImport, AppError> {
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
