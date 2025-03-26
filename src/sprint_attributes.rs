use std::f64;

use hdf5_metno::{
    types::{FixedAscii, FixedUnicode, TypeDescriptor},
    Attribute, Error,
};

pub trait Stringer {
    fn to_string(&self) -> String;
}

impl Stringer for Attribute {
    fn to_string(&self) -> String {
        match sprint_attribute(self) {
            Ok(s) => s,
            Err(_) => "N/A".to_string(),
        }
    }
}

fn sprint_attribute_scalar(
    attr: &hdf5_metno::Attribute,
    type_desc: TypeDescriptor,
) -> Result<String, Error> {
    Ok(match type_desc {
        hdf5_metno::types::TypeDescriptor::Integer(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => {
                let read = attr.read_scalar::<i8>()?;
                format!("{:?}", read)
            }
            hdf5_metno::types::IntSize::U2 => {
                let read = attr.read_scalar::<i16>()?;
                format!("{:?}", read)
            }
            hdf5_metno::types::IntSize::U4 => {
                let read = attr.read_scalar::<i32>()?;
                format!("{:?}", read)
            }
            hdf5_metno::types::IntSize::U8 => {
                let read = attr.read_scalar::<i64>()?;
                format!("{:?}", read)
            }
        },
        hdf5_metno::types::TypeDescriptor::Unsigned(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => {
                let read = attr.read_scalar::<u8>()?;
                format!("{:?}", read)
            }
            hdf5_metno::types::IntSize::U2 => {
                let read = attr.read_scalar::<u16>()?;
                format!("{:?}", read)
            }
            hdf5_metno::types::IntSize::U4 => {
                let read = attr.read_scalar::<u32>()?;
                format!("{:?}", read)
            }
            hdf5_metno::types::IntSize::U8 => {
                let read = attr.read_scalar::<u64>()?;
                format!("{:?}", read)
            }
        },
        hdf5_metno::types::TypeDescriptor::Float(float_size) => match float_size {
            hdf5_metno::types::FloatSize::U4 => {
                let read = attr.read_scalar::<f32>()?;
                format!("{:?}", read)
            }
            hdf5_metno::types::FloatSize::U8 => {
                let read = attr.read_scalar::<f64>()?;
                format!("{:?}", read)
            }
        },
        hdf5_metno::types::TypeDescriptor::Boolean => {
            let read = attr.read_scalar::<bool>()?;
            format!("{:?}", read)
        }
        hdf5_metno::types::TypeDescriptor::Enum(enum_type) => match enum_type {
            a => format!("{:?}", a),
        },
        hdf5_metno::types::TypeDescriptor::Compound(_) => unreachable!(),
        hdf5_metno::types::TypeDescriptor::FixedArray(_, _) => unreachable!(),
        hdf5_metno::types::TypeDescriptor::FixedAscii(a) => match a {
            0..32 => {
                let value: FixedAscii<32> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            32..64 => {
                let value: FixedAscii<64> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            64..128 => {
                let value: FixedAscii<128> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            128..256 => {
                let value: FixedAscii<256> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            256..512 => {
                let value: FixedAscii<512> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            512..1024 => {
                let value: FixedAscii<1024> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            1024..2048 => {
                let value: FixedAscii<2048> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            2048..4096 => {
                let value: FixedAscii<4096> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            _ => {
                let value: FixedAscii<8192> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
        },
        hdf5_metno::types::TypeDescriptor::FixedUnicode(a) => match a {
            0..32 => {
                let value: FixedUnicode<32> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            32..64 => {
                let value: FixedUnicode<64> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            64..128 => {
                let value: FixedUnicode<128> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            128..256 => {
                let value: FixedUnicode<256> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            256..512 => {
                let value: FixedUnicode<512> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            512..1024 => {
                let value: FixedUnicode<1024> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            1024..2048 => {
                let value: FixedUnicode<2048> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            2048..4096 => {
                let value: FixedUnicode<4096> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
            _ => {
                let value: FixedUnicode<8192> = attr.read_scalar()?;
                let value_string = value.to_string();
                format!("{value_string}")
            }
        },
        hdf5_metno::types::TypeDescriptor::VarLenArray(_) => unreachable!(),
        hdf5_metno::types::TypeDescriptor::VarLenAscii => {
            let value: hdf5_metno::types::VarLenAscii = attr.read_scalar()?;
            let value_string = value.to_string();
            format!("\"{value_string}\"")
        }
        hdf5_metno::types::TypeDescriptor::VarLenUnicode => {
            let value: hdf5_metno::types::VarLenUnicode = attr.read_scalar()?;
            let value_string = value.to_string();
            format!("\"{value_string}\"")
        }
        hdf5_metno::types::TypeDescriptor::Reference(_) => unreachable!(),
    })
}

fn spring_attribute_array(
    attr: &hdf5_metno::Attribute,
    type_desc: TypeDescriptor,
) -> Result<String, Error> {
    match type_desc {
        TypeDescriptor::Integer(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => {
                let value = attr.read_1d::<i8>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                Ok(format!("[{values_joined}]"))
            }
            hdf5_metno::types::IntSize::U2 => {
                let value = attr.read_1d::<i16>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                Ok(format!("[{values_joined}]"))
            }
            hdf5_metno::types::IntSize::U4 => {
                let value = attr.read_1d::<i32>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                Ok(format!("[{values_joined}]"))
            }
            hdf5_metno::types::IntSize::U8 => {
                let value = attr.read_1d::<i64>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                Ok(format!("[{values_joined}]"))
            }
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => {
                let value = attr.read_1d::<u8>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                Ok(format!("[{values_joined}]"))
            }
            hdf5_metno::types::IntSize::U2 => {
                let value = attr.read_1d::<u16>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                Ok(format!("[{values_joined}]"))
            }
            hdf5_metno::types::IntSize::U4 => {
                let value = attr.read_1d::<u32>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                Ok(format!("[{values_joined}]"))
            }
            hdf5_metno::types::IntSize::U8 => {
                let value = attr.read_1d::<u64>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                Ok(format!("[{values_joined}]"))
            }
        },
        TypeDescriptor::Float(float_size) => match float_size {
            hdf5_metno::types::FloatSize::U4 => {
                let value = attr.read_1d::<f32>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                Ok(format!("[{values_joined}]"))
            }
            hdf5_metno::types::FloatSize::U8 => {
                let value = attr.read_1d::<f64>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                Ok(format!("[{values_joined}]"))
            }
        },
        TypeDescriptor::Boolean => todo!(),
        TypeDescriptor::Enum(enum_type) => todo!(),
        TypeDescriptor::Compound(compound_type) => todo!(),
        TypeDescriptor::FixedArray(type_descriptor, _) => todo!(),
        TypeDescriptor::FixedAscii(n) => match n {
            0..32 => {
                let value = attr.read_1d::<FixedAscii<32>>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .map(|v| format!("{}", v))
                    .collect::<Vec<String>>()
                    .join(", ");

                Ok(format!("[{values_joined}]"))
            }
            32..64 => {
                let value = attr.read_1d::<FixedAscii<64>>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .map(|v| format!("{}", v))
                    .collect::<Vec<String>>()
                    .join(", ");

                Ok(format!("[{values_joined}]"))
            }
            64..128 => {
                let value = attr.read_1d::<FixedAscii<128>>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .map(|v| format!("{}", v))
                    .collect::<Vec<String>>()
                    .join(", ");

                Ok(format!("[{values_joined}]"))
            }
            128..256 => {
                let value = attr.read_1d::<FixedAscii<256>>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .map(|v| format!("{}", v))
                    .collect::<Vec<String>>()
                    .join(", ");

                Ok(format!("[{values_joined}]"))
            }
            256..512 => {
                let value = attr.read_1d::<FixedAscii<512>>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .map(|v| format!("{}", v))
                    .collect::<Vec<String>>()
                    .join(", ");

                Ok(format!("[{values_joined}]"))
            }
            512..1024 => {
                let value = attr.read_1d::<FixedAscii<1024>>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .map(|v| format!("{}", v))
                    .collect::<Vec<String>>()
                    .join(", ");

                Ok(format!("[{values_joined}]"))
            }
            1024..2048 => {
                let value = attr.read_1d::<FixedAscii<2048>>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .map(|v| format!("{}", v))
                    .collect::<Vec<String>>()
                    .join(", ");

                Ok(format!("[{values_joined}]"))
            }
            2048..4096 => {
                let value = attr.read_1d::<FixedAscii<4096>>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .map(|v| format!("{}", v))
                    .collect::<Vec<String>>()
                    .join(", ");

                Ok(format!("[{values_joined}]"))
            }
            _ => {
                let value = attr.read_1d::<FixedAscii<8192>>()?;
                let values_joined = value
                    .iter()
                    .map(|v| v.to_string())
                    .map(|v| format!("{}", v))
                    .collect::<Vec<String>>()
                    .join(", ");

                Ok(format!("[{values_joined}]"))
            }
        },
        TypeDescriptor::FixedUnicode(_) => todo!(),
        TypeDescriptor::VarLenArray(type_descriptor) => todo!(),
        TypeDescriptor::VarLenAscii => todo!(),
        TypeDescriptor::VarLenUnicode => todo!(),
        TypeDescriptor::Reference(reference) => todo!(),
    }
}

pub fn sprint_attribute(attr: &hdf5_metno::Attribute) -> Result<String, Error> {
    if attr.is_valid() {
        if attr.is_scalar() {
            let attr_type = attr.dtype()?.to_descriptor()?;
            let str = sprint_attribute_scalar(attr, attr_type)?;
            Ok(str)
        } else {
            let attr_type = attr.dtype()?.to_descriptor()?;
            let str = spring_attribute_array(attr, attr_type)?;
            Ok(str)
        }
    } else {
        Ok("Invalid".to_string())
    }
}
