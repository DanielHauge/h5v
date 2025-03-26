use hdf5_metno::types::TypeDescriptor;

pub fn sprint_typedescriptor(type_desc: TypeDescriptor) -> String {
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
        TypeDescriptor::Enum(enum_type) => "enum".to_string(),
        TypeDescriptor::Compound(compound_type) => "compound".to_string(),
        TypeDescriptor::FixedArray(type_descriptor, _) => "array".to_string(),
        TypeDescriptor::FixedAscii(_) => "ascii".to_string(),
        TypeDescriptor::FixedUnicode(_) => "unicode".to_string(),
        TypeDescriptor::VarLenArray(type_descriptor) => "array".to_string(),
        TypeDescriptor::VarLenAscii => "ascii".to_string(),
        TypeDescriptor::VarLenUnicode => "unicode".to_string(),
        TypeDescriptor::Reference(reference) => "reference".to_string(),
    }
}
