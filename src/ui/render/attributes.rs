use std::{ffi::c_void, slice};

use hdf5_metno::{
    datatype::Datatype,
    from_id, h5check,
    types::{
        self, CompoundType, FixedAscii, FixedUnicode, Reference, TypeDescriptor, VarLenArray,
        VarLenAscii, VarLenUnicode,
    },
    Attribute, Error, H5Type, ObjectReference, ObjectReference1, ObjectReference2,
    ReferencedObject,
};
use hdf5_metno_sys::{
    h5p::H5P_DEFAULT,
    h5t::{H5Tget_class, H5Tget_super, H5Treclaim, H5T_REFERENCE, H5T_VLEN},
};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::{
    configure,
    h5f::{ensure_attr_editable, format_opaque_bytes_for_edit, read_attr_memory_bytes},
    ui::matrix::{EnumRenderer, RenderIntercept},
};

use super::typedesc::sprint_typedescriptor;

pub trait Renderable {
    fn render(self) -> Span<'static>;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawVarLen {
    len: usize,
    ptr: *const c_void,
}

fn styled_span(value: impl std::fmt::Display, color: ratatui::style::Color) -> Span<'static> {
    Span::from(value.to_string()).style(color)
}

fn link_span(value: impl std::fmt::Display, color: ratatui::style::Color) -> Span<'static> {
    Span::from(value.to_string()).style(
        Style::default()
            .fg(color)
            .add_modifier(Modifier::UNDERLINED),
    )
}

fn string_span(value: impl std::fmt::Display) -> Span<'static> {
    styled_span(
        format_args!("\"{value}\""),
        configure::themed_color(|colors| colors.text.string),
    )
}

fn symbol_span(value: &'static str) -> Span<'static> {
    Span::raw(value).style(configure::themed_color(|colors| colors.accent.symbol))
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

fn checked_byte_count(count: usize, element_size: usize, context: &str) -> Result<usize, Error> {
    count
        .checked_mul(element_size)
        .ok_or_else(|| Error::from(format!("{context} byte size overflowed usize")))
}

macro_rules! impl_uint_renderable {
    ($($t:ty),*) => {
        $(
            impl Renderable for $t {
                fn render(self) -> Span<'static> {
                    styled_span(self, configure::themed_color(|colors| colors.text.number))
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
                    styled_span(self, configure::themed_color(|colors| colors.text.number))
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
                    styled_span(self, configure::themed_color(|colors| colors.text.number))
                }
            }
        )*
    };
}
impl_float_renderable!(f32, f64);

impl Renderable for bool {
    fn render(self) -> Span<'static> {
        styled_span(
            self,
            configure::themed_color(|colors| colors.text.bool_value),
        )
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
    underlined: bool,
) -> Vec<Span<'static>> {
    let name = if name.is_empty() {
        "<anonymous>".to_string()
    } else {
        name
    };
    let kind_span = if underlined {
        link_span(kind, color)
    } else {
        styled_span(kind, color)
    };
    let name_span = if underlined {
        link_span(&name, color)
    } else {
        styled_span(&name, color)
    };
    vec![symbol_span("&"), kind_span, symbol_span(":"), name_span]
}

fn render_referenced_object(object: ReferencedObject) -> Vec<Span<'static>> {
    match object {
        ReferencedObject::Group(group) => render_reference_target(
            "group",
            group.name(),
            configure::themed_color(|colors| colors.tree.group),
            true,
        ),
        ReferencedObject::Dataset(dataset) => render_reference_target(
            "dataset",
            dataset.name(),
            configure::themed_color(|colors| colors.tree.dataset),
            true,
        ),
        ReferencedObject::Datatype(datatype) => render_reference_target(
            "datatype",
            datatype
                .as_location()
                .map(|location| location.name())
                .unwrap_or_else(|_| datatype.to_string()),
            configure::themed_color(|colors| colors.text.type_desc),
            false,
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

fn render_reference_array<R>(attr: &Attribute) -> Result<Vec<Span<'static>>, Error>
where
    R: ObjectReference + H5Type,
{
    let references = attr.read_1d::<R>()?;
    let file = attr.file()?;
    let rendered = references
        .iter()
        .map(|reference| file.dereference(reference).map(render_referenced_object))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(comma_separated_groups(rendered))
}

fn detect_reference_descriptor(dtype: &Datatype) -> Result<TypeDescriptor, Error> {
    let object_ref = Datatype::from_descriptor(&TypeDescriptor::Reference(Reference::Object))?;
    if dtype == &object_ref {
        return Ok(TypeDescriptor::Reference(Reference::Object));
    }

    let region_ref = Datatype::from_descriptor(&TypeDescriptor::Reference(Reference::Region))?;
    if dtype == &region_ref {
        return Ok(TypeDescriptor::Reference(Reference::Region));
    }

    let std_ref = Datatype::from_descriptor(&TypeDescriptor::Reference(Reference::Std))?;
    if dtype == &std_ref {
        return Ok(TypeDescriptor::Reference(Reference::Std));
    }

    Err(Error::from("Unsupported reference datatype"))
}

fn fallback_type_descriptor(dtype: &Datatype) -> Result<TypeDescriptor, Error> {
    match unsafe { H5Tget_class(dtype.id()) } {
        H5T_REFERENCE => detect_reference_descriptor(dtype),
        H5T_VLEN => {
            let super_dtype = unsafe { from_id::<Datatype>(H5Tget_super(dtype.id())) }?;
            Ok(TypeDescriptor::VarLenArray(Box::new(
                type_descriptor_for_dtype(&super_dtype)?,
            )))
        }
        _ => Err(Error::from("Unsupported datatype class")),
    }
}

fn type_descriptor_for_dtype(dtype: &Datatype) -> Result<TypeDescriptor, Error> {
    match dtype.to_descriptor() {
        Ok(type_desc) => Ok(type_desc),
        Err(err) if err.to_string() == "Unsupported datatype class" => {
            fallback_type_descriptor(dtype)
        }
        Err(err) => Err(err),
    }
}

pub fn attribute_type_descriptor(attr: &Attribute) -> Result<TypeDescriptor, Error> {
    let dtype = attr.dtype()?;
    type_descriptor_for_dtype(&dtype)
}

pub fn attribute_type_description(attr: &Attribute) -> Result<String, Error> {
    match attribute_type_descriptor(attr) {
        Ok(type_desc) => Ok(type_desc.to_string()),
        Err(err) if err.to_string() == "Unsupported datatype class" => {
            Ok(format!("opaque[{} bytes]", attr.dtype()?.size()))
        }
        Err(err) => Err(err),
    }
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
    styled_span(
        name,
        configure::themed_color(|colors| colors.tree.group_name),
    )
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
            configure::themed_color(|colors| colors.text.type_desc),
        )])),
        TypeDescriptor::VarLenAscii | TypeDescriptor::VarLenUnicode => Ok(vec![styled_span(
            sprint_typedescriptor(type_desc),
            configure::themed_color(|colors| colors.text.type_desc),
        )]),
        TypeDescriptor::Reference(reference_type) => Ok(vec![styled_span(
            sprint_typedescriptor(&TypeDescriptor::Reference(*reference_type)),
            configure::themed_color(|colors| colors.text.type_desc),
        )]),
    }
}

fn render_raw_scalar_value(
    attr: &Attribute,
    type_desc: &TypeDescriptor,
) -> Result<Vec<Span<'static>>, Error> {
    let bytes = read_attr_memory_bytes(attr).map_err(|e| Error::from(e.to_string()))?;
    render_value_from_bytes(&bytes, type_desc)
}

fn render_raw_array_values(
    attr: &Attribute,
    element_type: &TypeDescriptor,
) -> Result<Vec<Span<'static>>, Error> {
    let bytes = read_attr_memory_bytes(attr).map_err(|e| Error::from(e.to_string()))?;
    let element_size = element_type.size();
    let expected_len = checked_byte_count(attr.size(), element_size, "Raw array")?;
    if bytes.len() != expected_len {
        return Err(Error::from(format!(
            "Raw array byte size mismatch: expected {}, got {}",
            expected_len,
            bytes.len()
        )));
    }

    let rendered = bytes
        .chunks_exact(element_size)
        .map(|chunk| render_value_from_bytes(chunk, element_type))
        .collect::<Result<Vec<_>, Error>>()?;

    Ok(comma_separated_groups(rendered))
}

fn render_varlen_entry(
    file: &hdf5_metno::File,
    entry: RawVarLen,
    element_type: &TypeDescriptor,
) -> Result<Vec<Span<'static>>, Error> {
    if entry.len == 0 {
        return Ok(bracketed_spans(vec![]));
    }
    if entry.ptr.is_null() {
        return Err(Error::from("Varlen value pointer was null"));
    }

    match element_type {
        TypeDescriptor::Reference(Reference::Object) => {
            let refs =
                unsafe { slice::from_raw_parts(entry.ptr.cast::<ObjectReference1>(), entry.len) };
            let rendered = refs
                .iter()
                .map(|reference| file.dereference(reference).map(render_referenced_object))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(bracketed_spans(comma_separated_groups(rendered)))
        }
        TypeDescriptor::Reference(Reference::Std) => {
            let refs =
                unsafe { slice::from_raw_parts(entry.ptr.cast::<ObjectReference2>(), entry.len) };
            let rendered = refs
                .iter()
                .map(|reference| file.dereference(reference).map(render_referenced_object))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(bracketed_spans(comma_separated_groups(rendered)))
        }
        TypeDescriptor::Reference(Reference::Region) => Ok(bracketed_spans(vec![styled_span(
            format!("{} region references", entry.len),
            configure::themed_color(|colors| colors.text.type_desc),
        )])),
        TypeDescriptor::VarLenAscii
        | TypeDescriptor::VarLenUnicode
        | TypeDescriptor::VarLenArray(_) => Ok(bracketed_spans(vec![styled_span(
            sprint_typedescriptor(&TypeDescriptor::VarLenArray(Box::new(element_type.clone()))),
            configure::themed_color(|colors| colors.text.type_desc),
        )])),
        _ => {
            let element_size = element_type.size();
            let byte_len = checked_byte_count(entry.len, element_size, "Varlen value")?;
            let bytes = unsafe { slice::from_raw_parts(entry.ptr.cast::<u8>(), byte_len) };
            let rendered = bytes
                .chunks_exact(element_size)
                .map(|chunk| render_value_from_bytes(chunk, element_type))
                .collect::<Result<Vec<_>, Error>>()?;
            Ok(bracketed_spans(comma_separated_groups(rendered)))
        }
    }
}

fn render_varlen_attr_values(
    attr: &Attribute,
    element_type: &TypeDescriptor,
    wrap_output: bool,
) -> Result<Vec<Span<'static>>, Error> {
    let dtype = attr.dtype()?;
    let space = attr.space()?;
    let file = attr.file()?;
    let mut bytes = read_attr_memory_bytes(attr).map_err(|e| Error::from(e.to_string()))?;
    let result = (|| {
        let chunks = bytes.chunks_exact(std::mem::size_of::<RawVarLen>());
        if !chunks.remainder().is_empty() {
            return Err(Error::from("Invalid varlen attribute payload length"));
        }
        let entries = chunks
            .map(|chunk| {
                let mut raw = [0_u8; std::mem::size_of::<RawVarLen>()];
                raw.copy_from_slice(chunk);
                let (len_bytes, ptr_bytes) = raw.split_at(std::mem::size_of::<usize>());
                Ok(RawVarLen {
                    len: usize::from_ne_bytes(
                        len_bytes
                            .try_into()
                            .map_err(|_| Error::from("Invalid varlen length bytes"))?,
                    ),
                    ptr: usize::from_ne_bytes(
                        ptr_bytes
                            .try_into()
                            .map_err(|_| Error::from("Invalid varlen pointer bytes"))?,
                    ) as *const c_void,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let rendered = entries
            .into_iter()
            .map(|entry| render_varlen_entry(&file, entry, element_type))
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(if wrap_output {
            bracketed_spans(comma_separated_groups(rendered))
        } else {
            comma_separated_groups(rendered)
        })
    })();
    let reclaim_result = h5check(unsafe {
        H5Treclaim(
            dtype.id(),
            space.id(),
            H5P_DEFAULT,
            bytes.as_mut_ptr().cast(),
        )
    })
    .map(|_| ());

    match (result, reclaim_result) {
        (Ok(rendered), Ok(())) => Ok(rendered),
        (Err(err), _) => Err(err),
        (_, Err(err)) => Err(Error::from(format!(
            "Failed to reclaim varlen attribute memory: {err}"
        ))),
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
                .map(|reference| file.dereference(reference).map(render_referenced_object))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(bracketed_spans(comma_separated_groups(rendered)))
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
            configure::themed_color(|colors| colors.text.type_desc),
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
    Span::from(s).style(configure::themed_color(|colors| colors.text.error))
}

fn render_opaque_scalar(attr: &Attribute) -> Result<Line<'static>, Error> {
    let bytes = read_attr_memory_bytes(attr).map_err(|err| Error::from(err.to_string()))?;
    Ok(Line::from(Span::styled(
        format_opaque_bytes_for_edit(&bytes),
        Style::default().fg(configure::themed_color(|colors| colors.text.opaque)),
    )))
}

fn render_opaque_array(attr: &Attribute) -> Result<Line<'static>, Error> {
    let dtype = attr.dtype()?;
    let item_size = dtype.size();
    let bytes = read_attr_memory_bytes(attr).map_err(|err| Error::from(err.to_string()))?;
    let spans = if item_size == 0 {
        vec![Span::styled(
            "<zero-sized opaque values>",
            Style::default().fg(configure::themed_color(|colors| colors.text.opaque)),
        )]
    } else {
        comma_separated_groups(
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
        TypeDescriptor::VarLenArray(type_descriptor) => {
            render_varlen_attr_values(attr, type_descriptor.as_ref(), false)?
        }
        TypeDescriptor::VarLenAscii => render_values(attr.read_1d::<VarLenAscii>()?),
        TypeDescriptor::VarLenUnicode => render_values(attr.read_1d::<VarLenUnicode>()?),
        TypeDescriptor::Reference(Reference::Object) => {
            render_reference_array::<ObjectReference1>(attr)?
        }
        TypeDescriptor::Reference(Reference::Std) => {
            render_reference_array::<ObjectReference2>(attr)?
        }
        TypeDescriptor::Reference(Reference::Region) => {
            vec![render_unsupported_type("region reference array")]
        }
    };
    Ok(gg)
}

pub fn sprint_attribute(attr: &hdf5_metno::Attribute) -> Result<Line<'static>, Error> {
    if attr.is_valid() {
        if attr.is_scalar() {
            let attr_type = match attribute_type_descriptor(attr) {
                Ok(attr_type) => attr_type,
                Err(err) if err.to_string() == "Unsupported datatype class" => {
                    return render_opaque_scalar(attr);
                }
                Err(err) => return Err(err),
            };
            let spans = sprint_attribute_scalar(attr, attr_type)?;
            let line = Line::from(spans);
            Ok(line)
        } else {
            let attr_type = match attribute_type_descriptor(attr) {
                Ok(attr_type) => attr_type,
                Err(err) if err.to_string() == "Unsupported datatype class" => {
                    return render_opaque_array(attr);
                }
                Err(err) => return Err(err),
            };
            let spans = bracketed_spans(spring_attribute_array(attr, attr_type)?);
            let line = Line::from(spans);
            Ok(line)
        }
    } else {
        let line = Line::from("Invalid attribute")
            .style(configure::themed_color(|colors| colors.text.error));
        Ok(line)
    }
}

pub trait AttributeEditable {
    fn can_edit(&self) -> Result<(), String>;
}

impl AttributeEditable for Attribute {
    fn can_edit(&self) -> Result<(), String> {
        ensure_attr_editable(self).map_err(|e| e.to_string())
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
