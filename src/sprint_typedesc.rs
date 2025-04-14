use hdf5_metno::types::TypeDescriptor;

use crate::h5f::Encoding;

pub fn sprint_typedescriptor(type_desc: &TypeDescriptor) -> String {
    match type_desc {
        TypeDescriptor::Integer(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => "i8".to_string(),
            hdf5_metno::types::IntSize::U2 => "i16".to_string(),
            hdf5_metno::types::IntSize::U4 => "i32".to_string(),
            hdf5_metno::types::IntSize::U8 => "i64".to_string(),
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            hdf5_metno::types::IntSize::U1 => "u8".to_string(),
            hdf5_metno::types::IntSize::U2 => "u16".to_string(),
            hdf5_metno::types::IntSize::U4 => "u32".to_string(),
            hdf5_metno::types::IntSize::U8 => "u64".to_string(),
        },
        TypeDescriptor::Float(float_size) => match float_size {
            hdf5_metno::types::FloatSize::U4 => "f32".to_string(),
            hdf5_metno::types::FloatSize::U8 => "f64".to_string(),
        },
        TypeDescriptor::Boolean => "bool".to_string(),
        TypeDescriptor::Enum(_) => "enum".to_string(),
        TypeDescriptor::Compound(_) => "compound".to_string(),
        TypeDescriptor::FixedArray(_, _) => "array".to_string(),
        TypeDescriptor::FixedAscii(_) => "ascii".to_string(),
        TypeDescriptor::FixedUnicode(_) => "unicode".to_string(),
        TypeDescriptor::VarLenArray(_) => "array".to_string(),
        TypeDescriptor::VarLenAscii => "ascii".to_string(),
        TypeDescriptor::VarLenUnicode => "unicode".to_string(),
        TypeDescriptor::Reference(_) => "reference".to_string(),
    }
}

pub fn is_type_numerical(type_desc: &TypeDescriptor) -> bool {
    match type_desc {
        TypeDescriptor::Integer(_) => true,
        TypeDescriptor::Unsigned(_) => true,
        TypeDescriptor::Float(_) => true,
        TypeDescriptor::Boolean => false,
        TypeDescriptor::Enum(_) => false,
        TypeDescriptor::Compound(_) => false,
        TypeDescriptor::FixedArray(_, _) => false,
        TypeDescriptor::FixedAscii(_) => false,
        TypeDescriptor::FixedUnicode(_) => false,
        TypeDescriptor::VarLenArray(_) => false,
        TypeDescriptor::VarLenAscii => false,
        TypeDescriptor::VarLenUnicode => false,
        TypeDescriptor::Reference(_) => false,
    }
}
pub fn encoding_from_dtype(dtype: &TypeDescriptor) -> Encoding {
    match dtype {
        TypeDescriptor::Integer(_) => Encoding::LittleEndian,
        TypeDescriptor::Unsigned(_) => Encoding::LittleEndian,
        TypeDescriptor::Float(_) => Encoding::LittleEndian,
        TypeDescriptor::Boolean => Encoding::LittleEndian,
        TypeDescriptor::Enum(_) => Encoding::Unknown,
        TypeDescriptor::Compound(_) => Encoding::Unknown,
        TypeDescriptor::FixedArray(_, _) => Encoding::Unknown,
        TypeDescriptor::FixedAscii(_) => Encoding::ASCII,
        TypeDescriptor::FixedUnicode(_) => Encoding::UTF8,
        TypeDescriptor::VarLenArray(_) => Encoding::Unknown,
        TypeDescriptor::VarLenAscii => Encoding::ASCII,
        TypeDescriptor::VarLenUnicode => Encoding::UTF8,
        TypeDescriptor::Reference(_) => Encoding::Unknown,
    }
}
