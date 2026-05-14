use std::str::FromStr;

use hdf5_metno::h5check;
use hdf5_metno::{
    types::{TypeDescriptor, VarLenAscii, VarLenUnicode},
    Attribute, Group, H5Type, ObjectReference2,
};
use hdf5_metno_sys::h5a::H5Aread;
use ndarray::{Array1, IxDyn};

use crate::error::AppError;

mod dataset;
mod enum_codec;
mod fixed_string;
mod opaque;

pub use dataset::{
    format_dataset_value_for_edit, read_scalar_string_dataset, read_single_value_dataset,
    read_string_dataset_preview, write_dataset_value_from_text,
};
pub use fixed_string::{read_string_attr_values, rewrite_fixed_string_attr, FixedStringRewrite};
pub use opaque::{
    format_opaque_bytes_for_edit, read_opaque_dataset_preview, read_opaque_values_1d,
    read_opaque_values_2d,
};

use self::{
    enum_codec::{
        copy_enum_to_group, format_enum_value_for_edit, read_1d_enum_values,
        read_scalar_enum_value, write_enum_1d_attr_from_text, write_enum_scalar_attr_from_text,
    },
    fixed_string::{
        copy_fixed_string_to_group, format_fixed_string_1d, format_fixed_string_scalar,
        write_fixed_string_1d_attr_from_text, write_fixed_string_memory,
        write_fixed_string_scalar_attr_from_text,
    },
    opaque::parse_opaque_bytes_from_text,
};
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
pub enum AttributeCreateType {
    Bool,
    I64,
    U64,
    F64,
    String,
    Ascii,
}

impl AttributeCreateType {
    pub const ALL: [Self; 6] = [
        Self::Bool,
        Self::I64,
        Self::U64,
        Self::F64,
        Self::String,
        Self::Ascii,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::F64 => "f64",
            Self::String => "string",
            Self::Ascii => "ascii",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::F64 => "f64",
            Self::String => "VarLenUnicode",
            Self::Ascii => "VarLenAscii",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "bool" | "boolean" => Ok(Self::Bool),
            "i64" | "int" | "integer" => Ok(Self::I64),
            "u64" | "uint" | "unsigned" => Ok(Self::U64),
            "f64" | "float" | "double" => Ok(Self::F64),
            "string" | "str" | "text" | "unicode" => Ok(Self::String),
            "ascii" => Ok(Self::Ascii),
            other => Err(AppError::InvalidCommand(format!(
                "Unsupported attribute type '{}'. Expected one of: bool, i64, u64, f64, string, ascii",
                other
            ))),
        }
    }
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

pub fn create_scalar_attr_from_text(
    group: &Group,
    attr_name: &str,
    attr_type: AttributeCreateType,
    value: &str,
) -> Result<String, AppError> {
    match attr_type {
        AttributeCreateType::Bool => {
            let attr = group.new_attr_builder().empty::<bool>().create(attr_name)?;
            write_parsed_scalar::<bool>(&attr, value, "bool")?;
        }
        AttributeCreateType::I64 => {
            let attr = group.new_attr_builder().empty::<i64>().create(attr_name)?;
            write_parsed_scalar::<i64>(&attr, value, "i64")?;
        }
        AttributeCreateType::U64 => {
            let attr = group.new_attr_builder().empty::<u64>().create(attr_name)?;
            write_parsed_scalar::<u64>(&attr, value, "u64")?;
        }
        AttributeCreateType::F64 => {
            let attr = group.new_attr_builder().empty::<f64>().create(attr_name)?;
            write_parsed_scalar::<f64>(&attr, value, "f64")?;
        }
        AttributeCreateType::String => {
            let attr = group
                .new_attr_builder()
                .empty::<VarLenUnicode>()
                .create(attr_name)?;
            write_scalar_attr_from_text(&attr, value)?;
        }
        AttributeCreateType::Ascii => {
            let attr = group
                .new_attr_builder()
                .empty::<VarLenAscii>()
                .create(attr_name)?;
            write_scalar_attr_from_text(&attr, value)?;
        }
    }
    group.file()?.flush()?;
    Ok(attr_type.description().to_string())
}

pub fn ensure_attr_editable(attr: &Attribute) -> Result<(), AppError> {
    if !attr.is_valid() {
        return Err(AppError::EditError("Invalid attribute".to_string()));
    }

    let dtype = attr.dtype()?;
    match dtype.to_descriptor() {
        Ok(type_desc) => validate_attr_edit_support(&type_desc, attr.ndim()),
        Err(err) if err.to_string() == "Unsupported datatype class" => {
            if attr.ndim() > 1 {
                Err(AppError::EditError(format!(
                    "Only scalar and 1D attributes can be edited, got {}D attribute",
                    attr.ndim()
                )))
            } else {
                Ok(())
            }
        }
        Err(err) => Err(err.into()),
    }
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
    let dtype = attr.dtype()?;
    let type_desc = match dtype.to_descriptor() {
        Ok(type_desc) => type_desc,
        Err(err) if err.to_string() == "Unsupported datatype class" => {
            return format_opaque_attr_for_edit(attr, dtype.size());
        }
        Err(err) => return Err(err.into()),
    };

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
        _ => Err(validate_attr_edit_support(type_desc, 1)
            .err()
            .unwrap_or_else(|| {
                AppError::EditError(format!(
                    "Attribute type {} is not supported for editing",
                    type_desc
                ))
            })),
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

fn format_opaque_attr_for_edit(attr: &Attribute, item_size: usize) -> Result<String, AppError> {
    let bytes = read_attr_memory_bytes(attr)?;
    if attr.is_scalar() {
        return Ok(format_opaque_bytes_for_edit(&bytes));
    }

    let lines = if item_size == 0 {
        vec![String::new(); expected_1d_len(attr)]
    } else {
        bytes
            .chunks_exact(item_size)
            .map(format_opaque_bytes_for_edit)
            .collect::<Vec<_>>()
    };
    Ok(lines.join("\n"))
}

pub fn write_attr_from_text(attr: &Attribute, new_value: &str) -> Result<String, AppError> {
    ensure_attr_editable(attr)?;
    let dtype = attr.dtype()?;
    let type_desc = match dtype.to_descriptor() {
        Ok(type_desc) => type_desc,
        Err(err) if err.to_string() == "Unsupported datatype class" => {
            write_opaque_attr_from_text(attr, new_value, dtype.size())?;
            return Ok(format!("opaque[{} bytes]", dtype.size()));
        }
        Err(err) => return Err(err.into()),
    };

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

fn write_opaque_attr_from_text(
    attr: &Attribute,
    new_value: &str,
    item_size: usize,
) -> Result<(), AppError> {
    if attr.is_scalar() {
        let bytes = parse_opaque_bytes_from_text(new_value, item_size)?;
        return write_fixed_string_memory(attr, &bytes);
    }

    let lines = parse_1d_lines(new_value, expected_1d_len(attr))?;
    let mut bytes = Vec::with_capacity(lines.len() * item_size);
    for line in lines {
        bytes.extend(parse_opaque_bytes_from_text(line, item_size)?);
    }
    write_fixed_string_memory(attr, &bytes)
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
        _ => Err(validate_attr_edit_support(type_desc, 1)
            .err()
            .unwrap_or_else(|| {
                AppError::EditError(format!(
                    "Attribute type {} is not supported for editing",
                    type_desc
                ))
            })),
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

pub fn read_attr_memory_bytes(attr: &Attribute) -> Result<Vec<u8>, AppError> {
    let dtype = attr.dtype()?;
    let total_size = dtype.size() * attr.size();
    let mut bytes = vec![0_u8; total_size];
    h5check(unsafe { H5Aread(attr.id(), dtype.id(), bytes.as_mut_ptr().cast()) })
        .map_err(|e| AppError::EditError(format!("Failed to read attribute: {}", e)))?;
    Ok(bytes)
}

pub(crate) fn parse_1d_lines(new_value: &str, expected_len: usize) -> Result<Vec<&str>, AppError> {
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

fn parse_scalar<T>(new_value: &str, type_name: &str) -> Result<T, AppError>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    T::from_str(new_value.trim())
        .map_err(|e| AppError::EditError(format!("Failed to convert to {}: {}", type_name, e)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use hdf5_metno::types::TypeDescriptor;
    use hdf5_metno::File;

    use super::opaque::{format_opaque_bytes_for_edit, parse_opaque_bytes_from_text};
    use super::{
        create_scalar_attr_from_text, parse_1d_lines, validate_attr_edit_support,
        AttributeCreateType,
    };

    fn temp_hdf5_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("h5v-{name}-{unique}.h5"))
    }

    #[test]
    fn opaque_bytes_round_trip_hex_text() {
        let bytes = vec![0xde, 0xad, 0xbe, 0xef];
        assert_eq!(format_opaque_bytes_for_edit(&bytes), "de ad be ef");
        assert_eq!(
            parse_opaque_bytes_from_text("de ad be ef", 4).expect("failed to parse bytes"),
            bytes
        );
        assert_eq!(
            parse_opaque_bytes_from_text("0xde, 0xad, 0xbe, 0xef", 4)
                .expect("failed to parse prefixed bytes"),
            bytes
        );
    }

    #[test]
    fn opaque_bytes_reject_wrong_length() {
        let error =
            parse_opaque_bytes_from_text("de ad", 4).expect_err("expected wrong length error");
        assert!(error.to_string().contains("Expected 4 opaque bytes"));
    }

    #[test]
    fn supports_scalar_enums_and_1d_arrays_only() {
        let enum_desc = TypeDescriptor::Enum(hdf5_metno::types::EnumType {
            size: hdf5_metno::types::IntSize::U1,
            signed: false,
            members: vec![],
        });
        assert!(validate_attr_edit_support(&enum_desc, 0).is_ok());
        assert!(validate_attr_edit_support(&TypeDescriptor::Boolean, 1).is_ok());
        assert!(validate_attr_edit_support(&TypeDescriptor::Boolean, 2).is_err());
    }

    #[test]
    fn parse_1d_lines_preserves_empty_entries() {
        assert_eq!(
            parse_1d_lines("first\n\nthird", 3).expect("failed parsing 1d lines"),
            vec!["first", "", "third"]
        );
    }

    #[test]
    fn creates_scalar_unicode_attribute_from_text() {
        let path = temp_hdf5_path("codec-create-attr");
        let file = File::create(&path).expect("failed creating temp hdf5 file");
        let root = file.as_group().expect("failed opening root as group");
        create_scalar_attr_from_text(&root, "title", AttributeCreateType::String, "hello")
            .expect("failed creating attribute");
        let attr = file.attr("title").expect("failed reading created attr");
        let rendered = super::format_attr_for_edit(&attr).expect("failed formatting created attr");
        assert_eq!(rendered, "hello");
        drop(attr);
        file.close().expect("failed closing temp hdf5 file");
        std::fs::remove_file(path).expect("failed removing temp hdf5 file");
    }

    #[test]
    fn parses_attribute_create_type_aliases() {
        assert_eq!(
            AttributeCreateType::parse("integer").expect("integer alias"),
            AttributeCreateType::I64
        );
        assert_eq!(
            AttributeCreateType::parse("text").expect("text alias"),
            AttributeCreateType::String
        );
        assert!(AttributeCreateType::parse("compound").is_err());
    }
}
