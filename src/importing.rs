use std::path::{Path, PathBuf};

use hdf5_metno::File;

use crate::error::AppError;

mod cache;
mod readers;
mod writer;

use cache::{detect_non_hdf5_format, import_tabular_file};

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

fn display_name_for_path(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
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
