use hdf5_metno::h5check;
use hdf5_metno::{types::EnumType, Attribute, Group, H5Type};
use hdf5_metno_sys::h5a::H5Awrite;

use crate::error::AppError;

use super::parse_1d_lines;
use hdf5_metno::types::TypeDescriptor;

pub(crate) fn copy_enum_to_group(
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

pub(crate) fn write_enum_scalar_attr_from_text(
    attr: &Attribute,
    new_value: &str,
    enum_type: &EnumType,
) -> Result<(), AppError> {
    let value = parse_enum_member_value(new_value, enum_type)?;
    write_enum_value(attr, value, enum_type)
}

pub(crate) fn write_enum_1d_attr_from_text(
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

pub(crate) fn read_scalar_enum_value(
    attr: &Attribute,
    enum_type: &EnumType,
) -> Result<u64, AppError> {
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

pub(crate) fn read_1d_enum_values(
    attr: &Attribute,
    enum_type: &EnumType,
) -> Result<Vec<u64>, AppError> {
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

pub(crate) fn format_enum_value_for_edit(value: u64, enum_type: &EnumType) -> String {
    enum_type
        .members
        .iter()
        .find(|member| member.value == value)
        .map(|member| member.name.clone())
        .unwrap_or_else(|| value.to_string())
}

pub(crate) fn parse_enum_member_value(text: &str, enum_type: &EnumType) -> Result<u64, AppError> {
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

pub(crate) fn encode_enum_value_bytes(
    new_value: &str,
    enum_type: &EnumType,
) -> Result<Vec<u8>, AppError> {
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

pub(crate) fn decode_enum_value_from_bytes(
    bytes: &[u8],
    enum_type: &EnumType,
) -> Result<u64, AppError> {
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

fn expected_1d_len(attr: &Attribute) -> usize {
    attr.shape().first().copied().unwrap_or(0)
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use hdf5_metno::types::{EnumMember, EnumType, IntSize, TypeDescriptor};

    use super::parse_enum_member_value;

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
    fn enum_validation_test_type_is_well_formed() {
        assert!(matches!(
            TypeDescriptor::Enum(color_enum()),
            TypeDescriptor::Enum(_)
        ));
    }
}
