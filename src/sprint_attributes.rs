use hdf5_metno::{
    types::{
        self, CompoundType, FixedAscii, FixedUnicode, Reference, TypeDescriptor, VarLenArray,
        VarLenAscii, VarLenUnicode,
    },
    Attribute, Error, H5Type, ObjectReference, ObjectReference1, ObjectReference2,
    ReferencedObject,
};
use ratatui::{text::Line, text::Span};

use crate::{
    color_consts,
    h5f::scalar_text_codec,
    sprint_typedesc::sprint_typedescriptor,
    ui::matrix::{EnumRenderer, RenderIntercept},
};

pub trait Renderable {
    fn render(self) -> Span<'static>;
}

fn styled_span(value: impl std::fmt::Display, color: ratatui::style::Color) -> Span<'static> {
    Span::from(value.to_string()).style(color)
}

fn string_span(value: impl std::fmt::Display) -> Span<'static> {
    styled_span(format_args!("\"{value}\""), color_consts::STRING_COLOR)
}

fn symbol_span(value: &'static str) -> Span<'static> {
    Span::raw(value).style(color_consts::SYMBOL_COLOR)
}

fn comma_separated<I>(spans: I) -> Vec<Span<'static>>
where
    I: IntoIterator<Item = Span<'static>>,
{
    itertools::intersperse(spans, symbol_span(", ")).collect()
}

fn comma_separated_groups<I>(groups: I) -> Vec<Span<'static>>
where
    I: IntoIterator<Item = Vec<Span<'static>>>,
{
    let mut out = Vec::new();
    for group in groups {
        if !out.is_empty() {
            out.push(symbol_span(", "));
        }
        out.extend(group);
    }
    out
}

fn bracketed_spans(spans: Vec<Span<'static>>) -> Vec<Span<'static>> {
    std::iter::once(symbol_span("["))
        .chain(spans)
        .chain(std::iter::once(symbol_span("]")))
        .collect()
}

fn braced_spans(spans: Vec<Span<'static>>) -> Vec<Span<'static>> {
    std::iter::once(symbol_span("{"))
        .chain(spans)
        .chain(std::iter::once(symbol_span("}")))
        .collect()
}

fn single_span(span: Span<'static>) -> Vec<Span<'static>> {
    vec![span]
}

fn render_values<T>(values: impl IntoIterator<Item = T>) -> Vec<Span<'static>>
where
    T: Renderable,
{
    comma_separated(values.into_iter().map(Renderable::render))
}

fn render_varlen_values<T>(attr: &Attribute) -> Result<Vec<Span<'static>>, Error>
where
    T: Renderable + Copy + H5Type,
{
    let values = Vec::from(attr.read_scalar::<VarLenArray<T>>()?);
    Ok(bracketed_spans(render_values(values)))
}

macro_rules! impl_uint_renderable {
    ($($t:ty),*) => {
        $(
            impl Renderable for $t {
                fn render(self) -> Span<'static> {
                    styled_span(self, color_consts::UINT_COLOR)
                }
            }
        )*
    };
}
impl_uint_renderable!(u8, u16, u32, u64);

macro_rules! impl_int_renderable {
    ($($t:ty),*) => {
        $(
            impl Renderable for $t {
                fn render(self) -> Span<'static> {
                    styled_span(self, color_consts::INT_COLOR)
                }
            }
        )*
    };
}

impl_int_renderable!(i8, i16, i32, i64);

macro_rules! impl_float_renderable {
    ($($t:ty),*) => {
        $(
            impl Renderable for $t {
                fn render(self) -> Span<'static> {
                    styled_span(self, color_consts::FLOAT_COLOR)
                }
            }
        )*
    };
}
impl_float_renderable!(f32, f64);

impl Renderable for bool {
    fn render(self) -> Span<'static> {
        styled_span(self, color_consts::BOOL_COLOR)
    }
}

impl<const N: usize> Renderable for FixedAscii<N> {
    fn render(self) -> Span<'static> {
        string_span(self)
    }
}

impl<const N: usize> Renderable for FixedUnicode<N> {
    fn render(self) -> Span<'static> {
        string_span(self)
    }
}

impl Renderable for VarLenAscii {
    fn render(self) -> Span<'static> {
        string_span(self)
    }
}

impl Renderable for VarLenUnicode {
    fn render(self) -> Span<'static> {
        string_span(self)
    }
}

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

fn render_reference_target(
    kind: &'static str,
    name: String,
    color: ratatui::style::Color,
) -> Vec<Span<'static>> {
    let name = if name.is_empty() {
        "<anonymous>".to_string()
    } else {
        name
    };
    vec![
        symbol_span("&"),
        styled_span(kind, color),
        symbol_span(":"),
        styled_span(name, color),
    ]
}

fn render_referenced_object(object: ReferencedObject) -> Vec<Span<'static>> {
    match object {
        ReferencedObject::Group(group) => {
            render_reference_target("group", group.name(), color_consts::GROUP_COLOR)
        }
        ReferencedObject::Dataset(dataset) => {
            render_reference_target("dataset", dataset.name(), color_consts::DATASET_COLOR)
        }
        ReferencedObject::Datatype(datatype) => render_reference_target(
            "datatype",
            datatype
                .as_location()
                .map(|location| location.name())
                .unwrap_or_else(|_| datatype.to_string()),
            color_consts::TYPE_DESC_COLOR,
        ),
    }
}

fn render_reference_scalar<R>(attr: &Attribute) -> Result<Vec<Span<'static>>, Error>
where
    R: ObjectReference,
{
    let reference = attr.read_scalar::<R>()?;
    let file = attr.file()?;
    let object = file.dereference(&reference)?;
    Ok(render_referenced_object(object))
}

fn to_array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], Error> {
    bytes.try_into().map_err(|_| {
        Error::from(format!(
            "Failed converting {} bytes to {N}-byte array",
            bytes.len()
        ))
    })
}

fn slice_bytes<'a>(
    bytes: &'a [u8],
    offset: usize,
    size: usize,
    context: &str,
) -> Result<&'a [u8], Error> {
    let end = offset + size;
    bytes.get(offset..end).ok_or_else(|| {
        Error::from(format!(
            "{context} exceeded byte bounds (offset {offset}, size {size}, len {})",
            bytes.len()
        ))
    })
}

fn render_field_name(name: &str) -> Span<'static> {
    styled_span(name, color_consts::VARIABLE_BLUE)
}

fn render_compound_bytes(
    bytes: &[u8],
    compound_type: &CompoundType,
) -> Result<Vec<Span<'static>>, Error> {
    let rendered_fields = compound_type
        .fields
        .iter()
        .map(|field| {
            let field_bytes = slice_bytes(
                bytes,
                field.offset,
                field.ty.size(),
                &format!("Compound field {}", field.name),
            )?;
            let mut spans = vec![render_field_name(&field.name), symbol_span(": ")];
            spans.extend(render_value_from_bytes(field_bytes, &field.ty)?);
            Ok(spans)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(braced_spans(comma_separated_groups(rendered_fields)))
}

fn render_fixed_array_bytes(
    bytes: &[u8],
    inner_type: &TypeDescriptor,
    size: usize,
) -> Result<Vec<Span<'static>>, Error> {
    let inner_size = inner_type.size();
    let rendered = bytes
        .chunks_exact(inner_size)
        .take(size)
        .map(|chunk| render_value_from_bytes(chunk, inner_type))
        .collect::<Result<Vec<_>, Error>>()?;

    if bytes.len() != inner_size * size {
        return Err(Error::from(format!(
            "Fixed array byte size mismatch: expected {}, got {}",
            inner_size * size,
            bytes.len()
        )));
    }

    Ok(bracketed_spans(comma_separated_groups(rendered)))
}

fn render_value_from_bytes(
    bytes: &[u8],
    type_desc: &TypeDescriptor,
) -> Result<Vec<Span<'static>>, Error> {
    match type_desc {
        TypeDescriptor::Integer(int_size) => match int_size {
            types::IntSize::U1 => Ok(single_span(i8::from_le_bytes(to_array(bytes)?).render())),
            types::IntSize::U2 => Ok(single_span(i16::from_le_bytes(to_array(bytes)?).render())),
            types::IntSize::U4 => Ok(single_span(i32::from_le_bytes(to_array(bytes)?).render())),
            types::IntSize::U8 => Ok(single_span(i64::from_le_bytes(to_array(bytes)?).render())),
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            types::IntSize::U1 => Ok(single_span(u8::from_le_bytes(to_array(bytes)?).render())),
            types::IntSize::U2 => Ok(single_span(u16::from_le_bytes(to_array(bytes)?).render())),
            types::IntSize::U4 => Ok(single_span(u32::from_le_bytes(to_array(bytes)?).render())),
            types::IntSize::U8 => Ok(single_span(u64::from_le_bytes(to_array(bytes)?).render())),
        },
        TypeDescriptor::Float(float_size) => match float_size {
            types::FloatSize::U4 => Ok(single_span(f32::from_le_bytes(to_array(bytes)?).render())),
            types::FloatSize::U8 => Ok(single_span(f64::from_le_bytes(to_array(bytes)?).render())),
        },
        TypeDescriptor::Boolean => Ok(single_span(
            (u8::from_le_bytes(to_array(bytes)?) != 0).render(),
        )),
        TypeDescriptor::Enum(enum_type) => {
            let enum_renderer = EnumRenderer::new(enum_type.clone());
            let value = match enum_type.base_type() {
                TypeDescriptor::Integer(types::IntSize::U1) => {
                    i8::from_le_bytes(to_array(bytes)?) as u64
                }
                TypeDescriptor::Integer(types::IntSize::U2) => {
                    i16::from_le_bytes(to_array(bytes)?) as u64
                }
                TypeDescriptor::Integer(types::IntSize::U4) => {
                    i32::from_le_bytes(to_array(bytes)?) as u64
                }
                TypeDescriptor::Integer(types::IntSize::U8) => {
                    i64::from_le_bytes(to_array(bytes)?) as u64
                }
                TypeDescriptor::Unsigned(types::IntSize::U1) => {
                    u8::from_le_bytes(to_array(bytes)?) as u64
                }
                TypeDescriptor::Unsigned(types::IntSize::U2) => {
                    u16::from_le_bytes(to_array(bytes)?) as u64
                }
                TypeDescriptor::Unsigned(types::IntSize::U4) => {
                    u32::from_le_bytes(to_array(bytes)?) as u64
                }
                TypeDescriptor::Unsigned(types::IntSize::U8) => {
                    u64::from_le_bytes(to_array(bytes)?)
                }
                base => {
                    return Err(Error::from(format!(
                        "Unsupported enum base type in compound attribute: {base}"
                    )))
                }
            };
            Ok(single_span(enum_renderer.render_as_span(&value)))
        }
        TypeDescriptor::Compound(compound_type) => render_compound_bytes(bytes, compound_type),
        TypeDescriptor::FixedArray(inner_type, size) => {
            render_fixed_array_bytes(bytes, inner_type.as_ref(), *size)
        }
        TypeDescriptor::FixedAscii(_) | TypeDescriptor::FixedUnicode(_) => {
            let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
            Ok(single_span(string_span(String::from_utf8_lossy(
                &bytes[..end],
            ))))
        }
        TypeDescriptor::VarLenArray(inner) => Ok(bracketed_spans(vec![styled_span(
            format!("varlen {}", sprint_typedescriptor(inner)),
            color_consts::TYPE_DESC_COLOR,
        )])),
        TypeDescriptor::VarLenAscii | TypeDescriptor::VarLenUnicode => Ok(vec![styled_span(
            sprint_typedescriptor(type_desc),
            color_consts::TYPE_DESC_COLOR,
        )]),
        TypeDescriptor::Reference(reference_type) => Ok(vec![styled_span(
            sprint_typedescriptor(&TypeDescriptor::Reference(*reference_type)),
            color_consts::TYPE_DESC_COLOR,
        )]),
    }
}

fn render_raw_scalar_value(
    attr: &Attribute,
    type_desc: &TypeDescriptor,
) -> Result<Vec<Span<'static>>, Error> {
    let bytes: Vec<u8> = attr.read_raw()?;
    render_value_from_bytes(&bytes, type_desc)
}

fn render_raw_array_values(
    attr: &Attribute,
    element_type: &TypeDescriptor,
) -> Result<Vec<Span<'static>>, Error> {
    let bytes: Vec<u8> = attr.read_raw()?;
    let element_size = element_type.size();
    if bytes.len() != attr.size() * element_size {
        return Err(Error::from(format!(
            "Raw array byte size mismatch: expected {}, got {}",
            attr.size() * element_size,
            bytes.len()
        )));
    }

    let rendered = bytes
        .chunks_exact(element_size)
        .map(|chunk| render_value_from_bytes(chunk, element_type))
        .collect::<Result<Vec<_>, Error>>()?;

    Ok(comma_separated_groups(rendered))
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
                .map(|reference| file.dereference(reference).map(render_referenced_object))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(bracketed_spans(comma_separated_groups(rendered)))
        }
        TypeDescriptor::FixedArray(inner, size) => Ok(bracketed_spans(vec![
            styled_span("fixed elements", color_consts::TYPE_DESC_COLOR),
            symbol_span(": "),
            styled_span(
                format!("[{size}]{}", sprint_typedescriptor(inner)),
                color_consts::TYPE_DESC_COLOR,
            ),
        ])),
        TypeDescriptor::Reference(Reference::Region) => Ok(bracketed_spans(vec![styled_span(
            "region references",
            color_consts::TYPE_DESC_COLOR,
        )])),
        TypeDescriptor::Reference(Reference::Std) => Ok(bracketed_spans(vec![styled_span(
            "standard references",
            color_consts::TYPE_DESC_COLOR,
        )])),
        TypeDescriptor::Compound(_) => Ok(bracketed_spans(vec![styled_span(
            "compound values",
            color_consts::TYPE_DESC_COLOR,
        )])),
        TypeDescriptor::VarLenArray(inner) => Ok(bracketed_spans(vec![styled_span(
            format!("nested {}", sprint_typedescriptor(inner)),
            color_consts::TYPE_DESC_COLOR,
        )])),
        TypeDescriptor::VarLenAscii
        | TypeDescriptor::VarLenUnicode
        | TypeDescriptor::FixedAscii(_)
        | TypeDescriptor::FixedUnicode(_) => Ok(bracketed_spans(vec![styled_span(
            sprint_typedescriptor(type_desc),
            color_consts::TYPE_DESC_COLOR,
        )])),
    }
}

fn sprint_attribute_scalar(
    attr: &hdf5_metno::Attribute,
    type_desc: TypeDescriptor,
) -> Result<Vec<Span<'static>>, Error> {
    let val = match type_desc {
        types::TypeDescriptor::Integer(int_size) => match int_size {
            types::IntSize::U1 => single_span(attr.read_scalar::<i8>()?.render()),
            types::IntSize::U2 => single_span(attr.read_scalar::<i16>()?.render()),
            types::IntSize::U4 => single_span(attr.read_scalar::<i32>()?.render()),
            types::IntSize::U8 => single_span(attr.read_scalar::<i64>()?.render()),
        },
        types::TypeDescriptor::Unsigned(int_size) => match int_size {
            types::IntSize::U1 => single_span(attr.read_scalar::<u8>()?.render()),
            types::IntSize::U2 => single_span(attr.read_scalar::<u16>()?.render()),
            types::IntSize::U4 => single_span(attr.read_scalar::<u32>()?.render()),
            types::IntSize::U8 => single_span(attr.read_scalar::<u64>()?.render()),
        },
        types::TypeDescriptor::Float(float_size) => match float_size {
            types::FloatSize::U4 => single_span(attr.read_scalar::<f32>()?.render()),
            types::FloatSize::U8 => single_span(attr.read_scalar::<f64>()?.render()),
        },
        types::TypeDescriptor::Boolean => single_span(attr.read_scalar::<bool>()?.render()),
        types::TypeDescriptor::Enum(enum_type) => {
            let enum_renderer = EnumRenderer::new(enum_type);
            let v = attr.read_scalar::<u64>()?;
            single_span(enum_renderer.render_as_span(&v))
        }
        types::TypeDescriptor::FixedAscii(a) => single_span(render_fixed_ascii_scalar(attr, a)?),
        types::TypeDescriptor::FixedUnicode(a) => {
            single_span(render_fixed_unicode_scalar(attr, a)?)
        }
        types::TypeDescriptor::VarLenAscii => {
            single_span(attr.read_scalar::<VarLenAscii>()?.render())
        }
        types::TypeDescriptor::VarLenUnicode => {
            single_span(attr.read_scalar::<VarLenUnicode>()?.render())
        }
        types::TypeDescriptor::Reference(Reference::Object) => {
            render_reference_scalar::<ObjectReference1>(attr)?
        }
        types::TypeDescriptor::Reference(Reference::Region) => single_span(styled_span(
            "region reference",
            color_consts::TYPE_DESC_COLOR,
        )),
        types::TypeDescriptor::Reference(Reference::Std) => {
            render_reference_scalar::<ObjectReference2>(attr)?
        }
        types::TypeDescriptor::VarLenArray(type_desc) => {
            render_scalar_varlen_array(attr, type_desc.as_ref())?
        }
        types::TypeDescriptor::Compound(compound_type) => {
            render_raw_scalar_value(attr, &TypeDescriptor::Compound(compound_type))?
        }
        types::TypeDescriptor::FixedArray(type_desc, size) => {
            render_raw_scalar_value(attr, &TypeDescriptor::FixedArray(type_desc, size))?
        }
    };
    Ok(val)
}

fn render_unsupported_type(type_name: impl Into<String>) -> Span<'static> {
    let type_name = type_name.into();
    let s = format!("Unsupported type: {type_name}");
    Span::from(s).style(color_consts::ERROR_COLOR)
}

fn spring_attribute_array(
    attr: &hdf5_metno::Attribute,
    type_desc: TypeDescriptor,
) -> Result<Vec<Span<'static>>, Error> {
    let gg = match type_desc {
        TypeDescriptor::Integer(int_size) => match int_size {
            types::IntSize::U1 => render_values(attr.read_1d::<i8>()?),
            types::IntSize::U2 => render_values(attr.read_1d::<i16>()?),
            types::IntSize::U4 => render_values(attr.read_1d::<i32>()?),
            types::IntSize::U8 => render_values(attr.read_1d::<i64>()?),
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            types::IntSize::U1 => render_values(attr.read_1d::<u8>()?),
            types::IntSize::U2 => render_values(attr.read_1d::<u16>()?),
            types::IntSize::U4 => render_values(attr.read_1d::<u32>()?),
            types::IntSize::U8 => render_values(attr.read_1d::<u64>()?),
        },
        TypeDescriptor::Float(float_size) => match float_size {
            types::FloatSize::U4 => render_values(attr.read_1d::<f32>()?),
            types::FloatSize::U8 => render_values(attr.read_1d::<f64>()?),
        },
        TypeDescriptor::FixedAscii(n) => render_fixed_ascii_array(attr, n)?,
        TypeDescriptor::Boolean => render_values(attr.read_1d::<bool>()?),
        TypeDescriptor::Enum(e) => {
            let enum_renderer = EnumRenderer::new(e);
            comma_separated(
                attr.read_1d::<u64>()?
                    .into_iter()
                    .map(|v| enum_renderer.render_as_span(&v)),
            )
        }
        TypeDescriptor::Compound(compound_type) => {
            render_raw_array_values(attr, &TypeDescriptor::Compound(compound_type))?
        }
        TypeDescriptor::FixedArray(type_descriptor, size) => {
            render_raw_array_values(attr, &TypeDescriptor::FixedArray(type_descriptor, size))?
        }
        TypeDescriptor::FixedUnicode(size) => render_fixed_unicode_array(attr, size)?,
        TypeDescriptor::VarLenArray(type_descriptor) => match type_descriptor.as_ref() {
            TypeDescriptor::Integer(_)
            | TypeDescriptor::Unsigned(_)
            | TypeDescriptor::Float(_)
            | TypeDescriptor::Boolean => {
                vec![render_unsupported_type("varlen array of primitive type")]
            }
            TypeDescriptor::Enum(enum_type) => {
                let enum_renderer = EnumRenderer::new(enum_type.clone());
                let varlen_enums = attr.read_1d::<VarLenArray<u64>>()?;
                varlen_enums
                    .into_iter()
                    .flat_map(|varlen_enum| {
                        comma_separated(varlen_enum.iter().map(|v| enum_renderer.render_as_span(v)))
                    })
                    .collect()
            }
            TypeDescriptor::Compound(compound_type) => {
                vec![render_unsupported_type(format!(
                    "varlen array of compound type with fields: {}",
                    compound_type
                        .fields
                        .iter()
                        .map(|f| f.name.to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                ))]
            }
            TypeDescriptor::FixedArray(type_descriptor, _) => {
                vec![render_unsupported_type(format!(
                    "recursive limit reached - varlen array of fixed array of {type_descriptor}"
                ))]
            }
            TypeDescriptor::VarLenAscii
            | TypeDescriptor::VarLenUnicode
            | TypeDescriptor::FixedAscii(_)
            | TypeDescriptor::FixedUnicode(_) => {
                vec![render_unsupported_type("varlen array of strings should be handled by non scalar variants of those types")]
            }
            TypeDescriptor::VarLenArray(type_descriptor) => {
                vec![render_unsupported_type(format!(
                    "recursive limit reached - varlen array of varlen array of {type_descriptor}"
                ))]
            }
            TypeDescriptor::Reference(_) => {
                vec![render_unsupported_type(
                    "varlen array of reference".to_string(),
                )]
            }
        },
        TypeDescriptor::VarLenAscii => render_values(attr.read_1d::<VarLenAscii>()?),
        TypeDescriptor::VarLenUnicode => render_values(attr.read_1d::<VarLenUnicode>()?),
        TypeDescriptor::Reference(_) => vec![render_unsupported_type("reference array")],
    };
    Ok(gg)
}

pub fn sprint_attribute(attr: &hdf5_metno::Attribute) -> Result<Line<'static>, Error> {
    if attr.is_valid() {
        if attr.is_scalar() {
            let attr_type = attr.dtype()?.to_descriptor()?;
            let spans = sprint_attribute_scalar(attr, attr_type)?;
            let line = Line::from(spans);
            Ok(line)
        } else {
            let attr_type = attr.dtype()?.to_descriptor()?;
            let spans = bracketed_spans(spring_attribute_array(attr, attr_type)?);
            let line = Line::from(spans);
            Ok(line)
        }
    } else {
        let line = Line::from("Invalid attribute").style(color_consts::ERROR_COLOR);
        Ok(line)
    }
}

pub trait AttributeEditable {
    fn can_edit(&self) -> Result<(), String>;
}

impl AttributeEditable for Attribute {
    fn can_edit(&self) -> Result<(), String> {
        if self.is_valid() {
            let dtype = self.dtype().map_err(|e| e.to_string())?;
            let type_desc = dtype.to_descriptor().map_err(|e| e.to_string())?;
            if scalar_text_codec(&type_desc).is_some() {
                Ok(())
            } else {
                Err(format!(
                    "{type_desc} attribute type is not supported for editing. Delete it and create a new one with a supported type if you want to edit it."
                ))
            }
        } else {
            Err("Invalid attribute".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use hdf5_metno::types::{CompoundField, CompoundType, FloatSize, IntSize, TypeDescriptor};

    use super::render_value_from_bytes;

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
}
