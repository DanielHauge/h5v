use hdf5_metno::{
    types::{
        self, FixedAscii, FixedUnicode, Reference, TypeDescriptor, VarLenArray, VarLenAscii,
        VarLenUnicode,
    },
    Attribute, Error, ObjectReference1, ObjectReference2,
};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::{
    configure,
    h5f::{ensure_attr_editable, format_opaque_bytes_for_edit, read_attr_memory_bytes},
    ui::matrix::{EnumRenderer, RenderIntercept},
};

use super::typedesc::sprint_typedescriptor;

mod descriptor;
mod raw;
mod references;
mod shared;

pub use descriptor::{attribute_type_description, attribute_type_descriptor};

use raw::{render_raw_array_values, render_raw_scalar_value, render_varlen_attr_values};
use references::{render_reference_array, render_reference_scalar};
use shared::{
    bracketed_spans, comma_separated, render_unsupported_type, render_values, render_varlen_values,
    single_span, styled_span, symbol_span, Renderable,
};

#[cfg(test)]
use descriptor::type_descriptor_for_dtype;
#[cfg(test)]
use raw::{render_value_from_bytes, render_varlen_entry, RawVarLen};

fn render_fixed_ascii_scalar(attr: &Attribute, size: usize) -> Result<Span<'static>, Error> {
    match size {
        0..32 => Ok(attr.read_scalar::<FixedAscii<32>>()?.render()),
        32..64 => Ok(attr.read_scalar::<FixedAscii<64>>()?.render()),
        64..128 => Ok(attr.read_scalar::<FixedAscii<128>>()?.render()),
        128..256 => Ok(attr.read_scalar::<FixedAscii<256>>()?.render()),
        256..512 => Ok(attr.read_scalar::<FixedAscii<512>>()?.render()),
        512..1024 => Ok(attr.read_scalar::<FixedAscii<1024>>()?.render()),
        1024..2048 => Ok(attr.read_scalar::<FixedAscii<2048>>()?.render()),
        2048..4096 => Ok(attr.read_scalar::<FixedAscii<4096>>()?.render()),
        _ => Ok(attr.read_scalar::<FixedAscii<8192>>()?.render()),
    }
}

fn render_fixed_unicode_scalar(attr: &Attribute, size: usize) -> Result<Span<'static>, Error> {
    match size {
        0..32 => Ok(attr.read_scalar::<FixedUnicode<32>>()?.render()),
        32..64 => Ok(attr.read_scalar::<FixedUnicode<64>>()?.render()),
        64..128 => Ok(attr.read_scalar::<FixedUnicode<128>>()?.render()),
        128..256 => Ok(attr.read_scalar::<FixedUnicode<256>>()?.render()),
        256..512 => Ok(attr.read_scalar::<FixedUnicode<512>>()?.render()),
        512..1024 => Ok(attr.read_scalar::<FixedUnicode<1024>>()?.render()),
        1024..2048 => Ok(attr.read_scalar::<FixedUnicode<2048>>()?.render()),
        2048..4096 => Ok(attr.read_scalar::<FixedUnicode<4096>>()?.render()),
        _ => Ok(attr.read_scalar::<FixedUnicode<8192>>()?.render()),
    }
}

fn render_fixed_ascii_array(attr: &Attribute, size: usize) -> Result<Vec<Span<'static>>, Error> {
    match size {
        0..32 => Ok(render_values(attr.read_1d::<FixedAscii<32>>()?)),
        32..64 => Ok(render_values(attr.read_1d::<FixedAscii<64>>()?)),
        64..128 => Ok(render_values(attr.read_1d::<FixedAscii<128>>()?)),
        128..256 => Ok(render_values(attr.read_1d::<FixedAscii<256>>()?)),
        256..512 => Ok(render_values(attr.read_1d::<FixedAscii<512>>()?)),
        512..1024 => Ok(render_values(attr.read_1d::<FixedAscii<1024>>()?)),
        1024..2048 => Ok(render_values(attr.read_1d::<FixedAscii<2048>>()?)),
        2048..4096 => Ok(render_values(attr.read_1d::<FixedAscii<4096>>()?)),
        _ => Ok(render_values(attr.read_1d::<FixedAscii<8192>>()?)),
    }
}

fn render_fixed_unicode_array(attr: &Attribute, size: usize) -> Result<Vec<Span<'static>>, Error> {
    match size {
        0..32 => Ok(render_values(attr.read_1d::<FixedUnicode<32>>()?)),
        32..64 => Ok(render_values(attr.read_1d::<FixedUnicode<64>>()?)),
        64..128 => Ok(render_values(attr.read_1d::<FixedUnicode<128>>()?)),
        128..256 => Ok(render_values(attr.read_1d::<FixedUnicode<256>>()?)),
        256..512 => Ok(render_values(attr.read_1d::<FixedUnicode<512>>()?)),
        512..1024 => Ok(render_values(attr.read_1d::<FixedUnicode<1024>>()?)),
        1024..2048 => Ok(render_values(attr.read_1d::<FixedUnicode<2048>>()?)),
        2048..4096 => Ok(render_values(attr.read_1d::<FixedUnicode<4096>>()?)),
        _ => Ok(render_values(attr.read_1d::<FixedUnicode<8192>>()?)),
    }
}

fn render_scalar_varlen_array(
    attr: &Attribute,
    type_desc: &TypeDescriptor,
) -> Result<Vec<Span<'static>>, Error> {
    match type_desc {
        TypeDescriptor::Integer(int_size) => match int_size {
            types::IntSize::U1 => render_varlen_values::<i8>(attr),
            types::IntSize::U2 => render_varlen_values::<i16>(attr),
            types::IntSize::U4 => render_varlen_values::<i32>(attr),
            types::IntSize::U8 => render_varlen_values::<i64>(attr),
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            types::IntSize::U1 => render_varlen_values::<u8>(attr),
            types::IntSize::U2 => render_varlen_values::<u16>(attr),
            types::IntSize::U4 => render_varlen_values::<u32>(attr),
            types::IntSize::U8 => render_varlen_values::<u64>(attr),
        },
        TypeDescriptor::Float(float_size) => match float_size {
            types::FloatSize::U4 => render_varlen_values::<f32>(attr),
            types::FloatSize::U8 => render_varlen_values::<f64>(attr),
        },
        TypeDescriptor::Boolean => render_varlen_values::<bool>(attr),
        TypeDescriptor::Enum(enum_type) => {
            let enum_renderer = EnumRenderer::new(enum_type.clone());
            let values = attr.read_scalar::<VarLenArray<u64>>()?;
            Ok(bracketed_spans(comma_separated(
                values
                    .iter()
                    .map(|value| enum_renderer.render_as_span(value)),
            )))
        }
        TypeDescriptor::Reference(Reference::Object) => {
            let refs = attr.read_scalar::<VarLenArray<ObjectReference1>>()?;
            let file = attr.file()?;
            let rendered = refs
                .iter()
                .map(|reference| {
                    file.dereference(reference)
                        .map(references::render_referenced_object)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(bracketed_spans(shared::comma_separated_groups(rendered)))
        }
        TypeDescriptor::FixedArray(inner, size) => Ok(bracketed_spans(vec![
            styled_span(
                "fixed elements",
                configure::themed_color(|colors| colors.text.type_desc),
            ),
            symbol_span(": "),
            styled_span(
                format!("[{size}]{}", sprint_typedescriptor(inner)),
                configure::themed_color(|colors| colors.text.type_desc),
            ),
        ])),
        TypeDescriptor::Reference(Reference::Region) => Ok(bracketed_spans(vec![styled_span(
            "region references",
            configure::themed_color(|colors| colors.text.type_desc),
        )])),
        TypeDescriptor::Reference(Reference::Std) => Ok(bracketed_spans(vec![styled_span(
            "standard references",
            configure::themed_color(|colors| colors.text.type_desc),
        )])),
        TypeDescriptor::Compound(_)
        | TypeDescriptor::VarLenArray(_)
        | TypeDescriptor::VarLenAscii
        | TypeDescriptor::VarLenUnicode
        | TypeDescriptor::FixedAscii(_)
        | TypeDescriptor::FixedUnicode(_) => render_varlen_attr_values(attr, type_desc, true),
    }
}

fn sprint_attribute_scalar(
    attr: &Attribute,
    type_desc: TypeDescriptor,
) -> Result<Vec<Span<'static>>, Error> {
    match type_desc {
        TypeDescriptor::Integer(int_size) => match int_size {
            types::IntSize::U1 => Ok(single_span(attr.read_scalar::<i8>()?.render())),
            types::IntSize::U2 => Ok(single_span(attr.read_scalar::<i16>()?.render())),
            types::IntSize::U4 => Ok(single_span(attr.read_scalar::<i32>()?.render())),
            types::IntSize::U8 => Ok(single_span(attr.read_scalar::<i64>()?.render())),
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            types::IntSize::U1 => Ok(single_span(attr.read_scalar::<u8>()?.render())),
            types::IntSize::U2 => Ok(single_span(attr.read_scalar::<u16>()?.render())),
            types::IntSize::U4 => Ok(single_span(attr.read_scalar::<u32>()?.render())),
            types::IntSize::U8 => Ok(single_span(attr.read_scalar::<u64>()?.render())),
        },
        TypeDescriptor::Float(float_size) => match float_size {
            types::FloatSize::U4 => Ok(single_span(attr.read_scalar::<f32>()?.render())),
            types::FloatSize::U8 => Ok(single_span(attr.read_scalar::<f64>()?.render())),
        },
        TypeDescriptor::Boolean => Ok(single_span(attr.read_scalar::<bool>()?.render())),
        TypeDescriptor::Enum(enum_type) => {
            let enum_renderer = EnumRenderer::new(enum_type);
            let value = attr.read_scalar::<u64>()?;
            Ok(single_span(enum_renderer.render_as_span(&value)))
        }
        TypeDescriptor::FixedAscii(size) => Ok(single_span(render_fixed_ascii_scalar(attr, size)?)),
        TypeDescriptor::FixedUnicode(size) => {
            Ok(single_span(render_fixed_unicode_scalar(attr, size)?))
        }
        TypeDescriptor::VarLenAscii => Ok(single_span(attr.read_scalar::<VarLenAscii>()?.render())),
        TypeDescriptor::VarLenUnicode => {
            Ok(single_span(attr.read_scalar::<VarLenUnicode>()?.render()))
        }
        TypeDescriptor::Reference(Reference::Object) => {
            render_reference_scalar::<ObjectReference1>(attr)
        }
        TypeDescriptor::Reference(Reference::Region) => Ok(single_span(styled_span(
            "region reference",
            configure::themed_color(|colors| colors.text.type_desc),
        ))),
        TypeDescriptor::Reference(Reference::Std) => {
            render_reference_scalar::<ObjectReference2>(attr)
        }
        TypeDescriptor::VarLenArray(type_desc) => {
            render_scalar_varlen_array(attr, type_desc.as_ref())
        }
        TypeDescriptor::Compound(compound_type) => {
            render_raw_scalar_value(attr, &TypeDescriptor::Compound(compound_type))
        }
        TypeDescriptor::FixedArray(type_desc, size) => {
            render_raw_scalar_value(attr, &TypeDescriptor::FixedArray(type_desc, size))
        }
    }
}

fn render_opaque_scalar(attr: &Attribute) -> Result<Line<'static>, Error> {
    let bytes = read_attr_memory_bytes(attr).map_err(|error| Error::from(error.to_string()))?;
    Ok(Line::from(Span::styled(
        format_opaque_bytes_for_edit(&bytes),
        Style::default().fg(configure::themed_color(|colors| colors.text.opaque)),
    )))
}

fn render_opaque_array(attr: &Attribute) -> Result<Line<'static>, Error> {
    let dtype = attr.dtype()?;
    let item_size = dtype.size();
    let bytes = read_attr_memory_bytes(attr).map_err(|error| Error::from(error.to_string()))?;
    let spans = if item_size == 0 {
        vec![Span::styled(
            "<zero-sized opaque values>",
            Style::default().fg(configure::themed_color(|colors| colors.text.opaque)),
        )]
    } else {
        shared::comma_separated_groups(
            bytes
                .chunks_exact(item_size)
                .map(|chunk| {
                    vec![Span::styled(
                        format_opaque_bytes_for_edit(chunk),
                        Style::default().fg(configure::themed_color(|colors| colors.text.opaque)),
                    )]
                })
                .collect::<Vec<_>>(),
        )
    };
    Ok(Line::from(bracketed_spans(spans)))
}

fn sprint_attribute_array(
    attr: &Attribute,
    type_desc: TypeDescriptor,
) -> Result<Vec<Span<'static>>, Error> {
    match type_desc {
        TypeDescriptor::Integer(int_size) => match int_size {
            types::IntSize::U1 => Ok(render_values(attr.read_1d::<i8>()?)),
            types::IntSize::U2 => Ok(render_values(attr.read_1d::<i16>()?)),
            types::IntSize::U4 => Ok(render_values(attr.read_1d::<i32>()?)),
            types::IntSize::U8 => Ok(render_values(attr.read_1d::<i64>()?)),
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            types::IntSize::U1 => Ok(render_values(attr.read_1d::<u8>()?)),
            types::IntSize::U2 => Ok(render_values(attr.read_1d::<u16>()?)),
            types::IntSize::U4 => Ok(render_values(attr.read_1d::<u32>()?)),
            types::IntSize::U8 => Ok(render_values(attr.read_1d::<u64>()?)),
        },
        TypeDescriptor::Float(float_size) => match float_size {
            types::FloatSize::U4 => Ok(render_values(attr.read_1d::<f32>()?)),
            types::FloatSize::U8 => Ok(render_values(attr.read_1d::<f64>()?)),
        },
        TypeDescriptor::FixedAscii(size) => render_fixed_ascii_array(attr, size),
        TypeDescriptor::Boolean => Ok(render_values(attr.read_1d::<bool>()?)),
        TypeDescriptor::Enum(enum_type) => {
            let enum_renderer = EnumRenderer::new(enum_type);
            Ok(comma_separated(
                attr.read_1d::<u64>()?
                    .into_iter()
                    .map(|value| enum_renderer.render_as_span(&value)),
            ))
        }
        TypeDescriptor::Compound(compound_type) => {
            render_raw_array_values(attr, &TypeDescriptor::Compound(compound_type))
        }
        TypeDescriptor::FixedArray(type_desc, size) => {
            render_raw_array_values(attr, &TypeDescriptor::FixedArray(type_desc, size))
        }
        TypeDescriptor::FixedUnicode(size) => render_fixed_unicode_array(attr, size),
        TypeDescriptor::VarLenArray(type_desc) => {
            render_varlen_attr_values(attr, type_desc.as_ref(), false)
        }
        TypeDescriptor::VarLenAscii => Ok(render_values(attr.read_1d::<VarLenAscii>()?)),
        TypeDescriptor::VarLenUnicode => Ok(render_values(attr.read_1d::<VarLenUnicode>()?)),
        TypeDescriptor::Reference(Reference::Object) => {
            render_reference_array::<ObjectReference1>(attr)
        }
        TypeDescriptor::Reference(Reference::Std) => {
            render_reference_array::<ObjectReference2>(attr)
        }
        TypeDescriptor::Reference(Reference::Region) => {
            Ok(vec![render_unsupported_type("region reference array")])
        }
    }
}

pub fn sprint_attribute(attr: &Attribute) -> Result<Line<'static>, Error> {
    if !attr.is_valid() {
        return Ok(Line::from("Invalid attribute")
            .style(configure::themed_color(|colors| colors.text.error)));
    }

    let attr_type = match attribute_type_descriptor(attr) {
        Ok(attr_type) => attr_type,
        Err(error) if error.to_string() == "Unsupported datatype class" => {
            return if attr.is_scalar() {
                render_opaque_scalar(attr)
            } else {
                render_opaque_array(attr)
            };
        }
        Err(error) => return Err(error),
    };

    if attr.is_scalar() {
        Ok(Line::from(sprint_attribute_scalar(attr, attr_type)?))
    } else {
        Ok(Line::from(bracketed_spans(sprint_attribute_array(
            attr, attr_type,
        )?)))
    }
}

pub trait AttributeEditable {
    fn can_edit(&self) -> Result<(), String>;
}

impl AttributeEditable for Attribute {
    fn can_edit(&self) -> Result<(), String> {
        ensure_attr_editable(self).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use hdf5_metno::{
        h5check,
        types::{CompoundField, CompoundType, FloatSize, IntSize, TypeDescriptor},
        Datatype, File,
    };
    use hdf5_metno_sys::h5a::H5Awrite;

    use super::{
        render_value_from_bytes, render_varlen_entry, sprint_attribute, type_descriptor_for_dtype,
        RawVarLen,
    };

    #[test]
    fn renders_scalar_compound_attribute() {
        let compound = CompoundType {
            fields: vec![
                CompoundField::new("field1", TypeDescriptor::Integer(IntSize::U4), 0, 0),
                CompoundField::new("field2", TypeDescriptor::Float(FloatSize::U8), 8, 1),
            ],
            size: 16,
        };

        let mut bytes = vec![0_u8; 16];
        bytes[0..4].copy_from_slice(&7_i32.to_le_bytes());
        bytes[8..16].copy_from_slice(&9.81_f64.to_le_bytes());

        let rendered = ratatui::text::Line::from(
            render_value_from_bytes(&bytes, &TypeDescriptor::Compound(compound))
                .expect("failed rendering scalar compound bytes"),
        )
        .to_string();
        assert_eq!(rendered, "{field1: 7, field2: 9.81}");
    }

    #[test]
    fn renders_compound_attribute_with_fixed_array_field() {
        let compound = CompoundType {
            fields: vec![
                CompoundField::new("name", TypeDescriptor::FixedAscii(8), 0, 0),
                CompoundField::new(
                    "samples",
                    TypeDescriptor::FixedArray(Box::new(TypeDescriptor::Integer(IntSize::U2)), 3),
                    8,
                    1,
                ),
            ],
            size: 14,
        };

        let mut bytes = vec![0_u8; 14];
        bytes[0..6].copy_from_slice(b"triple");
        bytes[8..10].copy_from_slice(&4_i16.to_le_bytes());
        bytes[10..12].copy_from_slice(&5_i16.to_le_bytes());
        bytes[12..14].copy_from_slice(&6_i16.to_le_bytes());

        let rendered = ratatui::text::Line::from(
            render_value_from_bytes(&bytes, &TypeDescriptor::Compound(compound))
                .expect("failed rendering fixed-array compound bytes"),
        )
        .to_string();
        assert_eq!(rendered, "{name: \"triple\", samples: [4, 5, 6]}");
    }

    #[test]
    #[ignore = "flaky low-level H5Aread in test environment"]
    fn renders_real_scalar_compound_attribute() {
        let _guard = crate::test_support::hdf5_test_guard();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("h5v-compound-attr-{unique}.h5"));
        let compound = CompoundType {
            fields: vec![
                CompoundField::new("field1", TypeDescriptor::Integer(IntSize::U4), 0, 0),
                CompoundField::new("field2", TypeDescriptor::Float(FloatSize::U8), 8, 1),
            ],
            size: 16,
        };

        let mut bytes = [0_u8; 16];
        bytes[0..4].copy_from_slice(&7_i32.to_le_bytes());
        bytes[8..16].copy_from_slice(&9.81_f64.to_le_bytes());

        let file = File::create(&path).expect("failed creating temp hdf5 file");
        let attr = file
            .new_attr_builder()
            .empty_as(&TypeDescriptor::Compound(compound))
            .create("compound")
            .expect("failed creating compound attr");
        let dtype = attr.dtype().expect("failed getting attr dtype");
        h5check(unsafe { H5Awrite(attr.id(), dtype.id(), bytes.as_ptr().cast()) })
            .expect("failed writing compound attr bytes");
        file.flush().expect("failed flushing temp hdf5 file");
        drop(dtype);
        drop(attr);
        file.close().expect("failed closing temp hdf5 file");

        let reopened = File::open(&path).expect("failed reopening temp hdf5 file");
        let attr = reopened
            .attr("compound")
            .expect("failed reopening compound attr");
        let rendered = sprint_attribute(&attr)
            .expect("failed rendering compound attr")
            .to_string();
        assert_eq!(rendered, "{field1: 7, field2: 9.81}");

        drop(attr);
        reopened
            .close()
            .expect("failed closing reopened temp hdf5 file");
        std::fs::remove_file(path).expect("failed removing temp hdf5 file");
    }

    #[test]
    fn resolves_reference_dtype_descriptor() {
        let dtype = Datatype::from_descriptor(&TypeDescriptor::Reference(
            hdf5_metno::types::Reference::Object,
        ))
        .expect("failed creating reference dtype");
        let descriptor =
            type_descriptor_for_dtype(&dtype).expect("failed resolving reference dtype");
        assert_eq!(
            descriptor,
            TypeDescriptor::Reference(hdf5_metno::types::Reference::Object)
        );
    }

    #[test]
    fn renders_varlen_compound_entry() {
        let _guard = crate::test_support::hdf5_test_guard();
        let compound = CompoundType {
            fields: vec![
                CompoundField::new("field1", TypeDescriptor::Integer(IntSize::U4), 0, 0),
                CompoundField::new("field2", TypeDescriptor::Float(FloatSize::U8), 8, 1),
            ],
            size: 16,
        };
        let mut bytes = [0_u8; 16];
        bytes[0..4].copy_from_slice(&7_i32.to_le_bytes());
        bytes[8..16].copy_from_slice(&9.81_f64.to_le_bytes());
        let entry = RawVarLen {
            len: 1,
            ptr: bytes.as_ptr().cast(),
        };

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("h5v-varlen-compound-entry-{unique}.h5"));
        let file = File::create(&path).expect("failed creating temp hdf5 file");
        let rendered = ratatui::text::Line::from(
            render_varlen_entry(&file, entry, &TypeDescriptor::Compound(compound))
                .expect("failed rendering varlen compound entry"),
        )
        .to_string();
        assert_eq!(rendered, "[{field1: 7, field2: 9.81}]");
        file.close().expect("failed closing temp hdf5 file");
        std::fs::remove_file(path).expect("failed removing temp hdf5 file");
    }
}
