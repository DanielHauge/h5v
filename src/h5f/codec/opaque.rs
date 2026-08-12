use hdf5_metno::{Dataset, Hyperslab, Selection, SliceOrIndex};
use ndarray::{Array1, Array2};

use crate::error::AppError;

use super::super::{
    compound::{read_dataset_raw_bytes, read_selected_values_bytes},
    meta::DatasetMeta,
};
use super::dataset::bounded_preview_selection;

const OPAQUE_PREVIEW_ELEMENT_BYTES: usize = 24;

pub fn format_opaque_bytes_for_edit(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn hexdump_opaque_bytes_at(bytes: &[u8], byte_offset: usize) -> String {
    if bytes.is_empty() {
        return "<empty>".to_string();
    }

    bytes
        .chunks(16)
        .enumerate()
        .map(|(line_idx, chunk)| {
            let hex = chunk
                .chunks(2)
                .map(|pair| match pair {
                    [first, second] => format!("{first:02x}{second:02x}"),
                    [first] => format!("{first:02x}  "),
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>()
                .join(" ");
            let ascii: String = chunk
                .iter()
                .map(|&byte| {
                    if (0x20..=0x7e).contains(&byte) {
                        byte as char
                    } else {
                        '.'
                    }
                })
                .collect();
            format!("{:08x}: {hex:<39}  |{ascii}|", byte_offset + line_idx * 16)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn hexdump_opaque_bytes(bytes: &[u8]) -> String {
    hexdump_opaque_bytes_at(bytes, 0)
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
    byte_start: usize,
    byte_count: usize,
) -> Result<String, AppError> {
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

    if item_size > OPAQUE_PREVIEW_ELEMENT_BYTES {
        return Ok(format!(
            "{}\n{}\n\n<opaque values are {} bytes each; preview cap is {} bytes>",
            meta.data_type, reason, item_size, OPAQUE_PREVIEW_ELEMENT_BYTES
        ));
    }

    if dataset.is_scalar() {
        return Ok(format!(
            "{}\n{}\n\n{}",
            meta.data_type,
            reason,
            hexdump_opaque_bytes(&read_dataset_raw_bytes(dataset)?)
        ));
    }

    if dataset.size() <= 1 {
        return Ok(format!(
            "{}\n{}\nshape {:?}\n\n{}",
            meta.data_type,
            reason,
            dataset.shape(),
            hexdump_opaque_bytes(&read_dataset_raw_bytes(dataset)?)
        ));
    }

    let preview_limit = 64usize;
    let shape = dataset.shape();
    let total = dataset.size();
    let paged = shape.len() == 1 && total > preview_limit;
    let (bytes, offset, truncated) = if paged {
        let total_bytes = total.saturating_mul(item_size);
        let start = byte_start.min(total_bytes.saturating_sub(1));
        let end = start.saturating_add(byte_count).min(total_bytes);
        let first_value = start / item_size;
        let last_value = end.saturating_add(item_size - 1) / item_size;
        let selection = Selection::Hyperslab(Hyperslab::from(vec![SliceOrIndex::SliceTo {
            start: first_value,
            step: 1,
            end: last_value,
            block: 1,
        }]));
        let (selected, _) = read_selected_values_bytes(dataset, selection)?;
        let selected_start = first_value * item_size;
        let from = start - selected_start;
        let to = from + (end - start);
        (selected[from..to].to_vec(), start, end < total_bytes)
    } else {
        let selection = bounded_preview_selection(&shape, preview_limit);
        let (bytes, _) = read_selected_values_bytes(dataset, selection)?;
        (bytes, 0, total > preview_limit)
    };

    let mut out = format!("{}\n{}\nshape {:?}\n\n", meta.data_type, reason, shape);
    out.push_str(&hexdump_opaque_bytes_at(&bytes, offset));
    if truncated {
        out.push_str("\n...");
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use hdf5_metno::{
        types::{IntSize, TypeDescriptor},
        File,
    };

    use super::{hexdump_opaque_bytes, hexdump_opaque_bytes_at, read_opaque_dataset_preview};
    use crate::{
        h5f::meta::{DatasetMeta, Encoding},
        ui::render::MatrixRenderType,
    };

    fn opaque_meta(item_size: usize) -> DatasetMeta {
        DatasetMeta {
            link_name: None,
            display_name: "opaque".to_string(),
            shape: Vec::new(),
            data_type: format!("opaque[{item_size} bytes]"),
            unsupported_reason: Some("Datatype fallback".to_string()),
            type_descriptor: TypeDescriptor::Unsigned(IntSize::U1),
            data_bytesize: item_size,
            storage_required: 0,
            total_bytes: 0,
            total_elems: 1,
            chunk_shape: None,
            hl: None,
            matrixable: Some(MatrixRenderType::Opaque),
            encoding: Encoding::Unknown,
            image: None,
            enum_render_overrides: None,
            is_link: false,
            filename: String::new(),
            compound_projection: None,
        }
    }

    #[test]
    fn hexdump_uses_paired_hex_and_ascii_gutter() {
        assert_eq!(
            hexdump_opaque_bytes(b"param\0\xff"),
            "00000000: 7061 7261 6d00 ff                        |param..|"
        );
    }

    #[test]
    fn hexdump_aligns_partial_final_row_and_offsets() {
        let bytes: Vec<u8> = (0..17).collect();
        assert_eq!(
            hexdump_opaque_bytes(&bytes),
            "00000000: 0001 0203 0405 0607 0809 0a0b 0c0d 0e0f  |................|\n\
00000010: 10                                       |.|"
        );
    }

    #[test]
    fn hexdump_uses_absolute_page_offset() {
        assert_eq!(
            hexdump_opaque_bytes_at(&[0xde, 0xad], 0x120),
            "00000120: dead                                     |..|"
        );
    }

    #[test]
    fn oversized_scalar_opaque_preview_returns_cap_without_reading() {
        let temp = tempfile::NamedTempFile::new().expect("create temporary HDF5 file");
        let file = File::create(temp.path()).expect("create HDF5 file");
        let dataset = file
            .new_dataset::<u8>()
            .create("opaque")
            .expect("create scalar dataset");
        dataset.write_scalar(&42_u8).expect("write scalar value");

        assert_eq!(
            read_opaque_dataset_preview(&dataset, &opaque_meta(25), 0, 0).expect("preview opaque"),
            "opaque[25 bytes]\nDatatype fallback\n\n<opaque values are 25 bytes each; preview cap is 24 bytes>"
        );
    }

    #[test]
    fn oversized_one_element_opaque_preview_returns_cap_without_reading() {
        let temp = tempfile::NamedTempFile::new().expect("create temporary HDF5 file");
        let file = File::create(temp.path()).expect("create HDF5 file");
        let dataset = file
            .new_dataset_builder()
            .with_data(&[42_u8])
            .create("opaque")
            .expect("create one-element dataset");

        assert_eq!(
            read_opaque_dataset_preview(&dataset, &opaque_meta(25), 0, 0).expect("preview opaque"),
            "opaque[25 bytes]\nDatatype fallback\n\n<opaque values are 25 bytes each; preview cap is 24 bytes>"
        );
    }

    #[test]
    fn safe_scalar_opaque_preview_keeps_xxd_format() {
        let temp = tempfile::NamedTempFile::new().expect("create temporary HDF5 file");
        let file = File::create(temp.path()).expect("create HDF5 file");
        let dataset = file
            .new_dataset::<u8>()
            .create("opaque")
            .expect("create scalar dataset");
        dataset.write_scalar(&42_u8).expect("write scalar value");

        assert_eq!(
            read_opaque_dataset_preview(&dataset, &opaque_meta(1), 0, 0).expect("preview opaque"),
            "opaque[1 bytes]\nDatatype fallback\n\n00000000: 2a                                       |*|"
        );
    }
}
