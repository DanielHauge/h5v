use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use hdf5_metno::File;

use crate::error::AppError;

use super::{
    readers::{read_delimited_file, read_parquet_file, read_xlsx_file},
    writer::write_tabular_hdf5,
    SourceFormat, IMPORT_SCHEMA_VERSION,
};

pub(super) fn detect_non_hdf5_format(path: &Path) -> Option<SourceFormat> {
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

pub(super) fn import_tabular_file(
    source_path: &Path,
    format: SourceFormat,
) -> Result<String, AppError> {
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
