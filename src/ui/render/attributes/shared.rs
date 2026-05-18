use hdf5_metno::{
    types::{FixedAscii, FixedUnicode, VarLenArray, VarLenAscii, VarLenUnicode},
    Attribute, Error, H5Type,
};
use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

use crate::configure;

pub(super) trait Renderable {
    fn render(self) -> Span<'static>;
}

pub(super) fn styled_span(
    value: impl std::fmt::Display,
    color: ratatui::style::Color,
) -> Span<'static> {
    Span::from(value.to_string()).style(color)
}

pub(super) fn link_span(
    value: impl std::fmt::Display,
    color: ratatui::style::Color,
) -> Span<'static> {
    Span::from(value.to_string()).style(
        Style::default()
            .fg(color)
            .add_modifier(Modifier::UNDERLINED),
    )
}

pub(super) fn string_span(value: impl std::fmt::Display) -> Span<'static> {
    styled_span(
        format_args!("\"{value}\""),
        configure::themed_color(|colors| colors.text.string),
    )
}

pub(super) fn symbol_span(value: &'static str) -> Span<'static> {
    Span::raw(value).style(configure::themed_color(|colors| colors.accent.symbol))
}

pub(super) fn comma_separated<I>(spans: I) -> Vec<Span<'static>>
where
    I: IntoIterator<Item = Span<'static>>,
{
    itertools::intersperse(spans, symbol_span(", ")).collect()
}

pub(super) fn comma_separated_groups<I>(groups: I) -> Vec<Span<'static>>
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

pub(super) fn bracketed_spans(spans: Vec<Span<'static>>) -> Vec<Span<'static>> {
    std::iter::once(symbol_span("["))
        .chain(spans)
        .chain(std::iter::once(symbol_span("]")))
        .collect()
}

pub(super) fn braced_spans(spans: Vec<Span<'static>>) -> Vec<Span<'static>> {
    std::iter::once(symbol_span("{"))
        .chain(spans)
        .chain(std::iter::once(symbol_span("}")))
        .collect()
}

pub(super) fn single_span(span: Span<'static>) -> Vec<Span<'static>> {
    vec![span]
}

pub(super) fn render_values<T>(values: impl IntoIterator<Item = T>) -> Vec<Span<'static>>
where
    T: Renderable,
{
    comma_separated(values.into_iter().map(Renderable::render))
}

pub(super) fn render_varlen_values<T>(attr: &Attribute) -> Result<Vec<Span<'static>>, Error>
where
    T: Renderable + Copy + H5Type,
{
    let values = Vec::from(attr.read_scalar::<VarLenArray<T>>()?);
    Ok(bracketed_spans(render_values(values)))
}

pub(super) fn checked_byte_count(
    count: usize,
    element_size: usize,
    context: &str,
) -> Result<usize, Error> {
    count
        .checked_mul(element_size)
        .ok_or_else(|| Error::from(format!("{context} byte size overflowed usize")))
}

pub(super) fn render_unsupported_type(type_name: impl Into<String>) -> Span<'static> {
    let type_name = type_name.into();
    let message = format!("Unsupported type: {type_name}");
    Span::from(message).style(configure::themed_color(|colors| colors.text.error))
}

macro_rules! impl_numeric_renderable {
    ($($t:ty),* $(,)?) => {
        $(
            impl Renderable for $t {
                fn render(self) -> Span<'static> {
                    styled_span(self, configure::themed_color(|colors| colors.text.number))
                }
            }
        )*
    };
}

impl_numeric_renderable!(u8, u16, u32, u64, i8, i16, i32, i64, f32, f64);

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
