use std::str::FromStr;

use hdf5_metno::{
    types::{FixedAscii, FixedUnicode, TypeDescriptor, VarLenAscii, VarLenUnicode},
    Attribute, Dataset, Group, H5Type, ObjectReference2,
};
use ndarray::IxDyn;

use crate::error::AppError;

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
        TypeDescriptor::Enum(_) | TypeDescriptor::Compound(_) => {
            let data: Vec<u8> = attr.read_raw()?;
            let new_attr = group
                .new_attr_builder()
                .empty_as(&type_desc)
                .create(new_name)?;
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
        TypeDescriptor::FixedUnicode(size) => match size {
            0..255 => copy_to_group::<FixedUnicode<255>>(attr, group, &type_desc, new_name)?,
            255..4096 => copy_to_group::<FixedUnicode<4096>>(attr, group, &type_desc, new_name)?,
            _ => copy_to_group::<VarLenUnicode>(attr, group, &type_desc, new_name)?,
        },
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
        TypeDescriptor::FixedAscii(size) => match size {
            0..255 => copy_to_group::<FixedAscii<255>>(attr, group, &type_desc, new_name)?,
            255..4096 => copy_to_group::<FixedAscii<4096>>(attr, group, &type_desc, new_name)?,
            _ => copy_to_group::<VarLenAscii>(attr, group, &type_desc, new_name)?,
        },
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

pub fn write_scalar_attr_from_text(attr: &Attribute, new_value: &str) -> Result<String, AppError> {
    let type_desc = attr.dtype()?.to_descriptor()?;
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

fn write_parsed_scalar<T>(
    attr: &Attribute,
    new_value: &str,
    type_name: &str,
) -> Result<(), AppError>
where
    T: H5Type + FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    let parsed = T::from_str(new_value)
        .map_err(|e| AppError::EditError(format!("Failed to convert to {}: {}", type_name, e)))?;
    attr.write_scalar(&parsed)
        .map_err(|e| AppError::EditError(format!("Failed to write attribute: {}", e)))
}

pub fn non_editable_scalar_error(type_desc: &TypeDescriptor) -> AppError {
    match type_desc {
        TypeDescriptor::Enum(_) => AppError::EditError(
            "Editing enum attributes is not supported".to_string(),
        ),
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
        Encoding::Ascii => Ok(dataset.read_scalar::<VarLenAscii>()?.to_string()),
        Encoding::UTF8 => Ok(dataset.read_scalar::<VarLenUnicode>()?.to_string()),
        Encoding::UTF8Fixed => Ok(dataset.read_scalar::<FixedUnicode<32768>>()?.to_string()),
        Encoding::AsciiFixed => Ok(dataset.read_scalar::<FixedAscii<32768>>()?.to_string()),
        Encoding::LittleEndian => Err(AppError::EditError(
            "LittleEndian not supported for string data".to_string(),
        )),
        Encoding::Unknown => Err(AppError::EditError(
            "Unknown encoding not supported for string data".to_string(),
        )),
    }
}
