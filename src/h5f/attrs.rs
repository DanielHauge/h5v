use hdf5_metno::{Attribute, Group};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::{
    configure,
    error::AppError,
    sprint_attributes::{attribute_type_description, sprint_attribute},
    ui::state::AttributeViewSelection,
};

use super::{
    codec::{
        copy_attr_to_group, create_scalar_attr_from_text, rewrite_fixed_string_attr,
        write_attr_from_text, AttributeCreateType, FixedStringRewrite,
    },
    meta::SYSTEM_PROPERTIES,
    model::{H5FNode, Node},
};

pub trait HasAttributes {
    fn attribute(&self, name: &str) -> Result<Attribute, hdf5_metno::Error>;
    fn attribute_names(&self) -> Result<Vec<String>, hdf5_metno::Error>;
    fn attributes(&self) -> Result<Vec<(String, Attribute)>, hdf5_metno::Error>;
    fn create_attr(
        &self,
        name: &str,
        attr_type: AttributeCreateType,
        value: &str,
    ) -> Result<String, AppError>;
    fn delete_attr(&self, name: &str) -> Result<(), AppError>;
    fn update_attr_name(&self, old_name: &str, new_name: &str) -> Result<(), AppError>;
}

pub trait HasName {
    fn name(&self) -> String;
}

pub trait HasPath {
    fn path(&self) -> String;
}

impl HasName for Node {
    fn name(&self) -> String {
        match self {
            Node::File(file) => {
                let f_name = file.filename();
                file.name()
                    .split('/')
                    .next_back()
                    .unwrap_or(&f_name)
                    .to_string()
            }
            Node::Group(_, meta) => meta.display_name.clone(),
            Node::Dataset(_, meta) => meta.display_name.clone(),
            Node::Broken(_, name, _) => name.clone(),
        }
    }
}

impl HasPath for Node {
    fn path(&self) -> String {
        match self {
            Node::File(file) => file.name(),
            Node::Group(group, _) => group.name(),
            Node::Dataset(dataset, meta) => meta
                .virtual_path()
                .map(ToString::to_string)
                .unwrap_or_else(|| dataset.name()),
            Node::Broken(_, n, _) => n.clone(),
        }
        .to_string()
    }
}

impl HasAttributes for Node {
    fn attributes(&self) -> Result<Vec<(String, Attribute)>, hdf5_metno::Error> {
        let attr_names = self.attribute_names()?;
        let mut attrs = vec![];
        for name in attr_names {
            let attr = self.attribute(&name)?;
            attrs.push((name, attr));
        }
        Ok(attrs)
    }

    fn attribute_names(&self) -> Result<Vec<String>, hdf5_metno::Error> {
        match self {
            Node::File(file) => Ok(file.attr_names()?),
            Node::Group(group, _) => Ok(group.attr_names()?),
            Node::Dataset(dataset, _) => Ok(dataset.attr_names()?),
            Node::Broken(_, _, _) => Ok(vec![]),
        }
    }

    fn attribute(&self, name: &str) -> Result<Attribute, hdf5_metno::Error> {
        match self {
            Node::File(file) => file.attr(name),
            Node::Group(group, _) => group.attr(name),
            Node::Dataset(dataset, _) => dataset.attr(name),
            Node::Broken(_, _, _) => Err(hdf5_metno::Error::Internal(String::from(
                "Cannot read from broken link",
            ))),
        }
    }

    fn create_attr(
        &self,
        name: &str,
        attr_type: AttributeCreateType,
        value: &str,
    ) -> Result<String, AppError> {
        let name = validate_user_attribute_name(name)?;
        let group = node_attribute_group(self)?;
        let existing = group.attr_names()?;
        if existing.iter().any(|existing_name| existing_name == &name) {
            return Err(AppError::EditError(format!(
                "Attribute '{}' already exists",
                name
            )));
        }
        create_scalar_attr_from_text(&group, &name, attr_type, value)
    }

    fn delete_attr(&self, name: &str) -> Result<(), AppError> {
        let name = validate_user_attribute_name(name)?;
        let group = node_attribute_group(self)?;
        let existing = group.attr_names()?;
        if !existing.iter().any(|existing_name| existing_name == &name) {
            return Err(AppError::EditError(format!(
                "Attribute '{}' does not exist",
                name
            )));
        }
        group.delete_attr(&name)?;
        group.file()?.flush()?;
        Ok(())
    }

    fn update_attr_name(&self, old_name: &str, new_name: &str) -> Result<(), AppError> {
        let new_name = validate_user_attribute_name(new_name)?;
        let group = match self {
            Node::File(file) => file.as_group()?,
            Node::Group(group, _) => group.as_group()?,
            Node::Dataset(dataset, _) => dataset.as_group()?,
            Node::Broken(_, _, _) => {
                return Err(hdf5_metno::Error::Internal(String::from(
                    "Cannot update attribute on broken link",
                ))
                .into())
            }
        };

        let attr = group.attr(old_name)?;
        let existing = group.attr_names()?;
        if existing
            .iter()
            .any(|existing_name| existing_name == &new_name && existing_name != old_name)
        {
            return Err(AppError::EditError(format!(
                "Attribute '{}' already exists",
                new_name
            )));
        }
        copy_attr_to_group(&attr, &group, &new_name)?;
        group.delete_attr(old_name)?;
        group.file()?.flush()?;

        Ok(())
    }
}

fn node_attribute_group(node: &Node) -> Result<Group, AppError> {
    match node {
        Node::File(file) => file.as_group().map_err(AppError::from),
        Node::Group(group, _) => group.as_group().map_err(AppError::from),
        Node::Dataset(dataset, _) => dataset.as_group().map_err(AppError::from),
        Node::Broken(_, _, _) => Err(hdf5_metno::Error::Internal(String::from(
            "Cannot update attribute on broken link",
        ))
        .into()),
    }
}

pub fn validate_user_attribute_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim_matches(|ch| matches!(ch, '=' | '─' | '-')).trim();
    if trimmed.is_empty() {
        return Err(AppError::EditError(
            "Attribute name cannot be empty".to_string(),
        ));
    }
    if SYSTEM_PROPERTIES.contains(&trimmed) {
        return Err(AppError::EditError(format!(
            "'{}' is a built-in h5v property and cannot be used as an attribute name",
            trimmed
        )));
    }
    Ok(trimmed.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataRowKind {
    SectionHeader,
    Property,
    Attribute,
}

#[derive(Debug, Clone)]
pub struct RenderedAttributeRow {
    pub kind: MetadataRowKind,
    pub key: Option<String>,
    pub name_line: Line<'static>,
    pub value_line: Line<'static>,
    pub type_line: Line<'static>,
}

impl RenderedAttributeRow {
    pub fn section(title: &str) -> Self {
        Self {
            kind: MetadataRowKind::SectionHeader,
            key: None,
            name_line: Line::styled(
                match title {
                    "Properties" => {
                        configure::configured_symbol(|symbols| symbols.section.properties_title)
                            .to_string()
                    }
                    "Attributes" => {
                        configure::configured_symbol(|symbols| symbols.section.attributes_title)
                            .to_string()
                    }
                    other => other.to_string(),
                },
                Style::default()
                    .fg(configure::themed_color(|colors| colors.metadata.section))
                    .bold(),
            ),
            value_line: Line::from(vec![Span::raw("")]),
            type_line: Line::from(vec![Span::raw("")]),
        }
    }

    pub fn property(
        key: impl Into<String>,
        cells: (Line<'static>, Line<'static>, Line<'static>),
    ) -> Self {
        let (name_line, value_line, type_line) = cells;
        Self {
            kind: MetadataRowKind::Property,
            key: Some(key.into()),
            name_line,
            value_line,
            type_line,
        }
    }

    pub fn attribute(
        key: impl Into<String>,
        cells: (Line<'static>, Line<'static>, Line<'static>),
    ) -> Self {
        let (name_line, value_line, type_line) = cells;
        Self {
            kind: MetadataRowKind::Attribute,
            key: Some(key.into()),
            name_line,
            value_line,
            type_line,
        }
    }

    pub fn is_selectable(&self) -> bool {
        !matches!(self.kind, MetadataRowKind::SectionHeader)
    }
}

#[derive(Debug)]
pub struct ComputedAttributes {
    pub longest_name_length: u16,
    #[allow(dead_code)]
    pub attributes: Vec<(String, Attribute)>,
    pub rendered_rows: Vec<RenderedAttributeRow>,
}

impl ComputedAttributes {
    pub fn new(node: &Node) -> Result<Self, hdf5_metno::Error> {
        let attributes = node.attributes()?;
        let longest_name_length = attributes
            .iter()
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(0)
            .max(
                SYSTEM_PROPERTIES
                    .iter()
                    .map(|name| name.len())
                    .max()
                    .unwrap_or(0),
            ) as u16;

        let name_area_width = longest_name_length + 3;
        let property_rows = std::iter::once(node.render(name_area_width))
            .chain(match node {
                Node::Dataset(_, ds) => ds.render(name_area_width),
                Node::Group(_, grp_meta) => grp_meta.render(name_area_width),
                _ => vec![],
            })
            .collect::<Vec<_>>();

        let rendered_custom_attributes =
            Self::render_attributes(&attributes, name_area_width as usize);
        let mut rendered_rows = vec![RenderedAttributeRow::section("Properties")];
        rendered_rows.extend(property_rows);
        if !rendered_custom_attributes.is_empty() {
            rendered_rows.push(RenderedAttributeRow::section("Attributes"));
            rendered_rows.extend(rendered_custom_attributes);
        }

        Ok(Self {
            longest_name_length,
            attributes,
            rendered_rows,
        })
    }

    fn render_attributes(
        attributes: &Vec<(String, Attribute)>,
        name_area_width: usize,
    ) -> Vec<RenderedAttributeRow> {
        let mut rendered_attributes = vec![];
        for (name, attr) in attributes {
            let name = name.to_string();
            let name_len = name.len();
            let name_styled = Span::styled(
                name.clone(),
                Style::default()
                    .fg(configure::themed_color(|colors| {
                        colors.metadata.attribute_name
                    }))
                    .bold(),
            );
            let extra_name_space = name_area_width - name_len;
            let name_helper_line = Span::styled(
                configure::configured_symbol(|symbols| symbols.tree.horizontal_rule)
                    .repeat(extra_name_space - 1),
                Style::default().fg(configure::themed_color(|colors| colors.tree.lines)),
            );
            let equals_sign = Span::styled(
                "=",
                Style::default().fg(configure::themed_color(|colors| colors.accent.equal_sign)),
            );
            let name_line = Line::from(vec![name_styled, name_helper_line, equals_sign]);

            let value_line = match sprint_attribute(attr) {
                Ok(l) => l,
                Err(e) => Line::styled(
                    format!("Error: {}", e),
                    Style::default().fg(configure::themed_color(|colors| colors.text.error)),
                ),
            };
            let type_desc_str = match attribute_type_description(attr) {
                Ok(type_desc) => type_desc,
                Err(e) => format!("Error getting type descriptor: {}", e),
            };
            let type_desc = Line::styled(
                format!(" ({})", type_desc_str),
                Style::default().fg(if type_desc_str.starts_with("opaque[") {
                    configure::themed_color(|colors| colors.text.opaque)
                } else {
                    configure::themed_color(|colors| colors.text.type_desc)
                }),
            );

            rendered_attributes.push(RenderedAttributeRow::attribute(
                name,
                (name_line, value_line, type_desc),
            ));
        }
        rendered_attributes
    }

    pub fn row_count(&self) -> usize {
        self.rendered_rows.len()
    }

    pub fn row(&self, row_index: usize) -> Option<&RenderedAttributeRow> {
        self.rendered_rows.get(row_index)
    }

    pub fn normalize_row_index(&self, row_index: usize) -> Option<usize> {
        let capped_index = row_index.min(self.rendered_rows.len().saturating_sub(1));
        self.rendered_rows
            .get(capped_index)
            .filter(|row| row.is_selectable())
            .map(|_| capped_index)
            .or_else(|| {
                self.rendered_rows
                    .iter()
                    .enumerate()
                    .skip(capped_index.saturating_add(1))
                    .find(|(_, row)| row.is_selectable())
                    .map(|(index, _)| index)
            })
            .or_else(|| {
                self.rendered_rows
                    .iter()
                    .enumerate()
                    .take(capped_index)
                    .rev()
                    .find(|(_, row)| row.is_selectable())
                    .map(|(index, _)| index)
            })
    }
}

impl H5FNode {
    pub fn normalize_attribute_selection(&mut self) -> Result<Option<usize>, hdf5_metno::Error> {
        let current_index = self.attributes_view_cursor.attribute_index;
        let normalized_index = self.read_attributes()?.normalize_row_index(current_index);
        if let Some(index) = normalized_index {
            self.attributes_view_cursor.attribute_index = index;
        } else {
            self.attributes_view_cursor.attribute_index = 0;
        }
        Ok(normalized_index)
    }

    fn rendered_attribute_index(&mut self, attr_name: &str) -> Result<usize, AppError> {
        let attributes = self.read_attributes()?;
        attributes
            .rendered_rows
            .iter()
            .position(|row| row.key.as_deref() == Some(attr_name))
            .ok_or_else(|| AppError::EditError(format!("Attribute '{}' not found", attr_name)))
    }

    pub fn create_attribute(
        &mut self,
        attr_name: &str,
        attr_type: AttributeCreateType,
        value: &str,
    ) -> Result<String, AppError> {
        let attr_name = validate_user_attribute_name(attr_name)?;
        let created_type = self.node.create_attr(&attr_name, attr_type, value)?;
        self.recompute_attributes()?;
        self.attributes_view_cursor.attribute_index = self.rendered_attribute_index(&attr_name)?;
        self.attributes_view_cursor.attribute_view_selection = AttributeViewSelection::Value;
        Ok(created_type)
    }

    pub fn delete_attribute(&mut self, attr_name: &str) -> Result<(), AppError> {
        let attr_name = validate_user_attribute_name(attr_name)?;
        let current_index = self.attributes_view_cursor.attribute_index;
        let deleted_index = self.rendered_attribute_index(&attr_name)?;
        self.node.delete_attr(&attr_name)?;
        self.recompute_attributes()?;
        let len = self.read_attributes()?.row_count();
        self.attributes_view_cursor.attribute_index = if len == 0 {
            0
        } else if deleted_index < current_index {
            current_index.saturating_sub(1).min(len - 1)
        } else {
            current_index.min(len - 1)
        };
        self.normalize_attribute_selection()?;
        Ok(())
    }

    pub fn update_attribute_name(
        &mut self,
        attr_name: &str,
        new_name: &str,
    ) -> Result<(), AppError> {
        let new_name = validate_user_attribute_name(new_name)?;
        if !attr_name.eq(&new_name) {
            self.node.update_attr_name(attr_name, &new_name)?;
        }
        self.recompute_attributes()?;
        Ok(())
    }

    pub fn update_attribute(&mut self, attr_name: &str, new_value: String) -> Result<(), AppError> {
        let attr = self.node.attribute(attr_name)?;
        write_attr_from_text(&attr, &new_value)?;
        self.recompute_attributes()?;
        Ok(())
    }

    pub fn rewrite_fixed_string_attribute(
        &mut self,
        attr_name: &str,
        new_value: &str,
        rewrite: FixedStringRewrite,
    ) -> Result<(), AppError> {
        {
            let group = node_attribute_group(&self.node)?;
            let attr = self.node.attribute(attr_name)?;
            rewrite_fixed_string_attr(&group, &attr, attr_name, new_value, rewrite)?;
        }
        self.recompute_attributes()?;
        Ok(())
    }

    pub fn read_attributes(&mut self) -> Result<&ComputedAttributes, hdf5_metno::Error> {
        match self.computed_attributes {
            Some(ref computed_attributes) => Ok(computed_attributes),
            None => {
                let computed_attributes = ComputedAttributes::new(&self.node)?;
                self.computed_attributes = Some(computed_attributes);
                self.computed_attributes
                    .as_ref()
                    .ok_or_else(|| hdf5_metno::Error::from("Failed to read attributes".to_string()))
            }
        }
    }

    pub fn recompute_attributes(&mut self) -> Result<(), hdf5_metno::Error> {
        self.computed_attributes = Some(ComputedAttributes::new(&self.node)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ComputedAttributes, RenderedAttributeRow};
    use ratatui::text::Line;

    fn line(text: &str) -> Line<'static> {
        Line::from(text.to_string())
    }

    fn test_rows() -> ComputedAttributes {
        ComputedAttributes {
            longest_name_length: 8,
            attributes: vec![],
            rendered_rows: vec![
                RenderedAttributeRow::section("Properties"),
                RenderedAttributeRow::property("path", (line("path"), line("/"), line(""))),
                RenderedAttributeRow::section("Attributes"),
                RenderedAttributeRow::attribute("units", (line("units"), line("m"), line("(str)"))),
            ],
        }
    }

    #[test]
    fn normalize_row_index_skips_section_headers() {
        let rows = test_rows();

        assert_eq!(rows.normalize_row_index(0), Some(1));
        assert_eq!(rows.normalize_row_index(2), Some(3));
        assert_eq!(rows.normalize_row_index(3), Some(3));
    }
}
