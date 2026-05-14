use hdf5_metno::h5check;
use hdf5_metno::{
    types::{TypeDescriptor, VarLenAscii, VarLenUnicode},
    Attribute, Group,
};
use hdf5_metno_sys::h5a::H5Awrite;

use crate::error::{AppError, FixedStringKind, FixedStringOverflow};

use super::{copy_attr_to_group, parse_1d_lines, read_attr_memory_bytes, write_attr_from_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedStringRewrite {
    ToVarLen,
    Resize(usize),
}

pub(crate) fn copy_fixed_string_to_group(
    attr: &Attribute,
    group: &Group,
    type_desc: &TypeDescriptor,
    new_name: &str,
) -> Result<(), AppError> {
    let data = read_attr_memory_bytes(attr)?;
    let builder = group.new_attr_builder().empty_as(type_desc);
    let new_attr = if attr.is_scalar() {
        builder.create(new_name)?
    } else {
        builder.shape(attr.shape()).create(new_name)?
    };
    write_fixed_string_memory(&new_attr, &data)
}

pub fn rewrite_fixed_string_attr(
    group: &Group,
    attr: &Attribute,
    attr_name: &str,
    new_value: &str,
    rewrite: FixedStringRewrite,
) -> Result<(), AppError> {
    let old_type_desc = attr.dtype()?.to_descriptor()?;
    let new_type_desc = replacement_fixed_string_type(&old_type_desc, rewrite)?;
    let temp_name = unique_temp_attr_name(group, attr_name)?;
    let builder = group.new_attr_builder().empty_as(&new_type_desc);
    {
        let temp_attr = if attr.is_scalar() {
            builder.create(temp_name.as_str())?
        } else {
            builder.shape(attr.shape()).create(temp_name.as_str())?
        };

        if let Err(err) = write_attr_from_text(&temp_attr, new_value) {
            let _ = group.delete_attr(temp_name.as_str());
            return Err(err);
        }

        if let Err(err) = group.delete_attr(attr_name) {
            let _ = group.delete_attr(temp_name.as_str());
            return Err(err.into());
        }

        if let Err(err) = copy_attr_to_group(&temp_attr, group, attr_name) {
            let _ = group.delete_attr(temp_name.as_str());
            return Err(err);
        }
    }

    group.delete_attr(temp_name.as_str())?;
    group.file()?.flush()?;
    Ok(())
}

pub fn read_string_attr_values(attr: &Attribute) -> Result<Vec<String>, AppError> {
    let type_desc = attr.dtype()?.to_descriptor()?;
    match type_desc {
        TypeDescriptor::FixedAscii(size) => {
            if attr.is_scalar() {
                Ok(vec![format_fixed_string_scalar(attr, size, true)?])
            } else if attr.ndim() == 1 {
                read_fixed_string_1d_values(attr, size, true)
            } else {
                Err(AppError::EditError(format!(
                    "Expected scalar or 1D string attribute, got {}D attribute",
                    attr.ndim()
                )))
            }
        }
        TypeDescriptor::FixedUnicode(size) => {
            if attr.is_scalar() {
                Ok(vec![format_fixed_string_scalar(attr, size, false)?])
            } else if attr.ndim() == 1 {
                read_fixed_string_1d_values(attr, size, false)
            } else {
                Err(AppError::EditError(format!(
                    "Expected scalar or 1D string attribute, got {}D attribute",
                    attr.ndim()
                )))
            }
        }
        TypeDescriptor::VarLenAscii => {
            if attr.is_scalar() {
                Ok(vec![attr.read_scalar::<VarLenAscii>()?.to_string()])
            } else if attr.ndim() == 1 {
                Ok(attr
                    .read_1d::<VarLenAscii>()?
                    .into_iter()
                    .map(|value| value.to_string())
                    .collect())
            } else {
                Err(AppError::EditError(format!(
                    "Expected scalar or 1D string attribute, got {}D attribute",
                    attr.ndim()
                )))
            }
        }
        TypeDescriptor::VarLenUnicode => {
            if attr.is_scalar() {
                Ok(vec![attr.read_scalar::<VarLenUnicode>()?.to_string()])
            } else if attr.ndim() == 1 {
                Ok(attr
                    .read_1d::<VarLenUnicode>()?
                    .into_iter()
                    .map(|value| value.to_string())
                    .collect())
            } else {
                Err(AppError::EditError(format!(
                    "Expected scalar or 1D string attribute, got {}D attribute",
                    attr.ndim()
                )))
            }
        }
        other => Err(AppError::EditError(format!(
            "Expected string attribute values, got {}",
            other
        ))),
    }
}

pub(crate) fn format_fixed_string_scalar(
    attr: &Attribute,
    size: usize,
    is_ascii: bool,
) -> Result<String, AppError> {
    let data = read_attr_memory_bytes(attr)?;
    decode_fixed_string_value(&data, size, is_ascii)
}

pub(crate) fn format_fixed_string_1d(
    attr: &Attribute,
    size: usize,
    is_ascii: bool,
) -> Result<String, AppError> {
    read_fixed_string_1d_values(attr, size, is_ascii).map(|values| values.join("\n"))
}

pub(crate) fn write_fixed_string_scalar_attr_from_text(
    attr: &Attribute,
    new_value: &str,
    size: usize,
    is_ascii: bool,
) -> Result<(), AppError> {
    let bytes = encode_fixed_string_value(new_value, size, is_ascii)?;
    write_fixed_string_memory(attr, &bytes)
}

pub(crate) fn write_fixed_string_1d_attr_from_text(
    attr: &Attribute,
    new_value: &str,
    size: usize,
    is_ascii: bool,
) -> Result<(), AppError> {
    let lines = parse_1d_lines(new_value, expected_1d_len(attr))?;
    let mut bytes = Vec::with_capacity(lines.len() * size);
    for line in lines {
        bytes.extend(encode_fixed_string_value(line, size, is_ascii)?);
    }
    write_fixed_string_memory(attr, &bytes)
}

pub(crate) fn decode_fixed_string_value(
    bytes: &[u8],
    size: usize,
    is_ascii: bool,
) -> Result<String, AppError> {
    if size == 0 {
        return Ok(String::new());
    }

    let used_len = bytes
        .iter()
        .rposition(|byte| *byte != 0)
        .map(|index| index + 1)
        .unwrap_or(0);
    let content = &bytes[..used_len];

    if is_ascii && !content.is_ascii() {
        return Err(AppError::EditError(
            "Failed to decode FixedAscii attribute: value contains non-ASCII bytes".to_string(),
        ));
    }

    std::str::from_utf8(content)
        .map(|value| value.to_string())
        .map_err(|e| {
            let kind = if is_ascii {
                "FixedAscii"
            } else {
                "FixedUnicode"
            };
            AppError::EditError(format!("Failed to decode {kind} attribute: {}", e))
        })
}

pub(crate) fn encode_fixed_string_value(
    value: &str,
    size: usize,
    is_ascii: bool,
) -> Result<Vec<u8>, AppError> {
    let bytes = value.as_bytes();
    let kind = if is_ascii {
        FixedStringKind::Ascii
    } else {
        FixedStringKind::Unicode
    };

    if is_ascii && !bytes.is_ascii() {
        return Err(AppError::EditError(format!(
            "Invalid {kind} value: only ASCII characters are allowed"
        )));
    }
    if bytes.len() > size {
        return Err(AppError::FixedStringOverflow(FixedStringOverflow {
            kind,
            current_size: size,
            required_size: bytes.len(),
        }));
    }

    let mut out = vec![0_u8; size];
    out[..bytes.len()].copy_from_slice(bytes);
    Ok(out)
}

pub(crate) fn write_fixed_string_memory(attr: &Attribute, bytes: &[u8]) -> Result<(), AppError> {
    let dtype = attr.dtype()?;
    h5check(unsafe { H5Awrite(attr.id(), dtype.id(), bytes.as_ptr().cast()) })
        .map_err(|e| AppError::EditError(format!("Failed to write attribute: {}", e)))?;
    Ok(())
}

fn replacement_fixed_string_type(
    old_type_desc: &TypeDescriptor,
    rewrite: FixedStringRewrite,
) -> Result<TypeDescriptor, AppError> {
    match (old_type_desc, rewrite) {
        (TypeDescriptor::FixedAscii(_), FixedStringRewrite::ToVarLen) => {
            Ok(TypeDescriptor::VarLenAscii)
        }
        (TypeDescriptor::FixedUnicode(_), FixedStringRewrite::ToVarLen) => {
            Ok(TypeDescriptor::VarLenUnicode)
        }
        (TypeDescriptor::FixedAscii(_), FixedStringRewrite::Resize(size)) => {
            Ok(TypeDescriptor::FixedAscii(size))
        }
        (TypeDescriptor::FixedUnicode(_), FixedStringRewrite::Resize(size)) => {
            Ok(TypeDescriptor::FixedUnicode(size))
        }
        _ => Err(AppError::EditError(format!(
            "Cannot rewrite non-fixed string attribute type {}",
            old_type_desc
        ))),
    }
}

fn unique_temp_attr_name(group: &Group, attr_name: &str) -> Result<String, AppError> {
    let existing = group.attr_names()?;
    let mut index = 0usize;
    loop {
        let candidate = format!("__h5v_tmp_{attr_name}_{index}");
        if !existing.iter().any(|name| name == &candidate) {
            return Ok(candidate);
        }
        index += 1;
    }
}

fn expected_1d_len(attr: &Attribute) -> usize {
    attr.shape().first().copied().unwrap_or(0)
}

fn read_fixed_string_1d_values(
    attr: &Attribute,
    size: usize,
    is_ascii: bool,
) -> Result<Vec<String>, AppError> {
    let data = read_attr_memory_bytes(attr)?;
    let len = expected_1d_len(attr);
    if size == 0 {
        return Ok(std::iter::repeat_n(String::new(), len).collect());
    }
    if data.len() != len * size {
        return Err(AppError::EditError(format!(
            "Fixed string array byte size mismatch: expected {}, got {}",
            len * size,
            data.len()
        )));
    }

    data.chunks_exact(size)
        .map(|chunk| decode_fixed_string_value(chunk, size, is_ascii))
        .collect::<Result<Vec<_>, _>>()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{decode_fixed_string_value, encode_fixed_string_value};

    #[test]
    fn fixed_ascii_runtime_encoding_and_decoding_roundtrip() {
        let bytes = encode_fixed_string_value("abc", 5, true).expect("failed encoding fixed ascii");
        assert_eq!(bytes, vec![b'a', b'b', b'c', 0, 0]);
        assert_eq!(
            decode_fixed_string_value(&bytes, 5, true).expect("failed decoding fixed ascii"),
            "abc"
        );
    }

    #[test]
    fn fixed_unicode_runtime_encoding_checks_capacity() {
        let err = encode_fixed_string_value("hello", 4, false)
            .expect_err("expected over-capacity fixed unicode to fail");
        assert!(err
            .to_string()
            .contains("requires 5 bytes but current fixed size is 4"));
    }
}
