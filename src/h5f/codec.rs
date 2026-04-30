use std::str::FromStr;

use hdf5_metno::h5check;
use hdf5_metno::{
    types::{EnumType, FixedAscii, FixedUnicode, TypeDescriptor, VarLenAscii, VarLenUnicode},
    Attribute, Dataset, Group, H5Type, ObjectReference2,
};
use hdf5_metno_sys::h5a::{H5Aread, H5Awrite};
use ndarray::{Array1, IxDyn};

use crate::error::{AppError, FixedStringKind, FixedStringOverflow};

use super::meta::Encoding;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarTextCodec {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Bool,
    VarLenAscii,
    VarLenUnicode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedStringRewrite {
    ToVarLen,
    Resize(usize),
}

pub fn scalar_text_codec(type_desc: &TypeDescriptor) -> Option<ScalarTextCodec> {
    match type_desc {
        TypeDescriptor::Integer(int_size) => Some(match int_size {
            hdf5_metno::types::IntSize::U1 => ScalarTextCodec::I8,
            hdf5_metno::types::IntSize::U2 => ScalarTextCodec::I16,
            hdf5_metno::types::IntSize::U4 => ScalarTextCodec::I32,
            hdf5_metno::types::IntSize::U8 => ScalarTextCodec::I64,
        }),
        TypeDescriptor::Unsigned(int_size) => Some(match int_size {
            hdf5_metno::types::IntSize::U1 => ScalarTextCodec::U8,
            hdf5_metno::types::IntSize::U2 => ScalarTextCodec::U16,
            hdf5_metno::types::IntSize::U4 => ScalarTextCodec::U32,
            hdf5_metno::types::IntSize::U8 => ScalarTextCodec::U64,
        }),
        TypeDescriptor::Float(float_size) => Some(match float_size {
            hdf5_metno::types::FloatSize::U4 => ScalarTextCodec::F32,
            hdf5_metno::types::FloatSize::U8 => ScalarTextCodec::F64,
        }),
        TypeDescriptor::Boolean => Some(ScalarTextCodec::Bool),
        TypeDescriptor::VarLenAscii => Some(ScalarTextCodec::VarLenAscii),
        TypeDescriptor::VarLenUnicode => Some(ScalarTextCodec::VarLenUnicode),
        _ => None,
    }
}

pub fn copy_attr_to_group(attr: &Attribute, group: &Group, new_name: &str) -> Result<(), AppError> {
    let type_desc = attr.dtype()?.to_descriptor()?;
    match type_desc {
        TypeDescriptor::Boolean => copy_to_group::<bool>(attr, group, &type_desc, new_name)?,
        TypeDescriptor::Integer(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => {
                copy_to_group::<i8>(attr, group, &type_desc, new_name)?
            }
            hdf5_metno::types::IntSize::U2 => {
                copy_to_group::<i16>(attr, group, &type_desc, new_name)?
            }
            hdf5_metno::types::IntSize::U4 => {
                copy_to_group::<i32>(attr, group, &type_desc, new_name)?
            }
            hdf5_metno::types::IntSize::U8 => {
                copy_to_group::<i64>(attr, group, &type_desc, new_name)?
            }
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => {
                copy_to_group::<u8>(attr, group, &type_desc, new_name)?
            }
            hdf5_metno::types::IntSize::U2 => {
                copy_to_group::<u16>(attr, group, &type_desc, new_name)?
            }
            hdf5_metno::types::IntSize::U4 => {
                copy_to_group::<u32>(attr, group, &type_desc, new_name)?
            }
            hdf5_metno::types::IntSize::U8 => {
                copy_to_group::<u64>(attr, group, &type_desc, new_name)?
            }
        },
        TypeDescriptor::Float(float_size) => match float_size {
            hdf5_metno::types::FloatSize::U4 => {
                copy_to_group::<f32>(attr, group, &type_desc, new_name)?
            }
            hdf5_metno::types::FloatSize::U8 => {
                copy_to_group::<f64>(attr, group, &type_desc, new_name)?
            }
        },
        TypeDescriptor::Enum(enum_type) => {
            copy_enum_to_group(attr, group, new_name, &enum_type)?;
        }
        TypeDescriptor::Compound(_) => {
            let data: Vec<u8> = attr.read_raw()?;
            let builder = group.new_attr_builder().empty_as(&type_desc);
            let new_attr = if attr.is_scalar() {
                builder.create(new_name)?
            } else {
                builder.shape(attr.shape()).create(new_name)?
            };
            new_attr.write_raw(&data)?;
        }
        TypeDescriptor::Reference(_) => {
            let data: ObjectReference2 = attr.read_scalar()?;
            let new_attr = group
                .new_attr_builder()
                .empty_as(&type_desc)
                .create(new_name)?;
            new_attr.write_scalar(&data)?;
        }
        TypeDescriptor::FixedUnicode(_) | TypeDescriptor::FixedAscii(_) => {
            copy_fixed_string_to_group(attr, group, &type_desc, new_name)?
        }
        TypeDescriptor::VarLenArray(_) => {
            return Err(AppError::EditError(
                "Edit of VarLenArray types are unsupported".to_string(),
            ))
        }
        TypeDescriptor::FixedArray(_, _) => {
            return Err(AppError::EditError(
                "Edit of FixedArray types are unsupported".to_string(),
            ))
        }
        TypeDescriptor::VarLenAscii => {
            copy_to_group::<VarLenAscii>(attr, group, &type_desc, new_name)?
        }
        TypeDescriptor::VarLenUnicode => {
            copy_to_group::<VarLenUnicode>(attr, group, &type_desc, new_name)?
        }
    }
    Ok(())
}

fn copy_fixed_string_to_group(
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

fn copy_enum_to_group(
    attr: &Attribute,
    group: &Group,
    new_name: &str,
    enum_type: &EnumType,
) -> Result<(), AppError> {
    let type_desc = TypeDescriptor::Enum(enum_type.clone());
    let builder = group.new_attr_builder().empty_as(&type_desc);
    let new_attr = if attr.is_scalar() {
        builder.create(new_name)?
    } else {
        builder.shape(attr.shape()).create(new_name)?
    };

    match enum_type.base_type() {
        TypeDescriptor::Integer(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => {
                write_enum_memory(&new_attr, &read_enum_values_as::<i8>(attr)?)
            }
            hdf5_metno::types::IntSize::U2 => {
                write_enum_memory(&new_attr, &read_enum_values_as::<i16>(attr)?)
            }
            hdf5_metno::types::IntSize::U4 => {
                write_enum_memory(&new_attr, &read_enum_values_as::<i32>(attr)?)
            }
            hdf5_metno::types::IntSize::U8 => {
                write_enum_memory(&new_attr, &read_enum_values_as::<i64>(attr)?)
            }
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => {
                write_enum_memory(&new_attr, &read_enum_values_as::<u8>(attr)?)
            }
            hdf5_metno::types::IntSize::U2 => {
                write_enum_memory(&new_attr, &read_enum_values_as::<u16>(attr)?)
            }
            hdf5_metno::types::IntSize::U4 => {
                write_enum_memory(&new_attr, &read_enum_values_as::<u32>(attr)?)
            }
            hdf5_metno::types::IntSize::U8 => {
                write_enum_memory(&new_attr, &read_enum_values_as::<u64>(attr)?)
            }
        },
        _ => Err(AppError::EditError(format!(
            "Unsupported enum base type: {}",
            enum_type.base_type()
        ))),
    }
}

fn copy_to_group<T: H5Type>(
    attr: &Attribute,
    group: &Group,
    type_desc: &TypeDescriptor,
    new_name: &str,
) -> Result<(), hdf5_metno::Error> {
    if attr.is_scalar() {
        let data: T = attr.read_scalar()?;
        let new_attr = group
            .new_attr_builder()
            .empty_as(type_desc)
            .create(new_name)?;
        new_attr.write_scalar(&data)?;
    } else {
        let data = attr.read::<T, IxDyn>()?;
        group
            .new_attr_builder()
            .with_data_as(&data, type_desc)
            .create(new_name)?;
    }
    Ok(())
}

pub fn ensure_attr_editable(attr: &Attribute) -> Result<(), AppError> {
    if !attr.is_valid() {
        return Err(AppError::EditError("Invalid attribute".to_string()));
    }

    let type_desc = attr.dtype()?.to_descriptor()?;
    validate_attr_edit_support(&type_desc, attr.ndim())
}

fn validate_attr_edit_support(type_desc: &TypeDescriptor, ndim: usize) -> Result<(), AppError> {
    if ndim > 1 {
        return Err(AppError::EditError(format!(
            "Only scalar and 1D attributes can be edited, got {ndim}D attribute"
        )));
    }

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

    Err(match type_desc {
        TypeDescriptor::Compound(_) => {
            AppError::EditError("Editing compound attributes is not supported".to_string())
        }
        TypeDescriptor::FixedArray(_, _) | TypeDescriptor::VarLenArray(_) => {
            AppError::EditError("Editing nested array attributes is not supported".to_string())
        }
        TypeDescriptor::Reference(_) => {
            AppError::EditError("Editing reference attributes is not supported".to_string())
        }
        _ => AppError::EditError(format!(
            "{} attribute type is not supported for editing",
            type_desc
        )),
    })
}

pub fn format_attr_for_edit(attr: &Attribute) -> Result<String, AppError> {
    ensure_attr_editable(attr)?;
    let type_desc = attr.dtype()?.to_descriptor()?;

    if attr.is_scalar() {
        return format_scalar_attr_for_edit(attr, &type_desc);
    }

    if attr.ndim() == 1 {
        return format_1d_attr_for_edit(attr, &type_desc);
    }

    Err(AppError::EditError(format!(
        "Only scalar and 1D attributes can be edited, got {}D attribute",
        attr.ndim()
    )))
}

fn format_scalar_attr_for_edit(
    attr: &Attribute,
    type_desc: &TypeDescriptor,
) -> Result<String, AppError> {
    match type_desc {
        TypeDescriptor::Integer(int_size) => Ok(match int_size {
            hdf5_metno::types::IntSize::U1 => attr.read_scalar::<i8>()?.to_string(),
            hdf5_metno::types::IntSize::U2 => attr.read_scalar::<i16>()?.to_string(),
            hdf5_metno::types::IntSize::U4 => attr.read_scalar::<i32>()?.to_string(),
            hdf5_metno::types::IntSize::U8 => attr.read_scalar::<i64>()?.to_string(),
        }),
        TypeDescriptor::Unsigned(int_size) => Ok(match int_size {
            hdf5_metno::types::IntSize::U1 => attr.read_scalar::<u8>()?.to_string(),
            hdf5_metno::types::IntSize::U2 => attr.read_scalar::<u16>()?.to_string(),
            hdf5_metno::types::IntSize::U4 => attr.read_scalar::<u32>()?.to_string(),
            hdf5_metno::types::IntSize::U8 => attr.read_scalar::<u64>()?.to_string(),
        }),
        TypeDescriptor::Float(float_size) => Ok(match float_size {
            hdf5_metno::types::FloatSize::U4 => attr.read_scalar::<f32>()?.to_string(),
            hdf5_metno::types::FloatSize::U8 => attr.read_scalar::<f64>()?.to_string(),
        }),
        TypeDescriptor::Boolean => Ok(attr.read_scalar::<bool>()?.to_string()),
        TypeDescriptor::FixedAscii(size) => format_fixed_string_scalar(attr, *size, true),
        TypeDescriptor::FixedUnicode(size) => format_fixed_string_scalar(attr, *size, false),
        TypeDescriptor::VarLenAscii => Ok(attr.read_scalar::<VarLenAscii>()?.to_string()),
        TypeDescriptor::VarLenUnicode => Ok(attr.read_scalar::<VarLenUnicode>()?.to_string()),
        TypeDescriptor::Enum(enum_type) => Ok(format_enum_value_for_edit(
            read_scalar_enum_value(attr, enum_type)?,
            enum_type,
        )),
        _ => Err(non_editable_scalar_error(type_desc)),
    }
}

fn format_1d_attr_for_edit(
    attr: &Attribute,
    type_desc: &TypeDescriptor,
) -> Result<String, AppError> {
    match type_desc {
        TypeDescriptor::Integer(int_size) => Ok(match int_size {
            hdf5_metno::types::IntSize::U1 => join_display_lines(attr.read_1d::<i8>()?.iter()),
            hdf5_metno::types::IntSize::U2 => join_display_lines(attr.read_1d::<i16>()?.iter()),
            hdf5_metno::types::IntSize::U4 => join_display_lines(attr.read_1d::<i32>()?.iter()),
            hdf5_metno::types::IntSize::U8 => join_display_lines(attr.read_1d::<i64>()?.iter()),
        }),
        TypeDescriptor::Unsigned(int_size) => Ok(match int_size {
            hdf5_metno::types::IntSize::U1 => join_display_lines(attr.read_1d::<u8>()?.iter()),
            hdf5_metno::types::IntSize::U2 => join_display_lines(attr.read_1d::<u16>()?.iter()),
            hdf5_metno::types::IntSize::U4 => join_display_lines(attr.read_1d::<u32>()?.iter()),
            hdf5_metno::types::IntSize::U8 => join_display_lines(attr.read_1d::<u64>()?.iter()),
        }),
        TypeDescriptor::Float(float_size) => Ok(match float_size {
            hdf5_metno::types::FloatSize::U4 => join_display_lines(attr.read_1d::<f32>()?.iter()),
            hdf5_metno::types::FloatSize::U8 => join_display_lines(attr.read_1d::<f64>()?.iter()),
        }),
        TypeDescriptor::Boolean => Ok(join_display_lines(attr.read_1d::<bool>()?.iter())),
        TypeDescriptor::FixedAscii(size) => format_fixed_string_1d(attr, *size, true),
        TypeDescriptor::FixedUnicode(size) => format_fixed_string_1d(attr, *size, false),
        TypeDescriptor::VarLenAscii => {
            Ok(join_display_lines(attr.read_1d::<VarLenAscii>()?.iter()))
        }
        TypeDescriptor::VarLenUnicode => {
            Ok(join_display_lines(attr.read_1d::<VarLenUnicode>()?.iter()))
        }
        TypeDescriptor::Enum(enum_type) => Ok(read_1d_enum_values(attr, enum_type)?
            .into_iter()
            .map(|value| format_enum_value_for_edit(value, enum_type))
            .collect::<Vec<_>>()
            .join("\n")),
        _ => Err(validate_attr_edit_support(type_desc, 1).unwrap_err()),
    }
}

fn join_display_lines<'a, T>(values: impl IntoIterator<Item = &'a T>) -> String
where
    T: std::fmt::Display + 'a,
{
    values
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn write_attr_from_text(attr: &Attribute, new_value: &str) -> Result<String, AppError> {
    ensure_attr_editable(attr)?;
    let type_desc = attr.dtype()?.to_descriptor()?;

    if attr.is_scalar() {
        return write_scalar_attr_from_text(attr, new_value);
    }

    if attr.ndim() == 1 {
        write_1d_attr_from_text(attr, new_value, &type_desc)?;
        return Ok(type_desc.to_string());
    }

    Err(AppError::EditError(format!(
        "Only scalar and 1D attributes can be edited, got {}D attribute",
        attr.ndim()
    )))
}

pub fn write_scalar_attr_from_text(attr: &Attribute, new_value: &str) -> Result<String, AppError> {
    let type_desc = attr.dtype()?.to_descriptor()?;
    if let TypeDescriptor::Enum(enum_type) = &type_desc {
        write_enum_scalar_attr_from_text(attr, new_value, enum_type)?;
        return Ok(type_desc.to_string());
    }
    if let TypeDescriptor::FixedAscii(size) = &type_desc {
        write_fixed_string_scalar_attr_from_text(attr, new_value, *size, true)?;
        return Ok(type_desc.to_string());
    }
    if let TypeDescriptor::FixedUnicode(size) = &type_desc {
        write_fixed_string_scalar_attr_from_text(attr, new_value, *size, false)?;
        return Ok(type_desc.to_string());
    }

    match scalar_text_codec(&type_desc) {
        Some(ScalarTextCodec::I8) => write_parsed_scalar::<i8>(attr, new_value, "i8")?,
        Some(ScalarTextCodec::I16) => write_parsed_scalar::<i16>(attr, new_value, "i16")?,
        Some(ScalarTextCodec::I32) => write_parsed_scalar::<i32>(attr, new_value, "i32")?,
        Some(ScalarTextCodec::I64) => write_parsed_scalar::<i64>(attr, new_value, "i64")?,
        Some(ScalarTextCodec::U8) => write_parsed_scalar::<u8>(attr, new_value, "u8")?,
        Some(ScalarTextCodec::U16) => write_parsed_scalar::<u16>(attr, new_value, "u16")?,
        Some(ScalarTextCodec::U32) => write_parsed_scalar::<u32>(attr, new_value, "u32")?,
        Some(ScalarTextCodec::U64) => write_parsed_scalar::<u64>(attr, new_value, "u64")?,
        Some(ScalarTextCodec::F32) => write_parsed_scalar::<f32>(attr, new_value, "f32")?,
        Some(ScalarTextCodec::F64) => write_parsed_scalar::<f64>(attr, new_value, "f64")?,
        Some(ScalarTextCodec::Bool) => write_parsed_scalar::<bool>(attr, new_value, "bool")?,
        Some(ScalarTextCodec::VarLenAscii) => {
            let ascii = VarLenAscii::from_ascii(new_value).map_err(|e| {
                AppError::EditError(format!("Failed to convert to VarLenAscii: {}", e))
            })?;
            attr.write_scalar(&ascii)
                .map_err(|e| AppError::EditError(format!("Failed to write attribute: {}", e)))?;
        }
        Some(ScalarTextCodec::VarLenUnicode) => {
            let unicode = VarLenUnicode::from_str(new_value).map_err(|e| {
                AppError::EditError(format!("Failed to convert to VarLenUnicode: {}", e))
            })?;
            attr.write_scalar(&unicode)
                .map_err(|e| AppError::EditError(format!("Failed to write attribute: {}", e)))?;
        }
        None => return Err(non_editable_scalar_error(&type_desc)),
    }
    Ok(type_desc.to_string())
}

fn write_1d_attr_from_text(
    attr: &Attribute,
    new_value: &str,
    type_desc: &TypeDescriptor,
) -> Result<(), AppError> {
    match type_desc {
        TypeDescriptor::Integer(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => write_parsed_1d_array::<i8>(attr, new_value, "i8"),
            hdf5_metno::types::IntSize::U2 => write_parsed_1d_array::<i16>(attr, new_value, "i16"),
            hdf5_metno::types::IntSize::U4 => write_parsed_1d_array::<i32>(attr, new_value, "i32"),
            hdf5_metno::types::IntSize::U8 => write_parsed_1d_array::<i64>(attr, new_value, "i64"),
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => write_parsed_1d_array::<u8>(attr, new_value, "u8"),
            hdf5_metno::types::IntSize::U2 => write_parsed_1d_array::<u16>(attr, new_value, "u16"),
            hdf5_metno::types::IntSize::U4 => write_parsed_1d_array::<u32>(attr, new_value, "u32"),
            hdf5_metno::types::IntSize::U8 => write_parsed_1d_array::<u64>(attr, new_value, "u64"),
        },
        TypeDescriptor::Float(float_size) => match float_size {
            hdf5_metno::types::FloatSize::U4 => {
                write_parsed_1d_array::<f32>(attr, new_value, "f32")
            }
            hdf5_metno::types::FloatSize::U8 => {
                write_parsed_1d_array::<f64>(attr, new_value, "f64")
            }
        },
        TypeDescriptor::Boolean => write_parsed_1d_array::<bool>(attr, new_value, "bool"),
        TypeDescriptor::FixedAscii(size) => {
            write_fixed_string_1d_attr_from_text(attr, new_value, *size, true)
        }
        TypeDescriptor::FixedUnicode(size) => {
            write_fixed_string_1d_attr_from_text(attr, new_value, *size, false)
        }
        TypeDescriptor::VarLenAscii => write_ascii_1d_array(attr, new_value),
        TypeDescriptor::VarLenUnicode => write_unicode_1d_array(attr, new_value),
        TypeDescriptor::Enum(enum_type) => write_enum_1d_attr_from_text(attr, new_value, enum_type),
        _ => Err(validate_attr_edit_support(type_desc, 1).unwrap_err()),
    }
}

fn write_parsed_scalar<T>(
    attr: &Attribute,
    new_value: &str,
    type_name: &str,
) -> Result<(), AppError>
where
    T: H5Type + FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    let parsed = T::from_str(new_value.trim())
        .map_err(|e| AppError::EditError(format!("Failed to convert to {}: {}", type_name, e)))?;
    attr.write_scalar(&parsed)
        .map_err(|e| AppError::EditError(format!("Failed to write attribute: {}", e)))
}

fn write_parsed_1d_array<T>(
    attr: &Attribute,
    new_value: &str,
    type_name: &str,
) -> Result<(), AppError>
where
    T: H5Type + FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    let parsed = parse_1d_lines(new_value, expected_1d_len(attr))?
        .into_iter()
        .map(|line| {
            T::from_str(line.trim()).map_err(|e| {
                AppError::EditError(format!("Failed to convert to {}: {}", type_name, e))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    attr.write(Array1::from(parsed).view())
        .map_err(|e| AppError::EditError(format!("Failed to write attribute: {}", e)))
}

fn write_ascii_1d_array(attr: &Attribute, new_value: &str) -> Result<(), AppError> {
    let parsed = parse_1d_lines(new_value, expected_1d_len(attr))?
        .into_iter()
        .map(|line| {
            VarLenAscii::from_ascii(line).map_err(|e| {
                AppError::EditError(format!("Failed to convert to VarLenAscii: {}", e))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    attr.write(Array1::from(parsed).view())
        .map_err(|e| AppError::EditError(format!("Failed to write attribute: {}", e)))
}

fn write_unicode_1d_array(attr: &Attribute, new_value: &str) -> Result<(), AppError> {
    let parsed = parse_1d_lines(new_value, expected_1d_len(attr))?
        .into_iter()
        .map(|line| {
            VarLenUnicode::from_str(line).map_err(|e| {
                AppError::EditError(format!("Failed to convert to VarLenUnicode: {}", e))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    attr.write(Array1::from(parsed).view())
        .map_err(|e| AppError::EditError(format!("Failed to write attribute: {}", e)))
}

fn write_fixed_string_scalar_attr_from_text(
    attr: &Attribute,
    new_value: &str,
    size: usize,
    is_ascii: bool,
) -> Result<(), AppError> {
    let bytes = encode_fixed_string_value(new_value, size, is_ascii)?;
    write_fixed_string_memory(attr, &bytes)
}

fn write_fixed_string_1d_attr_from_text(
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

fn write_enum_scalar_attr_from_text(
    attr: &Attribute,
    new_value: &str,
    enum_type: &EnumType,
) -> Result<(), AppError> {
    let value = parse_enum_member_value(new_value, enum_type)?;
    write_enum_value(attr, value, enum_type)
}

fn write_enum_1d_attr_from_text(
    attr: &Attribute,
    new_value: &str,
    enum_type: &EnumType,
) -> Result<(), AppError> {
    let values = parse_1d_lines(new_value, expected_1d_len(attr))?
        .into_iter()
        .map(|line| parse_enum_member_value(line, enum_type))
        .collect::<Result<Vec<_>, _>>()?;

    match enum_type.base_type() {
        TypeDescriptor::Integer(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => {
                write_enum_array(attr, values.into_iter().map(|value| value as i8).collect())
            }
            hdf5_metno::types::IntSize::U2 => {
                write_enum_array(attr, values.into_iter().map(|value| value as i16).collect())
            }
            hdf5_metno::types::IntSize::U4 => {
                write_enum_array(attr, values.into_iter().map(|value| value as i32).collect())
            }
            hdf5_metno::types::IntSize::U8 => {
                write_enum_array(attr, values.into_iter().map(|value| value as i64).collect())
            }
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => {
                write_enum_array(attr, values.into_iter().map(|value| value as u8).collect())
            }
            hdf5_metno::types::IntSize::U2 => {
                write_enum_array(attr, values.into_iter().map(|value| value as u16).collect())
            }
            hdf5_metno::types::IntSize::U4 => {
                write_enum_array(attr, values.into_iter().map(|value| value as u32).collect())
            }
            hdf5_metno::types::IntSize::U8 => write_enum_array(attr, values),
        },
        _ => Err(AppError::EditError(format!(
            "Unsupported enum base type: {}",
            enum_type.base_type()
        ))),
    }
}

fn write_enum_value(attr: &Attribute, value: u64, enum_type: &EnumType) -> Result<(), AppError> {
    match enum_type.base_type() {
        TypeDescriptor::Integer(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => write_enum_memory(attr, &[(value as i8)]),
            hdf5_metno::types::IntSize::U2 => write_enum_memory(attr, &[(value as i16)]),
            hdf5_metno::types::IntSize::U4 => write_enum_memory(attr, &[(value as i32)]),
            hdf5_metno::types::IntSize::U8 => write_enum_memory(attr, &[(value as i64)]),
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => write_enum_memory(attr, &[(value as u8)]),
            hdf5_metno::types::IntSize::U2 => write_enum_memory(attr, &[(value as u16)]),
            hdf5_metno::types::IntSize::U4 => write_enum_memory(attr, &[(value as u32)]),
            hdf5_metno::types::IntSize::U8 => write_enum_memory(attr, &[value]),
        },
        _ => Err(AppError::EditError(format!(
            "Unsupported enum base type: {}",
            enum_type.base_type()
        ))),
    }
}

fn write_enum_array<T>(attr: &Attribute, values: Vec<T>) -> Result<(), AppError> {
    write_enum_memory(attr, &values)
}

pub fn read_attr_memory_bytes(attr: &Attribute) -> Result<Vec<u8>, AppError> {
    let dtype = attr.dtype()?;
    let total_size = dtype.size() * attr.size();
    let mut bytes = vec![0_u8; total_size];
    h5check(unsafe { H5Aread(attr.id(), dtype.id(), bytes.as_mut_ptr().cast()) })
        .map_err(|e| AppError::EditError(format!("Failed to read attribute: {}", e)))?;
    Ok(bytes)
}

fn write_enum_memory<T>(attr: &Attribute, values: &[T]) -> Result<(), AppError> {
    let dtype = attr.dtype()?;
    h5check(unsafe { H5Awrite(attr.id(), dtype.id(), values.as_ptr().cast()) })
        .map_err(|e| AppError::EditError(format!("Failed to write attribute: {}", e)))?;
    Ok(())
}

fn read_enum_values_as<T: H5Type + Clone>(attr: &Attribute) -> Result<Vec<T>, AppError> {
    if attr.is_scalar() {
        Ok(vec![attr.read_scalar::<T>()?])
    } else {
        Ok(attr.read_1d::<T>()?.to_vec())
    }
}

fn parse_1d_lines<'a>(new_value: &'a str, expected_len: usize) -> Result<Vec<&'a str>, AppError> {
    if new_value.is_empty() {
        return match expected_len {
            0 => Ok(vec![]),
            1 => Ok(vec![""]),
            _ => Err(AppError::EditError(format!(
                "Expected {expected_len} values, got 0"
            ))),
        };
    }

    let lines = new_value.split('\n').collect::<Vec<_>>();
    if lines.len() != expected_len {
        return Err(AppError::EditError(format!(
            "Expected {expected_len} values, got {}",
            lines.len()
        )));
    }

    Ok(lines)
}

fn expected_1d_len(attr: &Attribute) -> usize {
    attr.shape().first().copied().unwrap_or(0)
}

fn format_fixed_string_scalar(
    attr: &Attribute,
    size: usize,
    is_ascii: bool,
) -> Result<String, AppError> {
    let data = read_attr_memory_bytes(attr)?;
    decode_fixed_string_value(&data, size, is_ascii)
}

fn format_fixed_string_1d(
    attr: &Attribute,
    size: usize,
    is_ascii: bool,
) -> Result<String, AppError> {
    let data = read_attr_memory_bytes(attr)?;
    let len = expected_1d_len(attr);
    if size == 0 {
        return Ok(std::iter::repeat_n(String::new(), len)
            .collect::<Vec<_>>()
            .join("\n"));
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
        .map(|values| values.join("\n"))
}

fn decode_fixed_string_value(
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

fn encode_fixed_string_value(
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

fn write_fixed_string_memory(attr: &Attribute, bytes: &[u8]) -> Result<(), AppError> {
    let dtype = attr.dtype()?;
    h5check(unsafe { H5Awrite(attr.id(), dtype.id(), bytes.as_ptr().cast()) })
        .map_err(|e| AppError::EditError(format!("Failed to write attribute: {}", e)))?;
    Ok(())
}

fn read_scalar_enum_value(attr: &Attribute, enum_type: &EnumType) -> Result<u64, AppError> {
    match enum_type.base_type() {
        TypeDescriptor::Integer(int_size) => Ok(match int_size {
            hdf5_metno::types::IntSize::U1 => attr.read_scalar::<i8>()? as u64,
            hdf5_metno::types::IntSize::U2 => attr.read_scalar::<i16>()? as u64,
            hdf5_metno::types::IntSize::U4 => attr.read_scalar::<i32>()? as u64,
            hdf5_metno::types::IntSize::U8 => attr.read_scalar::<i64>()? as u64,
        }),
        TypeDescriptor::Unsigned(int_size) => Ok(match int_size {
            hdf5_metno::types::IntSize::U1 => attr.read_scalar::<u8>()? as u64,
            hdf5_metno::types::IntSize::U2 => attr.read_scalar::<u16>()? as u64,
            hdf5_metno::types::IntSize::U4 => attr.read_scalar::<u32>()? as u64,
            hdf5_metno::types::IntSize::U8 => attr.read_scalar::<u64>()?,
        }),
        _ => Err(AppError::EditError(format!(
            "Unsupported enum base type: {}",
            enum_type.base_type()
        ))),
    }
}

fn read_1d_enum_values(attr: &Attribute, enum_type: &EnumType) -> Result<Vec<u64>, AppError> {
    match enum_type.base_type() {
        TypeDescriptor::Integer(int_size) => Ok(match int_size {
            hdf5_metno::types::IntSize::U1 => attr
                .read_1d::<i8>()?
                .into_iter()
                .map(|value| value as u64)
                .collect(),
            hdf5_metno::types::IntSize::U2 => attr
                .read_1d::<i16>()?
                .into_iter()
                .map(|value| value as u64)
                .collect(),
            hdf5_metno::types::IntSize::U4 => attr
                .read_1d::<i32>()?
                .into_iter()
                .map(|value| value as u64)
                .collect(),
            hdf5_metno::types::IntSize::U8 => attr
                .read_1d::<i64>()?
                .into_iter()
                .map(|value| value as u64)
                .collect(),
        }),
        TypeDescriptor::Unsigned(int_size) => Ok(match int_size {
            hdf5_metno::types::IntSize::U1 => attr
                .read_1d::<u8>()?
                .into_iter()
                .map(|value| value as u64)
                .collect(),
            hdf5_metno::types::IntSize::U2 => attr
                .read_1d::<u16>()?
                .into_iter()
                .map(|value| value as u64)
                .collect(),
            hdf5_metno::types::IntSize::U4 => attr
                .read_1d::<u32>()?
                .into_iter()
                .map(|value| value as u64)
                .collect(),
            hdf5_metno::types::IntSize::U8 => attr.read_1d::<u64>()?.into_iter().collect(),
        }),
        _ => Err(AppError::EditError(format!(
            "Unsupported enum base type: {}",
            enum_type.base_type()
        ))),
    }
}

fn format_enum_value_for_edit(value: u64, enum_type: &EnumType) -> String {
    enum_type
        .members
        .iter()
        .find(|member| member.value == value)
        .map(|member| member.name.clone())
        .unwrap_or_else(|| value.to_string())
}

fn enum_members_display(enum_type: &EnumType) -> String {
    format!(
        "[{}]",
        enum_type
            .members
            .iter()
            .map(|member| format!("\"{}\"", member.name))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn parse_enum_member_value(text: &str, enum_type: &EnumType) -> Result<u64, AppError> {
    let text = text.trim();
    if let Some(member) = enum_type.members.iter().find(|member| member.name == text) {
        return Ok(member.value);
    }

    let parsed = match enum_type.base_type() {
        TypeDescriptor::Integer(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => text.parse::<i8>().map(|value| value as u64),
            hdf5_metno::types::IntSize::U2 => text.parse::<i16>().map(|value| value as u64),
            hdf5_metno::types::IntSize::U4 => text.parse::<i32>().map(|value| value as u64),
            hdf5_metno::types::IntSize::U8 => text.parse::<i64>().map(|value| value as u64),
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => text.parse::<u8>().map(|value| value as u64),
            hdf5_metno::types::IntSize::U2 => text.parse::<u16>().map(|value| value as u64),
            hdf5_metno::types::IntSize::U4 => text.parse::<u32>().map(|value| value as u64),
            hdf5_metno::types::IntSize::U8 => text.parse::<u64>(),
        },
        _ => {
            return Err(AppError::EditError(format!(
                "Unsupported enum base type: {}",
                enum_type.base_type()
            )))
        }
    };

    let parsed = match parsed {
        Ok(parsed) => parsed,
        Err(_) => {
            return Err(AppError::EditError(format!(
                "Invalid enum value '{text}'. Available members: {}",
                enum_members_display(enum_type)
            )))
        }
    };

    if enum_type
        .members
        .iter()
        .any(|member| member.value == parsed)
    {
        Ok(parsed)
    } else {
        Err(AppError::EditError(format!(
            "Invalid enum value '{text}'. Available members: {}",
            enum_members_display(enum_type)
        )))
    }
}

pub fn non_editable_scalar_error(type_desc: &TypeDescriptor) -> AppError {
    match type_desc {
        TypeDescriptor::Compound(_) => AppError::EditError(
            "Editing compound attributes is not supported".to_string(),
        ),
        TypeDescriptor::FixedArray(_, _) | TypeDescriptor::VarLenArray(_) => AppError::EditError(
            "Editing array attributes is not supported".to_string(),
        ),
        TypeDescriptor::FixedAscii(_) => AppError::EditWarning("Editing FixedAscii attributes is disabled due to performance and dependency concerns. \nIf you truly wish to edit this attribute, delete it and create it with desired type such as vlen string".to_string()),
        TypeDescriptor::FixedUnicode(_) => AppError::EditWarning("Editing FixedUnicode attributes is disabled due to performance and dependency concerns. \nIf you truly wish to edit this attribute, delete it and create it with desired type such as vlen string".to_string()),
        TypeDescriptor::Reference(_) => AppError::EditError(
            "Editing reference attributes is not supported".to_string(),
        ),
        _ => AppError::EditError(format!(
            "{} attribute type is not supported for editing",
            type_desc
        )),
    }
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
mod tests {
    use hdf5_metno::types::{EnumMember, EnumType, IntSize, TypeDescriptor};

    use super::{
        decode_fixed_string_value, encode_fixed_string_value, parse_1d_lines,
        parse_enum_member_value, validate_attr_edit_support,
    };

    fn color_enum() -> EnumType {
        EnumType {
            size: IntSize::U1,
            signed: false,
            members: vec![
                EnumMember {
                    name: "RED".to_string(),
                    value: 1,
                },
                EnumMember {
                    name: "GREEN".to_string(),
                    value: 2,
                },
                EnumMember {
                    name: "BLUE".to_string(),
                    value: 3,
                },
            ],
        }
    }

    #[test]
    fn parses_enum_member_names() {
        assert_eq!(
            parse_enum_member_value("GREEN", &color_enum()).expect("failed to parse enum member"),
            2
        );
    }

    #[test]
    fn validates_numeric_enum_membership() {
        assert_eq!(
            parse_enum_member_value("3", &color_enum()).expect("failed to parse enum value"),
            3
        );

        let err = parse_enum_member_value("4", &color_enum())
            .expect_err("expected invalid enum value to fail");
        assert!(err
            .to_string()
            .contains("Available members: [\"RED\", \"GREEN\", \"BLUE\"]"));
    }

    #[test]
    fn invalid_enum_name_lists_available_members() {
        let err = parse_enum_member_value("purple", &color_enum())
            .expect_err("expected invalid enum name to fail");
        assert!(err
            .to_string()
            .contains("Available members: [\"RED\", \"GREEN\", \"BLUE\"]"));
    }

    #[test]
    fn supports_scalar_enums_and_1d_arrays_only() {
        let enum_desc = TypeDescriptor::Enum(color_enum());
        assert!(validate_attr_edit_support(&enum_desc, 0).is_ok());
        assert!(validate_attr_edit_support(&TypeDescriptor::Boolean, 1).is_ok());
        assert!(validate_attr_edit_support(&TypeDescriptor::Boolean, 2).is_err());
    }

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

    #[test]
    fn parse_1d_lines_preserves_empty_entries() {
        assert_eq!(
            parse_1d_lines("first\n\nthird", 3).expect("failed parsing 1d lines"),
            vec!["first", "", "third"]
        );
    }
}
