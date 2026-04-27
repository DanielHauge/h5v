use hdf5_metno::{
    types::{
        self, FixedAscii, FixedUnicode, Reference, TypeDescriptor, VarLenArray, VarLenAscii,
        VarLenUnicode,
    },
    Attribute, Error,
};
use ratatui::{text::Line, text::Span};

use crate::{
    color_consts,
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

fn render_values<T>(values: impl IntoIterator<Item = T>) -> Vec<Span<'static>>
where
    T: Renderable,
{
    comma_separated(values.into_iter().map(Renderable::render))
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

fn sprint_attribute_scalar<'a>(
    attr: &hdf5_metno::Attribute,
    type_desc: TypeDescriptor,
) -> Result<Span<'a>, Error> {
    let val = match type_desc {
        types::TypeDescriptor::Integer(int_size) => match int_size {
            types::IntSize::U1 => attr.read_scalar::<i8>()?.render(),
            types::IntSize::U2 => attr.read_scalar::<i16>()?.render(),
            types::IntSize::U4 => attr.read_scalar::<i32>()?.render(),
            types::IntSize::U8 => attr.read_scalar::<i64>()?.render(),
        },
        types::TypeDescriptor::Unsigned(int_size) => match int_size {
            types::IntSize::U1 => attr.read_scalar::<u8>()?.render(),
            types::IntSize::U2 => attr.read_scalar::<u16>()?.render(),
            types::IntSize::U4 => attr.read_scalar::<u32>()?.render(),
            types::IntSize::U8 => attr.read_scalar::<u64>()?.render(),
        },
        types::TypeDescriptor::Float(float_size) => match float_size {
            types::FloatSize::U4 => attr.read_scalar::<f32>()?.render(),
            types::FloatSize::U8 => attr.read_scalar::<f64>()?.render(),
        },
        types::TypeDescriptor::Boolean => attr.read_scalar::<bool>()?.render(),
        types::TypeDescriptor::Enum(enum_type) => {
            let enum_renderer = EnumRenderer::new(enum_type);
            let v = attr.read_scalar::<u64>()?;
            enum_renderer.render_as_span(&v)
        }
        types::TypeDescriptor::FixedAscii(a) => render_fixed_ascii_scalar(attr, a)?,
        types::TypeDescriptor::FixedUnicode(a) => render_fixed_unicode_scalar(attr, a)?,
        types::TypeDescriptor::VarLenAscii => attr.read_scalar::<VarLenAscii>()?.render(),
        types::TypeDescriptor::VarLenUnicode => attr.read_scalar::<VarLenUnicode>()?.render(),
        types::TypeDescriptor::Reference(Reference::Object) => render_unsupported_type("ref obj"),
        types::TypeDescriptor::Reference(Reference::Region) => render_unsupported_type("ref reg"),
        types::TypeDescriptor::Reference(Reference::Std) => render_unsupported_type("ref std"),
        types::TypeDescriptor::VarLenArray(_) => render_unsupported_type("custom varlen array"),
        types::TypeDescriptor::Compound(_) => render_unsupported_type("compound"),
        types::TypeDescriptor::FixedArray(_, _) => render_unsupported_type("custom fixed array"),
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
        TypeDescriptor::Compound(_) => vec![render_unsupported_type("compound array")],
        TypeDescriptor::FixedArray(type_descriptor, size) => {
            vec![render_unsupported_type(format!(
                "fixed array of {type_descriptor} with size {size}"
            ))]
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
            let span = sprint_attribute_scalar(attr, attr_type)?;
            let line = Line::from(span);
            Ok(line)
        } else {
            let attr_type = attr.dtype()?.to_descriptor()?;
            let spans = spring_attribute_array(attr, attr_type)?;
            let spans = std::iter::once(symbol_span("["))
                .chain(spans)
                .chain(std::iter::once(symbol_span("]")))
                .collect::<Vec<Span<'static>>>();
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
            match type_desc {
                TypeDescriptor::Integer(_) => Ok(()),
                TypeDescriptor::Unsigned(_) => Ok(()),
                TypeDescriptor::Float(_) => Ok(()),
                TypeDescriptor::Boolean => Ok(()),
                TypeDescriptor::VarLenAscii => Ok(()),
                TypeDescriptor::VarLenUnicode => Ok(()),
                _ => Err(format!(
                    "{type_desc} attribute type is not supported for editing. Delete it and create a new one with a supported type if you want to edit it."
                )),
            }
        } else {
            Err("Invalid attribute".to_string())
        }
    }
}
