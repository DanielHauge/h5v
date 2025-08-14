use hdf5_metno::{
    types::{Reference, TypeDescriptor},
    Dataset,
};

use crate::{
    h5f::{Encoding, ImageType, InterlaceMode},
    sprint_attributes::sprint_attribute,
};

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
        TypeDescriptor::FixedArray(inner, l) => {
            format!("[{l}]{}", sprint_typedescriptor(inner))
        }
        TypeDescriptor::FixedAscii(l) => format!("[{l}]char (ascii)"),
        TypeDescriptor::FixedUnicode(l) => format!("[{l}]char (utf)"),
        TypeDescriptor::VarLenArray(inner) => {
            format!("[]{}", sprint_typedescriptor(inner))
        }
        TypeDescriptor::VarLenAscii => "[]char (ascii)".to_string(),
        TypeDescriptor::VarLenUnicode => "[]char (utf)".to_string(),
        TypeDescriptor::Reference(Reference::Object) => "object-reference".to_string(),
        TypeDescriptor::Reference(Reference::Region) => "region-reference".to_string(),
        TypeDescriptor::Reference(Reference::Std) => "std-reference".to_string(),
    }
}

// https://support.hdfgroup.org/documentation/hdf5/latest/_i_m_g.html
pub fn is_image(d: &Dataset) -> Option<ImageType> {
    let class = match d.attr("CLASS") {
        Ok(class) => class,
        Err(_) => return None,
    };

    match sprint_attribute(&class) {
        Ok(class) => {
            let class_string = class.to_string().replace("\"", "");
            if class_string != "IMAGE" {
                return None;
            }
        }
        Err(_) => return None,
    }

    let image_subclass = match d.attr("IMAGE_SUBCLASS") {
        Ok(image_subclass) => image_subclass,
        Err(_) => return None,
    };

    let read_image_subclass = match sprint_attribute(&image_subclass) {
        Ok(read_image_subclass) => read_image_subclass.to_string().replace("\"", ""),
        Err(_) => return None,
    };

    let interlace_mode = d.attr("INTERLACE_MODE").ok();

    let interlace_mode_read = match interlace_mode {
        Some(interlace_mode) => match sprint_attribute(&interlace_mode) {
            Ok(interlace_mode_read) => Some(interlace_mode_read.to_string().replace("\"", "")),
            Err(_) => None,
        },
        None => None,
    };

    let interlace_node_parsed = match interlace_mode_read {
        Some(interlace) => match interlace.as_str() {
            "INTERLACE_PIXEL" => Some(InterlaceMode::Pixel),
            "INTERLACE_PLANE" => Some(InterlaceMode::Plane),
            _ => None,
        },
        None => None,
    };

    match read_image_subclass.as_str() {
        "IMAGE_GRAYSCALE" => Some(ImageType::Grayscale),
        "IMAGE_TRUECOLOR" => Some(ImageType::Truecolor(interlace_node_parsed?)),
        "IMAGE_BITMAP" => Some(ImageType::Bitmap),
        "IMAGE_INDEXED" => Some(ImageType::Indexed(interlace_node_parsed?)),
        "IMAGE_JPEG" => Some(ImageType::Jpeg),
        "IMAGE_PNG" => Some(ImageType::Png),
        _ => None,
    }
}

pub fn is_type_matrixable(type_desc: &TypeDescriptor) -> Option<MatrixRenderType> {
    match type_desc {
        TypeDescriptor::Integer(_) => Some(MatrixRenderType::Int64),
        TypeDescriptor::Unsigned(_) => Some(MatrixRenderType::Uint64),
        TypeDescriptor::Float(_) => Some(MatrixRenderType::Float64),
        TypeDescriptor::Boolean => Some(MatrixRenderType::Uint64),
        TypeDescriptor::Enum(_) => None,
        TypeDescriptor::Compound(_) => Some(MatrixRenderType::Compound),
        TypeDescriptor::FixedArray(_, _) => None,
        TypeDescriptor::FixedAscii(_) => Some(MatrixRenderType::Strings),
        TypeDescriptor::FixedUnicode(_) => Some(MatrixRenderType::Strings),
        TypeDescriptor::VarLenArray(_) => None,
        TypeDescriptor::VarLenAscii => Some(MatrixRenderType::Strings),
        TypeDescriptor::VarLenUnicode => Some(MatrixRenderType::Strings),
        TypeDescriptor::Reference(_) => None,
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
        TypeDescriptor::FixedAscii(_) => Encoding::AsciiFixed,
        TypeDescriptor::FixedUnicode(_) => Encoding::UTF8Fixed,
        TypeDescriptor::VarLenArray(_) => Encoding::Unknown,
        TypeDescriptor::VarLenAscii => Encoding::Ascii,
        TypeDescriptor::VarLenUnicode => Encoding::UTF8,
        TypeDescriptor::Reference(_) => Encoding::Unknown,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum MatrixRenderType {
    Float64,
    Uint64,
    Int64,
    Compound,
    Strings,
}
