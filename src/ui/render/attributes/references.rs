use hdf5_metno::{Attribute, Error, H5Type, ObjectReference, ReferencedObject};
use ratatui::text::Span;

use crate::configure;

use super::shared::{comma_separated_groups, link_span, styled_span, symbol_span};

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

pub(super) fn render_referenced_object(object: ReferencedObject) -> Vec<Span<'static>> {
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

pub(super) fn render_reference_scalar<R>(attr: &Attribute) -> Result<Vec<Span<'static>>, Error>
where
    R: ObjectReference,
{
    let reference = attr.read_scalar::<R>()?;
    let file = attr.file()?;
    let object = file.dereference(&reference)?;
    Ok(render_referenced_object(object))
}

pub(super) fn render_reference_array<R>(attr: &Attribute) -> Result<Vec<Span<'static>>, Error>
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
