use std::str::FromStr;

use hdf5_metno::h5check;
use hdf5_metno::{
    types::{EnumType, FixedAscii, FixedUnicode, TypeDescriptor, VarLenAscii, VarLenUnicode},
    Attribute, Dataset, Group, H5Type, ObjectReference2, Selection,
};
use hdf5_metno_sys::h5a::{H5Aread, H5Awrite};
use ndarray::{arr0, Array1, Array2, IxDyn};

use crate::error::{AppError, FixedStringKind, FixedStringOverflow};

use super::{
    compound::{
        read_dataset_raw_bytes, read_projected_selection_bytes, read_selected_element_bytes,
        read_selected_values_bytes, write_projected_selection_bytes, write_selected_element_bytes,
    },
    meta::{DatasetMeta, Encoding},
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
pub enum FixedStringRewrite {
    ToVarLen,
    Resize(usize),
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

fn parse_1d_lines(new_value: &str, expected_len: usize) -> Result<Vec<&str>, AppError> {
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
    read_fixed_string_1d_values(attr, size, is_ascii).map(|values| values.join("\n"))
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

pub fn format_dataset_value_for_edit(
    dataset: &Dataset,
    meta: &DatasetMeta,
    selection: Option<&Selection>,
) -> Result<String, AppError> {
    if meta.is_opaque() {
        let bytes = read_selected_element_bytes(dataset, selection)?;
        return Ok(format_opaque_bytes_for_edit(&bytes));
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

fn parse_opaque_bytes_from_text(text: &str, expected_len: usize) -> Result<Vec<u8>, AppError> {
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

fn parse_scalar<T>(new_value: &str, type_name: &str) -> Result<T, AppError>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    T::from_str(new_value.trim())
        .map_err(|e| AppError::EditError(format!("Failed to convert to {}: {}", type_name, e)))
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

fn encode_enum_value_bytes(new_value: &str, enum_type: &EnumType) -> Result<Vec<u8>, AppError> {
    let value = parse_enum_member_value(new_value, enum_type)?;
    match enum_type.base_type() {
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U1) => {
            Ok((value as i8).to_le_bytes().to_vec())
        }
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U2) => {
            Ok((value as i16).to_le_bytes().to_vec())
        }
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U4) => {
            Ok((value as i32).to_le_bytes().to_vec())
        }
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U8) => {
            Ok((value as i64).to_le_bytes().to_vec())
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U1) => {
            Ok((value as u8).to_le_bytes().to_vec())
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U2) => {
            Ok((value as u16).to_le_bytes().to_vec())
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U4) => {
            Ok((value as u32).to_le_bytes().to_vec())
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U8) => {
            Ok(value.to_le_bytes().to_vec())
        }
        _ => Err(AppError::EditError(format!(
            "Unsupported enum base type: {}",
            enum_type.base_type()
        ))),
    }
}

fn decode_enum_value_from_bytes(bytes: &[u8], enum_type: &EnumType) -> Result<u64, AppError> {
    match enum_type.base_type() {
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U1) => {
            Ok(i8::from_le_bytes(to_array(bytes)?) as u64)
        }
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U2) => {
            Ok(i16::from_le_bytes(to_array(bytes)?) as u64)
        }
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U4) => {
            Ok(i32::from_le_bytes(to_array(bytes)?) as u64)
        }
        TypeDescriptor::Integer(hdf5_metno::types::IntSize::U8) => {
            Ok(i64::from_le_bytes(to_array(bytes)?) as u64)
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U1) => {
            Ok(u8::from_le_bytes(to_array(bytes)?) as u64)
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U2) => {
            Ok(u16::from_le_bytes(to_array(bytes)?) as u64)
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U4) => {
            Ok(u32::from_le_bytes(to_array(bytes)?) as u64)
        }
        TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U8) => {
            Ok(u64::from_le_bytes(to_array(bytes)?))
        }
        _ => Err(AppError::EditError(format!(
            "Unsupported enum base type: {}",
            enum_type.base_type()
        ))),
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
    use std::time::{SystemTime, UNIX_EPOCH};

    use hdf5_metno::types::{EnumMember, EnumType, IntSize, TypeDescriptor};
    use hdf5_metno::File;

    use super::{
        create_scalar_attr_from_text, decode_fixed_string_value,
        encode_fixed_array_memory_from_text, encode_fixed_string_value,
        format_fixed_array_memory_for_edit, format_opaque_bytes_for_edit, parse_1d_lines,
        parse_enum_member_value, parse_opaque_bytes_from_text, validate_attr_edit_support,
        AttributeCreateType,
    };

    fn temp_hdf5_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("h5v-{name}-{unique}.h5"))
    }

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
