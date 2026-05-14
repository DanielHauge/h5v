use std::str::FromStr;

use hdf5_metno::{
    types::{FixedAscii, FixedUnicode, TypeDescriptor, VarLenAscii, VarLenUnicode},
    Dataset, H5Type, Selection,
};
use ndarray::{arr0, IxDyn};

use crate::error::AppError;

use super::super::{
    compound::{
        read_projected_selection_bytes, read_selected_element_bytes,
        write_projected_selection_bytes, write_selected_element_bytes,
    },
    meta::{DatasetMeta, Encoding},
};
use super::{
    enum_codec::{
        decode_enum_value_from_bytes, encode_enum_value_bytes, format_enum_value_for_edit,
    },
    fixed_string::{decode_fixed_string_value, encode_fixed_string_value},
    opaque::parse_opaque_bytes_from_text,
    parse_scalar, scalar_text_codec,
};

pub fn format_dataset_value_for_edit(
    dataset: &Dataset,
    meta: &DatasetMeta,
    selection: Option<&Selection>,
) -> Result<String, AppError> {
    if meta.is_opaque() {
        let bytes = read_selected_element_bytes(dataset, selection)?;
        return Ok(super::format_opaque_bytes_for_edit(&bytes));
    }

    let type_desc = dataset_value_type_descriptor(meta);

    if meta.compound_projection.is_some() {
        let bytes = read_projected_selection_bytes(dataset, meta, selection)?;
        return format_projected_value_for_edit(&type_desc, &bytes);
    }

    validate_dataset_value_edit_support(&type_desc)?;

    match &type_desc {
        TypeDescriptor::Integer(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => {
                Ok(read_selected_dataset_scalar::<i8>(dataset, selection)?.to_string())
            }
            hdf5_metno::types::IntSize::U2 => {
                Ok(read_selected_dataset_scalar::<i16>(dataset, selection)?.to_string())
            }
            hdf5_metno::types::IntSize::U4 => {
                Ok(read_selected_dataset_scalar::<i32>(dataset, selection)?.to_string())
            }
            hdf5_metno::types::IntSize::U8 => {
                Ok(read_selected_dataset_scalar::<i64>(dataset, selection)?.to_string())
            }
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => {
                Ok(read_selected_dataset_scalar::<u8>(dataset, selection)?.to_string())
            }
            hdf5_metno::types::IntSize::U2 => {
                Ok(read_selected_dataset_scalar::<u16>(dataset, selection)?.to_string())
            }
            hdf5_metno::types::IntSize::U4 => {
                Ok(read_selected_dataset_scalar::<u32>(dataset, selection)?.to_string())
            }
            hdf5_metno::types::IntSize::U8 => {
                Ok(read_selected_dataset_scalar::<u64>(dataset, selection)?.to_string())
            }
        },
        TypeDescriptor::Float(float_size) => match float_size {
            hdf5_metno::types::FloatSize::U4 => {
                Ok(read_selected_dataset_scalar::<f32>(dataset, selection)?.to_string())
            }
            hdf5_metno::types::FloatSize::U8 => {
                Ok(read_selected_dataset_scalar::<f64>(dataset, selection)?.to_string())
            }
        },
        TypeDescriptor::Boolean => {
            Ok(read_selected_dataset_scalar::<bool>(dataset, selection)?.to_string())
        }
        TypeDescriptor::VarLenAscii => {
            Ok(read_selected_dataset_scalar::<VarLenAscii>(dataset, selection)?.to_string())
        }
        TypeDescriptor::VarLenUnicode => {
            Ok(read_selected_dataset_scalar::<VarLenUnicode>(dataset, selection)?.to_string())
        }
        TypeDescriptor::FixedAscii(_)
        | TypeDescriptor::FixedUnicode(_)
        | TypeDescriptor::Enum(_) => {
            let bytes = read_selected_element_bytes(dataset, selection)?;
            format_scalar_memory_for_edit(&type_desc, &bytes)
        }
        _ => Err(non_editable_dataset_error(&type_desc)),
    }
}

pub fn write_dataset_value_from_text(
    dataset: &Dataset,
    meta: &DatasetMeta,
    selection: Option<&Selection>,
    new_value: &str,
) -> Result<String, AppError> {
    if meta.is_opaque() {
        let bytes = parse_opaque_bytes_from_text(new_value, meta.data_bytesize)?;
        write_selected_element_bytes(dataset, selection, &bytes)?;
        dataset.file()?.flush()?;
        return Ok(meta.data_type.clone());
    }

    let type_desc = dataset_value_type_descriptor(meta);

    if meta.compound_projection.is_some() {
        let bytes = encode_projected_value_from_text(&type_desc, new_value)?;
        write_projected_selection_bytes(dataset, meta, selection, &bytes)?;
        dataset.file()?.flush()?;
        return Ok(type_desc.to_string());
    }

    validate_dataset_value_edit_support(&type_desc)?;

    match &type_desc {
        TypeDescriptor::Integer(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => write_selected_dataset_scalar::<i8>(
                dataset,
                selection,
                parse_scalar::<i8>(new_value, "i8")?,
            )?,
            hdf5_metno::types::IntSize::U2 => write_selected_dataset_scalar::<i16>(
                dataset,
                selection,
                parse_scalar::<i16>(new_value, "i16")?,
            )?,
            hdf5_metno::types::IntSize::U4 => write_selected_dataset_scalar::<i32>(
                dataset,
                selection,
                parse_scalar::<i32>(new_value, "i32")?,
            )?,
            hdf5_metno::types::IntSize::U8 => write_selected_dataset_scalar::<i64>(
                dataset,
                selection,
                parse_scalar::<i64>(new_value, "i64")?,
            )?,
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => write_selected_dataset_scalar::<u8>(
                dataset,
                selection,
                parse_scalar::<u8>(new_value, "u8")?,
            )?,
            hdf5_metno::types::IntSize::U2 => write_selected_dataset_scalar::<u16>(
                dataset,
                selection,
                parse_scalar::<u16>(new_value, "u16")?,
            )?,
            hdf5_metno::types::IntSize::U4 => write_selected_dataset_scalar::<u32>(
                dataset,
                selection,
                parse_scalar::<u32>(new_value, "u32")?,
            )?,
            hdf5_metno::types::IntSize::U8 => write_selected_dataset_scalar::<u64>(
                dataset,
                selection,
                parse_scalar::<u64>(new_value, "u64")?,
            )?,
        },
        TypeDescriptor::Float(float_size) => match float_size {
            hdf5_metno::types::FloatSize::U4 => write_selected_dataset_scalar::<f32>(
                dataset,
                selection,
                parse_scalar::<f32>(new_value, "f32")?,
            )?,
            hdf5_metno::types::FloatSize::U8 => write_selected_dataset_scalar::<f64>(
                dataset,
                selection,
                parse_scalar::<f64>(new_value, "f64")?,
            )?,
        },
        TypeDescriptor::Boolean => write_selected_dataset_scalar::<bool>(
            dataset,
            selection,
            parse_scalar::<bool>(new_value, "bool")?,
        )?,
        TypeDescriptor::VarLenAscii => write_selected_dataset_scalar::<VarLenAscii>(
            dataset,
            selection,
            VarLenAscii::from_ascii(new_value).map_err(|e| {
                AppError::EditError(format!("Failed to convert to VarLenAscii: {}", e))
            })?,
        )?,
        TypeDescriptor::VarLenUnicode => write_selected_dataset_scalar::<VarLenUnicode>(
            dataset,
            selection,
            VarLenUnicode::from_str(new_value).map_err(|e| {
                AppError::EditError(format!("Failed to convert to VarLenUnicode: {}", e))
            })?,
        )?,
        TypeDescriptor::FixedAscii(_)
        | TypeDescriptor::FixedUnicode(_)
        | TypeDescriptor::Enum(_) => {
            let bytes = encode_scalar_memory_from_text(&type_desc, new_value)?;
            write_selected_element_bytes(dataset, selection, &bytes)?
        }
        _ => return Err(non_editable_dataset_error(&type_desc)),
    }

    dataset.file()?.flush()?;
    Ok(type_desc.to_string())
}

fn dataset_value_type_descriptor(meta: &DatasetMeta) -> TypeDescriptor {
    meta.compound_projection
        .as_ref()
        .map(|projection| projection.field_type.clone())
        .unwrap_or_else(|| meta.type_descriptor.clone())
}

fn validate_dataset_value_edit_support(type_desc: &TypeDescriptor) -> Result<(), AppError> {
    if scalar_text_codec(type_desc).is_some()
        || matches!(
            type_desc,
            TypeDescriptor::Enum(_)
                | TypeDescriptor::FixedAscii(_)
                | TypeDescriptor::FixedUnicode(_)
        )
    {
        return Ok(());
    }

    Err(non_editable_dataset_error(type_desc))
}

fn non_editable_dataset_error(type_desc: &TypeDescriptor) -> AppError {
    match type_desc {
        TypeDescriptor::Compound(_) => AppError::EditError(
            "Editing whole compound dataset values is not supported".to_string(),
        ),
        TypeDescriptor::FixedArray(_, _) | TypeDescriptor::VarLenArray(_) => {
            AppError::EditError("Editing nested array dataset values is not supported".to_string())
        }
        TypeDescriptor::Reference(_) => {
            AppError::EditError("Editing reference dataset values is not supported".to_string())
        }
        _ => AppError::EditError(format!(
            "{} dataset type is not supported for editing",
            type_desc
        )),
    }
}

fn read_selected_dataset_scalar<T>(
    dataset: &Dataset,
    selection: Option<&Selection>,
) -> Result<T, AppError>
where
    T: H5Type + Clone,
{
    if let Some(selection) = selection {
        let values = dataset.read_slice::<T, _, IxDyn>(selection.clone())?;
        if values.len() != 1 {
            return Err(AppError::EditError(format!(
                "Expected a single selected dataset value, got {}",
                values.len()
            )));
        }
        values
            .iter()
            .next()
            .cloned()
            .ok_or_else(|| AppError::EditError("Selected dataset value was empty".to_string()))
    } else {
        dataset.read_scalar::<T>().map_err(AppError::from)
    }
}

fn write_selected_dataset_scalar<T>(
    dataset: &Dataset,
    selection: Option<&Selection>,
    value: T,
) -> Result<(), AppError>
where
    T: H5Type,
{
    if let Some(selection) = selection {
        dataset
            .write_slice(arr0(value).view(), selection.clone())
            .map_err(|e| AppError::EditError(format!("Failed to write dataset value: {}", e)))
    } else {
        dataset
            .write_scalar(&value)
            .map_err(|e| AppError::EditError(format!("Failed to write dataset value: {}", e)))
    }
}

fn format_scalar_memory_for_edit(
    type_desc: &TypeDescriptor,
    bytes: &[u8],
) -> Result<String, AppError> {
    match type_desc {
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U1) => {
            Ok(i8::from_le_bytes(to_array(bytes)?).to_string())
        }
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U2) => {
            Ok(i16::from_le_bytes(to_array(bytes)?).to_string())
        }
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U4) => {
            Ok(i32::from_le_bytes(to_array(bytes)?).to_string())
        }
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U8) => {
            Ok(i64::from_le_bytes(to_array(bytes)?).to_string())
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U1) => {
            Ok(u8::from_le_bytes(to_array(bytes)?).to_string())
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U2) => {
            Ok(u16::from_le_bytes(to_array(bytes)?).to_string())
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U4) => {
            Ok(u32::from_le_bytes(to_array(bytes)?).to_string())
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U8) => {
            Ok(u64::from_le_bytes(to_array(bytes)?).to_string())
        }
        TypeDescriptor::Float(hdf5_metno::types::FloatSize::U4) => {
            Ok(f32::from_le_bytes(to_array(bytes)?).to_string())
        }
        TypeDescriptor::Float(hdf5_metno::types::FloatSize::U8) => {
            Ok(f64::from_le_bytes(to_array(bytes)?).to_string())
        }
        TypeDescriptor::Boolean => Ok((u8::from_le_bytes(to_array(bytes)?) != 0).to_string()),
        TypeDescriptor::FixedAscii(size) => decode_fixed_string_value(bytes, *size, true),
        TypeDescriptor::FixedUnicode(size) => decode_fixed_string_value(bytes, *size, false),
        TypeDescriptor::Enum(enum_type) => Ok(format_enum_value_for_edit(
            decode_enum_value_from_bytes(bytes, enum_type)?,
            enum_type,
        )),
        _ => Err(non_editable_dataset_error(type_desc)),
    }
}

fn format_fixed_array_memory_for_edit(
    inner_type: &TypeDescriptor,
    size: usize,
    bytes: &[u8],
) -> Result<String, AppError> {
    let inner_size = inner_type.size();
    let expected_len = inner_size * size;
    if bytes.len() != expected_len {
        return Err(AppError::EditError(format!(
            "FixedArray edit size mismatch: expected {} bytes, got {}",
            expected_len,
            bytes.len()
        )));
    }

    bytes
        .chunks_exact(inner_size)
        .take(size)
        .map(|chunk| format_scalar_memory_for_edit(inner_type, chunk))
        .collect::<Result<Vec<_>, _>>()
        .map(|values| values.join("\n"))
}

fn encode_scalar_memory_from_text(
    type_desc: &TypeDescriptor,
    new_value: &str,
) -> Result<Vec<u8>, AppError> {
    match type_desc {
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U1) => {
            Ok(parse_scalar::<i8>(new_value, "i8")?.to_le_bytes().to_vec())
        }
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U2) => {
            Ok(parse_scalar::<i16>(new_value, "i16")?
                .to_le_bytes()
                .to_vec())
        }
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U4) => {
            Ok(parse_scalar::<i32>(new_value, "i32")?
                .to_le_bytes()
                .to_vec())
        }
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U8) => {
            Ok(parse_scalar::<i64>(new_value, "i64")?
                .to_le_bytes()
                .to_vec())
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U1) => {
            Ok(parse_scalar::<u8>(new_value, "u8")?.to_le_bytes().to_vec())
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U2) => {
            Ok(parse_scalar::<u16>(new_value, "u16")?
                .to_le_bytes()
                .to_vec())
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U4) => {
            Ok(parse_scalar::<u32>(new_value, "u32")?
                .to_le_bytes()
                .to_vec())
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U8) => {
            Ok(parse_scalar::<u64>(new_value, "u64")?
                .to_le_bytes()
                .to_vec())
        }
        TypeDescriptor::Float(hdf5_metno::types::FloatSize::U4) => {
            Ok(parse_scalar::<f32>(new_value, "f32")?
                .to_le_bytes()
                .to_vec())
        }
        TypeDescriptor::Float(hdf5_metno::types::FloatSize::U8) => {
            Ok(parse_scalar::<f64>(new_value, "f64")?
                .to_le_bytes()
                .to_vec())
        }
        TypeDescriptor::Boolean => Ok(vec![u8::from(parse_scalar::<bool>(new_value, "bool")?)]),
        TypeDescriptor::FixedAscii(size) => encode_fixed_string_value(new_value, *size, true),
        TypeDescriptor::FixedUnicode(size) => encode_fixed_string_value(new_value, *size, false),
        TypeDescriptor::Enum(enum_type) => encode_enum_value_bytes(new_value, enum_type),
        _ => Err(non_editable_dataset_error(type_desc)),
    }
}

fn encode_fixed_array_memory_from_text(
    inner_type: &TypeDescriptor,
    size: usize,
    new_value: &str,
) -> Result<Vec<u8>, AppError> {
    let mut lines = new_value
        .split('\n')
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>();
    if matches!(lines.last(), Some(last) if last.is_empty()) {
        lines.pop();
    }
    if lines.len() != size {
        return Err(AppError::EditError(format!(
            "Expected {size} array values, got {}. Put one value on each line.",
            lines.len()
        )));
    }

    let mut bytes = Vec::with_capacity(inner_type.size() * size);
    for line in lines {
        bytes.extend(encode_scalar_memory_from_text(inner_type, line)?);
    }
    Ok(bytes)
}

fn format_projected_value_for_edit(
    type_desc: &TypeDescriptor,
    bytes: &[u8],
) -> Result<String, AppError> {
    match type_desc {
        TypeDescriptor::FixedArray(inner_type, size) => {
            format_fixed_array_memory_for_edit(inner_type, *size, bytes)
        }
        _ => format_scalar_memory_for_edit(type_desc, bytes),
    }
}

fn encode_projected_value_from_text(
    type_desc: &TypeDescriptor,
    new_value: &str,
) -> Result<Vec<u8>, AppError> {
    match type_desc {
        TypeDescriptor::FixedArray(inner_type, size) => {
            encode_fixed_array_memory_from_text(inner_type, *size, new_value)
        }
        _ => encode_scalar_memory_from_text(type_desc, new_value),
    }
}

fn to_array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], AppError> {
    bytes.try_into().map_err(|_| {
        AppError::EditError(format!(
            "Failed converting {} bytes into fixed array of {} bytes",
            bytes.len(),
            N
        ))
    })
}

pub fn read_scalar_string_dataset(
    dataset: &Dataset,
    encoding: &Encoding,
) -> Result<String, AppError> {
    match encoding {
        Encoding::Ascii => Ok(read_single_value_dataset::<VarLenAscii>(dataset)?.to_string()),
        Encoding::UTF8 => Ok(read_single_value_dataset::<VarLenUnicode>(dataset)?.to_string()),
        Encoding::UTF8Fixed => {
            Ok(read_single_value_dataset::<FixedUnicode<32768>>(dataset)?.to_string())
        }
        Encoding::AsciiFixed => {
            Ok(read_single_value_dataset::<FixedAscii<32768>>(dataset)?.to_string())
        }
        Encoding::LittleEndian => Err(AppError::EditError(
            "LittleEndian not supported for string data".to_string(),
        )),
        Encoding::Unknown => Err(AppError::EditError(
            "Unknown encoding not supported for string data".to_string(),
        )),
    }
}

pub fn read_string_dataset_preview(
    dataset: &Dataset,
    encoding: &Encoding,
) -> Result<String, AppError> {
    if dataset.size() <= 1 {
        return read_scalar_string_dataset(dataset, encoding);
    }

    fn format_values(shape: &[usize], values: impl Iterator<Item = String>) -> String {
        let preview_limit = 64usize;
        let collected = values.take(preview_limit + 1).collect::<Vec<_>>();
        let truncated = collected.len() > preview_limit;
        let shown = if truncated {
            &collected[..preview_limit]
        } else {
            &collected[..]
        };
        let mut out = format!("shape {:?}\n\n", shape);
        for (idx, value) in shown.iter().enumerate() {
            out.push_str(&format!("[{idx}] {value}\n"));
        }
        if truncated {
            out.push_str("...\n");
        }
        out.trim_end().to_string()
    }

    let shape = dataset.shape();
    match encoding {
        Encoding::Ascii => Ok(format_values(
            &shape,
            dataset
                .read::<VarLenAscii, IxDyn>()
                .map_err(AppError::from)?
                .iter()
                .map(|value| value.to_string()),
        )),
        Encoding::UTF8 => Ok(format_values(
            &shape,
            dataset
                .read::<VarLenUnicode, IxDyn>()
                .map_err(AppError::from)?
                .iter()
                .map(|value| value.to_string()),
        )),
        Encoding::UTF8Fixed => Ok(format_values(
            &shape,
            dataset
                .read::<FixedUnicode<32768>, IxDyn>()
                .map_err(AppError::from)?
                .iter()
                .map(|value| value.to_string()),
        )),
        Encoding::AsciiFixed => Ok(format_values(
            &shape,
            dataset
                .read::<FixedAscii<32768>, IxDyn>()
                .map_err(AppError::from)?
                .iter()
                .map(|value| value.to_string()),
        )),
        Encoding::LittleEndian => Err(AppError::EditError(
            "LittleEndian not supported for string data".to_string(),
        )),
        Encoding::Unknown => Err(AppError::EditError(
            "Unknown encoding not supported for string data".to_string(),
        )),
    }
}

pub fn read_single_value_dataset<T>(dataset: &Dataset) -> Result<T, AppError>
where
    T: H5Type + Clone,
{
    if dataset.is_scalar() {
        return dataset.read_scalar::<T>().map_err(AppError::from);
    }

    if dataset.size() != 1 {
        return Err(AppError::DrawingError(format!(
            "Expected dataset with a single value, got shape {:?}",
            dataset.shape()
        )));
    }

    let values = dataset.read::<T, IxDyn>().map_err(AppError::from)?;
    values
        .iter()
        .next()
        .cloned()
        .ok_or_else(|| AppError::DrawingError("Expected one dataset value, found none".to_string()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use hdf5_metno::types::{IntSize, TypeDescriptor};

    use super::{encode_fixed_array_memory_from_text, format_fixed_array_memory_for_edit};

    #[test]
    fn fixed_array_edit_format_uses_one_line_per_value() {
        let rendered = format_fixed_array_memory_for_edit(
            &TypeDescriptor::Integer(IntSize::U2),
            3,
            &[0, 0, 1, 0, 2, 0],
        )
        .expect("failed formatting fixed array edit content");
        assert_eq!(rendered, "0\n1\n2");
    }

    #[test]
    fn fixed_array_edit_encoding_requires_exact_line_count() {
        let encoded = encode_fixed_array_memory_from_text(
            &TypeDescriptor::Integer(IntSize::U2),
            3,
            "3\n4\n5\n",
        )
        .expect("failed encoding fixed array edit content");
        assert_eq!(encoded, vec![3, 0, 4, 0, 5, 0]);

        let err =
            encode_fixed_array_memory_from_text(&TypeDescriptor::Integer(IntSize::U2), 3, "1\n2")
                .expect_err("expected missing array entries to fail");
        assert!(err.to_string().contains("Expected 3 array values"));
    }
}
