use std::f64;

use hdf5_metno::{
    types::{self, FixedAscii, FixedUnicode, TypeDescriptor, VarLenAscii, VarLenUnicode},
    Error,
};
use ratatui::{text::Line, text::Span};

use crate::color_consts;

// pub trait LineRenderable {
//     fn render_line(&self) -> Line<'static>;
// }
//
// impl LineRenderable for Attribute {
//     fn render_line(&self) -> Line<'static> {
//         match sprint_attribute(self) {
//             Ok(s) => s,
//             Err(c) => {
//                 let s = format!("{c}");
//                 Line::from(s).style(color_consts::ERROR_COLOR)
//             }
//         }
//     }
// }

pub trait Renderable {
    fn render(self) -> Span<'static>;
}

macro_rules! impl_uint_renderable {
    ($($t:ty),*) => {
        $(
            impl Renderable for $t {
                fn render(self) -> Span<'static> {
                    let s = format!("{self}");
                    Span::from(s.clone()).style(color_consts::UINT_COLOR)
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
                    let s = format!("{self}");
                    Span::from(s.clone()).style(color_consts::INT_COLOR)
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
                    let s = format!("{self}");
                    Span::from(s.clone()).style(color_consts::FLOAT_COLOR)
                }
            }
        )*
    };
}
impl_float_renderable!(f32, f64);

impl Renderable for bool {
    fn render(self) -> Span<'static> {
        let s = format!("{self}");
        Span::from(s).style(color_consts::BOOL_COLOR)
    }
}

impl<const N: usize> Renderable for FixedAscii<N> {
    fn render(self) -> Span<'static> {
        let s = format!("\"{self}\"");
        Span::from(s.to_string()).style(color_consts::STRING_COLOR)
    }
}

impl<const N: usize> Renderable for FixedUnicode<N> {
    fn render(self) -> Span<'static> {
        let s = format!("\"{self}\"");
        Span::from(s.to_string()).style(color_consts::STRING_COLOR)
    }
}

impl Renderable for VarLenAscii {
    fn render(self) -> Span<'static> {
        let s = format!("\"{self}\"");
        Span::from(s.to_string()).style(color_consts::STRING_COLOR)
    }
}

impl Renderable for VarLenUnicode {
    fn render(self) -> Span<'static> {
        let s = format!("\"{self}\"");
        Span::from(s.to_string()).style(color_consts::STRING_COLOR)
    }
}

trait RenderableVec {
    fn render(self) -> Vec<Span<'static>>;
}

impl<T> RenderableVec for Vec<T>
where
    T: Renderable,
{
    fn render(self) -> Vec<Span<'static>> {
        let spans: Vec<Span<'static>> = self.into_iter().map(|item| item.render()).collect();
        let spans_iter = spans.into_iter();
        let spans_interspersed: Vec<Span<'static>> = itertools::intersperse(
            spans_iter,
            Span::raw(", ").style(color_consts::SYMBOL_COLOR),
        )
        .collect();

        spans_interspersed
    }
}

impl<T> RenderableVec for &[T]
where
    T: Renderable + Clone, // Needed because we iterate over references
{
    fn render(self) -> Vec<Span<'static>> {
        let spans: Vec<Span<'static>> = self.iter().map(|item| item.clone().render()).collect();
        let spans_iter = spans.into_iter();
        let spans_interspersed: Vec<Span<'static>> = itertools::intersperse(
            spans_iter,
            Span::raw(", ").style(color_consts::SYMBOL_COLOR),
        )
        .collect();

        spans_interspersed
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
            let s = format!("{:?}", enum_type);
            let span = Span::from(s).style(color_consts::BOOL_COLOR);
            span
        }
        types::TypeDescriptor::FixedAscii(a) => match a {
            0..32 => attr.read_scalar::<FixedAscii<32>>()?.render(),
            32..64 => attr.read_scalar::<FixedAscii<64>>()?.render(),
            64..128 => attr.read_scalar::<FixedAscii<128>>()?.render(),
            128..256 => attr.read_scalar::<FixedAscii<256>>()?.render(),
            256..512 => attr.read_scalar::<FixedAscii<512>>()?.render(),
            512..1024 => attr.read_scalar::<FixedAscii<1024>>()?.render(),
            1024..2048 => attr.read_scalar::<FixedAscii<2048>>()?.render(),
            2048..4096 => attr.read_scalar::<FixedAscii<4096>>()?.render(),
            _ => attr.read_scalar::<FixedAscii<8192>>()?.render(),
        },
        types::TypeDescriptor::FixedUnicode(a) => match a {
            0..32 => attr.read_scalar::<FixedUnicode<32>>()?.render(),
            32..64 => attr.read_scalar::<FixedUnicode<64>>()?.render(),
            64..128 => attr.read_scalar::<FixedUnicode<128>>()?.render(),
            128..256 => attr.read_scalar::<FixedUnicode<256>>()?.render(),
            256..512 => attr.read_scalar::<FixedUnicode<512>>()?.render(),
            512..1024 => attr.read_scalar::<FixedUnicode<1024>>()?.render(),
            1024..2048 => attr.read_scalar::<FixedUnicode<2048>>()?.render(),
            2048..4096 => attr.read_scalar::<FixedUnicode<4096>>()?.render(),
            _ => attr.read_scalar::<FixedUnicode<8192>>()?.render(),
        },
        types::TypeDescriptor::VarLenAscii => attr.read_scalar::<VarLenAscii>()?.render(),
        types::TypeDescriptor::VarLenUnicode => attr.read_scalar::<VarLenUnicode>()?.render(),
        types::TypeDescriptor::Reference(_) => render_unsupported_type("reference"),
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
            types::IntSize::U1 => attr
                .read_1d::<i8>()?
                .into_iter()
                .collect::<Vec<i8>>()
                .render(),
            types::IntSize::U2 => attr
                .read_1d::<i16>()?
                .into_iter()
                .collect::<Vec<i16>>()
                .render(),
            types::IntSize::U4 => attr
                .read_1d::<i32>()?
                .into_iter()
                .collect::<Vec<i32>>()
                .render(),
            types::IntSize::U8 => attr
                .read_1d::<i64>()?
                .into_iter()
                .collect::<Vec<i64>>()
                .render(),
        },
        TypeDescriptor::Unsigned(int_size) => match int_size {
            types::IntSize::U1 => attr
                .read_1d::<u8>()?
                .into_iter()
                .collect::<Vec<u8>>()
                .render(),
            types::IntSize::U2 => attr
                .read_1d::<u16>()?
                .into_iter()
                .collect::<Vec<u16>>()
                .render(),
            types::IntSize::U4 => attr
                .read_1d::<u32>()?
                .into_iter()
                .collect::<Vec<u32>>()
                .render(),
            types::IntSize::U8 => attr
                .read_1d::<u64>()?
                .into_iter()
                .collect::<Vec<u64>>()
                .render(),
        },
        TypeDescriptor::Float(float_size) => match float_size {
            types::FloatSize::U4 => attr
                .read_1d::<f32>()?
                .into_iter()
                .collect::<Vec<f32>>()
                .render(),
            types::FloatSize::U8 => attr
                .read_1d::<f64>()?
                .into_iter()
                .collect::<Vec<f64>>()
                .render(),
        },
        TypeDescriptor::FixedAscii(n) => match n {
            0..32 => attr
                .read_1d::<FixedAscii<32>>()?
                .into_iter()
                .collect::<Vec<FixedAscii<32>>>()
                .render(),
            32..64 => attr
                .read_1d::<FixedAscii<64>>()?
                .into_iter()
                .collect::<Vec<FixedAscii<64>>>()
                .render(),
            64..128 => attr
                .read_1d::<FixedAscii<128>>()?
                .into_iter()
                .collect::<Vec<FixedAscii<128>>>()
                .render(),
            128..256 => attr
                .read_1d::<FixedAscii<256>>()?
                .into_iter()
                .collect::<Vec<FixedAscii<256>>>()
                .render(),
            256..512 => attr
                .read_1d::<FixedAscii<512>>()?
                .into_iter()
                .collect::<Vec<FixedAscii<512>>>()
                .render(),
            512..1024 => attr
                .read_1d::<FixedAscii<1024>>()?
                .into_iter()
                .collect::<Vec<FixedAscii<1024>>>()
                .render(),
            1024..2048 => attr
                .read_1d::<FixedAscii<2048>>()?
                .into_iter()
                .collect::<Vec<FixedAscii<2048>>>()
                .render(),
            2048..4096 => attr
                .read_1d::<FixedAscii<4096>>()?
                .into_iter()
                .collect::<Vec<FixedAscii<4096>>>()
                .render(),
            _ => attr
                .read_1d::<FixedAscii<8192>>()?
                .into_iter()
                .collect::<Vec<FixedAscii<8192>>>()
                .render(),
        },
        TypeDescriptor::Boolean => attr
            .read_1d::<bool>()?
            .into_iter()
            .collect::<Vec<bool>>()
            .render(),
        TypeDescriptor::Enum(_) => vec![render_unsupported_type("enum array")],
        TypeDescriptor::Compound(_) => vec![render_unsupported_type("compound array")],
        TypeDescriptor::FixedArray(type_descriptor, size) => {
            vec![render_unsupported_type(format!(
                "fixed array of {type_descriptor} with size {size}"
            ))]
        }
        TypeDescriptor::FixedUnicode(size) => match size {
            0..32 => attr
                .read_1d::<FixedUnicode<32>>()?
                .into_iter()
                .collect::<Vec<FixedUnicode<32>>>()
                .render(),
            32..64 => attr
                .read_1d::<FixedUnicode<64>>()?
                .into_iter()
                .collect::<Vec<FixedUnicode<64>>>()
                .render(),
            64..128 => attr
                .read_1d::<FixedUnicode<128>>()?
                .into_iter()
                .collect::<Vec<FixedUnicode<128>>>()
                .render(),
            128..256 => attr
                .read_1d::<FixedUnicode<256>>()?
                .into_iter()
                .collect::<Vec<FixedUnicode<256>>>()
                .render(),
            256..512 => attr
                .read_1d::<FixedUnicode<512>>()?
                .into_iter()
                .collect::<Vec<FixedUnicode<512>>>()
                .render(),
            512..1024 => attr
                .read_1d::<FixedUnicode<1024>>()?
                .into_iter()
                .collect::<Vec<FixedUnicode<1024>>>()
                .render(),
            1024..2048 => attr
                .read_1d::<FixedUnicode<2048>>()?
                .into_iter()
                .collect::<Vec<FixedUnicode<2048>>>()
                .render(),
            2048..4096 => attr
                .read_1d::<FixedUnicode<4096>>()?
                .into_iter()
                .collect::<Vec<FixedUnicode<4096>>>()
                .render(),
            _ => attr
                .read_1d::<FixedUnicode<8192>>()?
                .into_iter()
                .collect::<Vec<FixedUnicode<8192>>>()
                .render(),
        },
        TypeDescriptor::VarLenArray(type_descriptor) => vec![render_unsupported_type(format!(
            "varlen array of {type_descriptor}"
        ))],
        TypeDescriptor::VarLenAscii => attr
            .read_1d::<VarLenAscii>()?
            .into_iter()
            .collect::<Vec<VarLenAscii>>()
            .render(),
        TypeDescriptor::VarLenUnicode => attr
            .read_1d::<VarLenUnicode>()?
            .into_iter()
            .collect::<Vec<VarLenUnicode>>()
            .render(),
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
            let start_start = Span::raw("[").style(color_consts::SYMBOL_COLOR);
            let start_end = Span::raw("]").style(color_consts::SYMBOL_COLOR);
            let start = vec![start_start];
            let end = vec![start_end];
            let spans = start
                .into_iter()
                .chain(spans)
                .chain(end)
                .collect::<Vec<Span<'static>>>();
            let line = Line::from(spans);
            Ok(line)
        }
    } else {
        let line = Line::from("Ivalid Attribute").style(color_consts::ERROR_COLOR);
        Ok(line)
    }
}
