use hdf5_metno::Attribute;
use ratatui::{
    style::Style,
    text::{Line, Span, ToSpan},
};

use crate::{color_consts, error::AppError, sprint_attributes::sprint_attribute};

use super::{
    codec::{copy_attr_to_group, write_scalar_attr_from_text},
    model::{H5FNode, Node},
};

pub trait HasAttributes {
    fn attribute(&self, name: &str) -> Result<Attribute, hdf5_metno::Error>;
    fn attribute_names(&self) -> Result<Vec<String>, hdf5_metno::Error>;
    fn attributes(&self) -> Result<Vec<(String, Attribute)>, hdf5_metno::Error>;
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
            Node::Dataset(dataset, _) => dataset.name(),
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

    fn update_attr_name(&self, old_name: &str, new_name: &str) -> Result<(), AppError> {
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
        copy_attr_to_group(&attr, &group, new_name)?;
        group.delete_attr(old_name)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct ComputedAttributes {
    pub longest_name_length: u16,
    #[allow(dead_code)]
    pub attributes: Vec<(String, Attribute)>,
    pub rendered_attributes: Vec<(Line<'static>, Line<'static>, Line<'static>)>,
}

impl ComputedAttributes {
    pub fn new(node: &Node) -> Result<Self, hdf5_metno::Error> {
        let attributes = node.attributes()?;
        let longest_name_length = attributes
            .iter()
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(0) as u16;

        let name_area_width = longest_name_length + 3;
        let path_attr = node.render(name_area_width);

        let rendered_ds_attributes = match node {
            Node::Dataset(_, ds) => ds.render(name_area_width),
            Node::Group(_, grp_meta) => grp_meta.render(name_area_width),
            _ => vec![],
        };

        let rendered_custom_attributes =
            Self::render_attributes(&attributes, name_area_width as usize);
        let rendered_attributes = vec![path_attr]
            .into_iter()
            .chain(rendered_ds_attributes)
            .chain(rendered_custom_attributes)
            .collect::<Vec<(Line<'static>, Line<'static>, Line<'static>)>>();

        Ok(Self {
            longest_name_length,
            attributes,
            rendered_attributes,
        })
    }

    fn update_value_inplace(
        &mut self,
        attr_name: &str,
        new_value: String,
        typedesc: String,
    ) -> Result<(), AppError> {
        for (name_line, value_line, type_desc) in &mut self.rendered_attributes {
            if name_line.to_span().to_string().starts_with(attr_name) {
                let first_span = value_line.spans.get_mut(0).ok_or_else(|| {
                    AppError::EditError(format!(
                        "Value line for attribute '{}' has no spans",
                        attr_name
                    ))
                })?;
                first_span.content = new_value.into();

                let new_type_desc_line = Line::from(vec![Span::styled(
                    format!(" ({})", typedesc),
                    Style::default().fg(color_consts::TYPE_DESC_COLOR),
                )]);
                *type_desc = new_type_desc_line;
                break;
            }
        }
        Ok(())
    }

    fn render_attributes(
        attributes: &Vec<(String, Attribute)>,
        name_area_width: usize,
    ) -> Vec<(Line<'static>, Line<'static>, Line<'static>)> {
        let mut rendered_attributes = vec![];
        for (name, attr) in attributes {
            let name = name.to_string();
            let name_len = name.len();
            let name_styled = Span::styled(
                name,
                Style::default().fg(color_consts::VARIABLE_BLUE).bold(),
            );
            let extra_name_space = name_area_width - name_len;
            let name_helper_line = Span::styled(
                "─".repeat(extra_name_space - 1),
                Style::default().fg(color_consts::LINES_COLOR),
            );
            let equals_sign = Span::styled("=", Style::default().fg(color_consts::EQUAL_SIGN_COLOR));
            let name_line = Line::from(vec![name_styled, name_helper_line, equals_sign]);

            let value_line = match sprint_attribute(attr) {
                Ok(l) => l,
                Err(e) => Line::styled(
                    format!("Error: {}", e),
                    Style::default().fg(color_consts::ERROR_COLOR),
                ),
            };
            let type_desc_str = match attr.dtype() {
                Ok(dtype) => match dtype.to_descriptor() {
                    Ok(td) => td.to_string(),
                    Err(e) => format!("Error getting type descriptor: {}", e),
                },
                Err(e) => format!("Error getting dtype: {}", e),
            };
            let type_desc = Line::styled(
                format!(" ({})", type_desc_str),
                Style::default().fg(color_consts::TYPE_DESC_COLOR),
            );

            rendered_attributes.push((name_line, value_line, type_desc));
        }
        rendered_attributes
    }
}

impl H5FNode {
    pub fn update_attribute_name(&mut self, attr_name: &str, new_name: &str) -> Result<(), AppError> {
        let new_name = new_name.trim_matches('=').trim_matches('─').trim();
        if !attr_name.eq(new_name) {
            self.node.update_attr_name(attr_name, new_name)?;
        }
        self.recompute_attributes()?;
        Ok(())
    }

    pub fn update_attribute(&mut self, attr_name: &str, new_value: String) -> Result<(), AppError> {
        let attr = self.node.attribute(attr_name)?;
        if !attr.is_scalar() {
            return Err(AppError::EditError(
                "Only scalar attributes can be edited".to_string(),
            ));
        }
        let type_desc = write_scalar_attr_from_text(&attr, &new_value)?;
        match &mut self.computed_attributes {
            Some(computed_attributes) => {
                computed_attributes.update_value_inplace(
                    attr_name,
                    new_value,
                    type_desc,
                )?;
            }
            None => {
                Err(AppError::EditError(
                    "Failed to update attribute view: Computed attributes not found".to_string(),
                ))?;
            }
        }

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
