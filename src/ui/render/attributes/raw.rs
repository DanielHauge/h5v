use std::{ffi::c_void, slice};

use hdf5_metno::{
    h5check,
    types::{self, CompoundType, Reference, TypeDescriptor},
    Attribute, Error, ObjectReference1, ObjectReference2,
};
use hdf5_metno_sys::{h5p::H5P_DEFAULT, h5t::H5Treclaim};
use ratatui::text::Span;

use crate::{
    configure,
    h5f::read_attr_memory_bytes,
    ui::matrix::{EnumRenderer, RenderIntercept},
};

use super::{
    references::render_referenced_object,
    shared::{
        braced_spans, bracketed_spans, checked_byte_count, comma_separated_groups, single_span,
        string_span, styled_span, symbol_span, Renderable,
    },
};
use crate::ui::render::typedesc::sprint_typedescriptor;

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct RawVarLen {
    pub(super) len: usize,
    pub(super) ptr: *const c_void,
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

pub(super) fn render_value_from_bytes(
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
            let end = bytes
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(bytes.len());
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

pub(super) fn render_raw_scalar_value(
    attr: &Attribute,
    type_desc: &TypeDescriptor,
) -> Result<Vec<Span<'static>>, Error> {
    let bytes = read_attr_memory_bytes(attr).map_err(|error| Error::from(error.to_string()))?;
    render_value_from_bytes(&bytes, type_desc)
}

pub(super) fn render_raw_array_values(
    attr: &Attribute,
    element_type: &TypeDescriptor,
) -> Result<Vec<Span<'static>>, Error> {
    let bytes = read_attr_memory_bytes(attr).map_err(|error| Error::from(error.to_string()))?;
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

pub(super) fn render_varlen_entry(
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

pub(super) fn render_varlen_attr_values(
    attr: &Attribute,
    element_type: &TypeDescriptor,
    wrap_output: bool,
) -> Result<Vec<Span<'static>>, Error> {
    let dtype = attr.dtype()?;
    let space = attr.space()?;
    let file = attr.file()?;
    let mut bytes = read_attr_memory_bytes(attr).map_err(|error| Error::from(error.to_string()))?;
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
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(Error::from(format!(
            "Failed to reclaim varlen attribute memory: {error}"
        ))),
    }
}
