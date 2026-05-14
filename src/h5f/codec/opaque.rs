use hdf5_metno::{Dataset, Selection};
use ndarray::{Array1, Array2};

use crate::error::AppError;

use super::super::{
    compound::{read_dataset_raw_bytes, read_selected_values_bytes},
    meta::DatasetMeta,
};

pub fn format_opaque_bytes_for_edit(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn compact_opaque_preview(bytes: &[u8], max_bytes: usize) -> String {
    let shown = bytes
        .iter()
        .take(max_bytes)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    if bytes.len() > max_bytes {
        format!("{shown} …")
    } else {
        shown
    }
}

fn hexdump_opaque_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "<empty>".to_string();
    }

    bytes
        .chunks(16)
        .enumerate()
        .map(|(line_idx, chunk)| {
            format!(
                "{:04x}: {}",
                line_idx * 16,
                chunk
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn parse_opaque_bytes_from_text(
    text: &str,
    expected_len: usize,
) -> Result<Vec<u8>, AppError> {
    let mut bytes = Vec::new();
    for token in text
        .split(|ch: char| ch.is_ascii_whitespace() || ch == ',' || ch == ';')
        .filter(|token| !token.is_empty())
    {
        let token = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
            .unwrap_or(token);
        if token.len() != 2 {
            return Err(AppError::EditError(format!(
                "Invalid opaque byte '{token}'. Use two-digit hex bytes like 'de ad be ef'"
            )));
        }
        let byte = u8::from_str_radix(token, 16).map_err(|_| {
            AppError::EditError(format!(
                "Invalid opaque byte '{token}'. Use hexadecimal values from 00 to ff"
            ))
        })?;
        bytes.push(byte);
    }

    if bytes.len() != expected_len {
        return Err(AppError::EditError(format!(
            "Expected {expected_len} opaque bytes, got {}",
            bytes.len()
        )));
    }

    Ok(bytes)
}

fn opaque_strings_from_bytes(
    bytes: &[u8],
    item_size: usize,
    expected_count: usize,
) -> Result<Vec<String>, AppError> {
    if item_size == 0 {
        return Ok(vec!["".to_string(); expected_count]);
    }
    let expected_len = item_size
        .checked_mul(expected_count)
        .ok_or_else(|| AppError::EditError("Opaque byte count overflowed usize".to_string()))?;
    if bytes.len() != expected_len {
        return Err(AppError::EditError(format!(
            "Opaque read size mismatch: expected {expected_len} bytes, got {}",
            bytes.len()
        )));
    }
    Ok(bytes
        .chunks_exact(item_size)
        .map(format_opaque_bytes_for_edit)
        .collect())
}

pub fn read_opaque_values_1d(
    dataset: &Dataset,
    selection: Selection,
) -> Result<Array1<String>, AppError> {
    let dtype = dataset.dtype()?;
    let item_size = dtype.size();
    let (bytes, out_shape) = read_selected_values_bytes(dataset, selection)?;
    let total = out_shape.iter().product::<usize>();
    Ok(Array1::from_vec(opaque_strings_from_bytes(
        &bytes, item_size, total,
    )?))
}

pub fn read_opaque_values_2d(
    dataset: &Dataset,
    selection: Selection,
) -> Result<Array2<String>, AppError> {
    let dtype = dataset.dtype()?;
    let item_size = dtype.size();
    let (bytes, out_shape) = read_selected_values_bytes(dataset, selection)?;
    if out_shape.len() != 2 {
        return Err(AppError::EditError(format!(
            "Expected 2D opaque selection, got shape {:?}",
            out_shape
        )));
    }
    let rows = out_shape[0];
    let cols = out_shape[1];
    let values = opaque_strings_from_bytes(&bytes, item_size, rows * cols)?;
    Array2::from_shape_vec((rows, cols), values)
        .map_err(|err| AppError::EditError(format!("Failed reshaping opaque matrix data: {err}")))
}

pub fn read_opaque_dataset_preview(
    dataset: &Dataset,
    meta: &DatasetMeta,
) -> Result<String, AppError> {
    let bytes = read_dataset_raw_bytes(dataset)?;
    let item_size = meta.data_bytesize;
    let reason = meta
        .unsupported_reason
        .as_deref()
        .unwrap_or("Datatype fallback");

    if item_size == 0 {
        return Ok(format!(
            "{}\nshape {:?}\n\n<zero-sized opaque values>",
            meta.data_type,
            dataset.shape()
        ));
    }

    if dataset.size() <= 1 {
        return Ok(format!(
            "{}\n{}\n\n{}",
            meta.data_type,
            reason,
            hexdump_opaque_bytes(&bytes)
        ));
    }

    let preview_limit = 64usize;
    let mut out = format!(
        "{}\n{}\nshape {:?}\n\n",
        meta.data_type,
        reason,
        dataset.shape()
    );
    for (idx, chunk) in bytes
        .chunks_exact(item_size)
        .take(preview_limit)
        .enumerate()
    {
        out.push_str(&format!("[{idx}] {}\n", compact_opaque_preview(chunk, 24)));
    }
    if dataset.size() > preview_limit {
        out.push_str("...\n");
    }
    Ok(out.trim_end().to_string())
}
