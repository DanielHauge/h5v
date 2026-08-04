use hdf5_metno::{
    types::{CompoundField, CompoundType, Reference, TypeDescriptor, VarLenAscii, VarLenUnicode},
    Attribute, Dataset,
};

use crate::h5f::{Encoding, ImageType, InterlaceMode};

use super::attributes::attribute_type_descriptor;

const MAX_IMAGE_METADATA_BYTES: usize = 256;

pub fn type_contains_varlen(type_desc: &TypeDescriptor) -> bool {
    match type_desc {
        TypeDescriptor::VarLenAscii
        | TypeDescriptor::VarLenUnicode
        | TypeDescriptor::VarLenArray(_) => true,
        TypeDescriptor::Compound(compound) => compound
            .fields
            .iter()
            .any(|field| type_contains_varlen(&field.ty)),
        TypeDescriptor::FixedArray(inner, _) => type_contains_varlen(inner),
        _ => false,
    }
}

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
        TypeDescriptor::Enum(x) => {
            let base_type = x.base_type();
            format!("enum({base_type})[{}]", x.members.len())
        }
        TypeDescriptor::Compound(c) => {
            let field_strings: Vec<String> = c
                .fields
                .iter()
                .map(|field| field.name.to_string())
                .collect();
            format!("{{{}}}", field_strings.join(", "))
        }
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

const MAX_COMPOUND_SCHEMA_DEPTH: usize = 32;

fn push_schema_line(out: &mut String, indent: usize, line: &str) {
    out.push_str(&" ".repeat(indent));
    out.push_str(line);
    out.push('\n');
}

fn schema_type_label(type_desc: &TypeDescriptor) -> String {
    match type_desc {
        TypeDescriptor::Compound(_) => "compound".to_string(),
        TypeDescriptor::FixedArray(inner, len) => format!("[{len}]{}", schema_type_label(inner)),
        TypeDescriptor::VarLenArray(inner) => format!("[]{}", schema_type_label(inner)),
        _ => sprint_typedescriptor(type_desc),
    }
}

fn render_schema_type(
    type_desc: &TypeDescriptor,
    indent: usize,
    active_compounds: &mut Vec<usize>,
    depth: usize,
    out: &mut String,
) {
    match type_desc {
        TypeDescriptor::Compound(compound) => {
            render_schema_compound(compound, indent, active_compounds, depth, out)
        }
        _ => push_schema_line(out, indent, &schema_type_label(type_desc)),
    }
}

fn render_schema_collection_element(
    inner: &TypeDescriptor,
    indent: usize,
    active_compounds: &mut Vec<usize>,
    depth: usize,
    out: &mut String,
) {
    if matches!(
        inner,
        TypeDescriptor::Compound(_)
            | TypeDescriptor::FixedArray(_, _)
            | TypeDescriptor::VarLenArray(_)
    ) {
        push_schema_line(out, indent, "element:");
        render_schema_type(inner, indent + 2, active_compounds, depth, out);
    }
}

fn render_schema_field(
    field: &CompoundField,
    indent: usize,
    active_compounds: &mut Vec<usize>,
    depth: usize,
    out: &mut String,
) {
    push_schema_line(
        out,
        indent,
        &format!(
            "{}: {} @{}",
            field.name,
            schema_type_label(&field.ty),
            field.offset
        ),
    );
    match &field.ty {
        TypeDescriptor::Compound(compound) => {
            render_schema_compound(compound, indent + 2, active_compounds, depth, out)
        }
        TypeDescriptor::FixedArray(inner, _) | TypeDescriptor::VarLenArray(inner) => {
            render_schema_collection_element(inner, indent + 2, active_compounds, depth, out)
        }
        _ => {}
    }
}

fn render_schema_compound(
    compound: &CompoundType,
    indent: usize,
    active_compounds: &mut Vec<usize>,
    depth: usize,
    out: &mut String,
) {
    if depth >= MAX_COMPOUND_SCHEMA_DEPTH {
        push_schema_line(out, indent, "<compound nesting limit reached>");
        return;
    }

    let ptr = compound as *const CompoundType as usize;
    if active_compounds.contains(&ptr) {
        push_schema_line(out, indent, "<recursive compound omitted>");
        return;
    }

    push_schema_line(out, indent, "compound");
    if compound.fields.is_empty() {
        push_schema_line(out, indent + 2, "<empty>");
        return;
    }

    active_compounds.push(ptr);
    for field in &compound.fields {
        render_schema_field(field, indent + 2, active_compounds, depth + 1, out);
    }
    active_compounds.pop();
}

pub fn sprint_type_schema(type_desc: &TypeDescriptor) -> String {
    let mut out = String::new();
    let mut active_compounds = Vec::new();
    render_schema_type(type_desc, 0, &mut active_compounds, 0, &mut out);
    out.trim_end().to_string()
}

// https://support.hdfgroup.org/documentation/hdf5/latest/_i_m_g.html
fn read_image_metadata(attr: &Attribute) -> Option<String> {
    if !attr.is_scalar() {
        return None;
    }

    let value = match attribute_type_descriptor(attr).ok()? {
        TypeDescriptor::VarLenAscii => attr.read_scalar::<VarLenAscii>().ok()?.as_str().to_owned(),
        TypeDescriptor::VarLenUnicode => attr
            .read_scalar::<VarLenUnicode>()
            .ok()?
            .as_str()
            .to_owned(),
        _ => return None,
    };
    (value.len() <= MAX_IMAGE_METADATA_BYTES).then_some(value)
}

pub fn is_image(d: &Dataset) -> Option<ImageType> {
    let class = read_image_metadata(&d.attr("CLASS").ok()?)?;
    if class != "IMAGE" {
        return None;
    }

    let read_image_subclass = read_image_metadata(&d.attr("IMAGE_SUBCLASS").ok()?)?;
    let interlace_mode_read = d
        .attr("INTERLACE_MODE")
        .ok()
        .and_then(|attr| read_image_metadata(&attr));

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
        TypeDescriptor::Enum(_) => Some(MatrixRenderType::Enum),
        TypeDescriptor::Compound(_) => Some(MatrixRenderType::Compound),
        TypeDescriptor::FixedArray(_, _) => None,
        TypeDescriptor::FixedAscii(_) => Some(MatrixRenderType::Strings),
        TypeDescriptor::FixedUnicode(_) => Some(MatrixRenderType::Strings),
        TypeDescriptor::VarLenArray(_) if is_varlen_byte_array(type_desc) => {
            Some(MatrixRenderType::ByteArray)
        }
        TypeDescriptor::VarLenArray(_) => None,
        TypeDescriptor::VarLenAscii => Some(MatrixRenderType::Strings),
        TypeDescriptor::VarLenUnicode => Some(MatrixRenderType::Strings),
        TypeDescriptor::Reference(_) => None,
    }
}

pub fn is_varlen_byte_array(type_desc: &TypeDescriptor) -> bool {
    matches!(
        type_desc,
        TypeDescriptor::VarLenArray(inner)
            if matches!(inner.as_ref(), TypeDescriptor::Unsigned(hdf5_metno::types::IntSize::U1))
    )
}
pub fn encoding_from_dtype(dtype: &TypeDescriptor) -> Encoding {
    match dtype {
        TypeDescriptor::Integer(_) => Encoding::LittleEndian,
        TypeDescriptor::Unsigned(_) => Encoding::LittleEndian,
        TypeDescriptor::Float(_) => Encoding::LittleEndian,
        TypeDescriptor::Boolean => Encoding::LittleEndian,
        TypeDescriptor::Enum(_) => Encoding::UTF8,
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
    Opaque,
    Uint64,
    Int64,
    Compound,
    Strings,
    Enum,
    ByteArray,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use hdf5_metno::{
        types::{CompoundField, FloatSize, IntSize, VarLenUnicode},
        File,
    };
    use std::str::FromStr;

    #[test]
    fn detects_varlen_members_nested_in_compounds_and_fixed_arrays() {
        let type_desc = TypeDescriptor::Compound(CompoundType {
            fields: vec![CompoundField::new(
                "rows",
                TypeDescriptor::FixedArray(
                    Box::new(TypeDescriptor::Compound(CompoundType {
                        fields: vec![CompoundField::new(
                            "label",
                            TypeDescriptor::VarLenUnicode,
                            0,
                            0,
                        )],
                        size: std::mem::size_of::<usize>(),
                    })),
                    2,
                ),
                0,
                0,
            )],
            size: 2 * std::mem::size_of::<usize>(),
        });

        assert!(type_contains_varlen(&type_desc));
    }

    #[test]
    fn detects_scalar_varlen_image_metadata() {
        let _guard = crate::test_support::hdf5_test_guard();
        let file = File::create("typedesc-vlen-image-metadata.h5").expect("create file");
        let dataset = file
            .new_dataset::<u8>()
            .shape([1])
            .create("image")
            .expect("create dataset");
        for (name, value) in [
            ("CLASS", "IMAGE"),
            ("IMAGE_SUBCLASS", "IMAGE_TRUECOLOR"),
            ("INTERLACE_MODE", "INTERLACE_PIXEL"),
        ] {
            dataset
                .new_attr::<VarLenUnicode>()
                .create(name)
                .expect("create metadata attribute")
                .write_scalar(&VarLenUnicode::from_str(value).expect("valid metadata"))
                .expect("write metadata attribute");
        }

        assert!(matches!(
            is_image(&dataset),
            Some(ImageType::Truecolor(InterlaceMode::Pixel))
        ));
        drop(dataset);
        file.close().expect("close file");
        std::fs::remove_file("typedesc-vlen-image-metadata.h5").expect("remove file");
    }

    #[test]
    fn renders_nested_compound_schema() {
        let type_desc = TypeDescriptor::Compound(CompoundType {
            fields: vec![
                CompoundField::new("id", TypeDescriptor::Unsigned(IntSize::U4), 0, 0),
                CompoundField::new(
                    "pos",
                    TypeDescriptor::Compound(CompoundType {
                        fields: vec![
                            CompoundField::new("x", TypeDescriptor::Float(FloatSize::U8), 0, 0),
                            CompoundField::new("y", TypeDescriptor::Float(FloatSize::U8), 8, 1),
                        ],
                        size: 16,
                    }),
                    8,
                    1,
                ),
            ],
            size: 24,
        });

        assert_eq!(
            sprint_type_schema(&type_desc),
            "compound\n  id: u32 @0\n  pos: compound @8\n    compound\n      x: f64 @0\n      y: f64 @8"
        );
    }

    #[test]
    fn renders_compound_array_schema() {
        let type_desc = TypeDescriptor::Compound(CompoundType {
            fields: vec![CompoundField::new(
                "samples",
                TypeDescriptor::FixedArray(
                    Box::new(TypeDescriptor::Compound(CompoundType {
                        fields: vec![CompoundField::new(
                            "value",
                            TypeDescriptor::Integer(IntSize::U2),
                            0,
                            0,
                        )],
                        size: 2,
                    })),
                    3,
                ),
                0,
                0,
            )],
            size: 6,
        });

        assert_eq!(
            sprint_type_schema(&type_desc),
            "compound\n  samples: [3]compound @0\n    element:\n      compound\n        value: i16 @0"
        );
    }

    #[test]
    fn compound_schema_depth_limit_stops_unbounded_nesting() {
        let mut type_desc = TypeDescriptor::Integer(IntSize::U1);
        for depth in (0..40).rev() {
            let field_name = format!("level_{depth}");
            type_desc = TypeDescriptor::Compound(CompoundType {
                fields: vec![CompoundField::new(&field_name, type_desc, 0, 0)],
                size: 1,
            });
        }

        let rendered = sprint_type_schema(&type_desc);
        assert!(rendered.contains("<compound nesting limit reached>"));
    }
}
