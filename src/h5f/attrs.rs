use std::str::FromStr;

use hdf5_metno::{
    types::{FixedAscii, FixedUnicode, TypeDescriptor, VarLenAscii, VarLenUnicode},
    Attribute, Group, H5Type, ObjectReference2,
};
use ndarray::IxDyn;
use ratatui::{
    style::Style,
    text::{Line, Span, ToSpan},
};

use crate::{color_consts, error::AppError, sprint_attributes::sprint_attribute};

use super::model::{H5FNode, Node};

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
        let type_desc = attr.dtype()?.to_descriptor()?;
        match type_desc {
            TypeDescriptor::Boolean => copy_to_group::<bool>(&attr, &group, &type_desc, new_name)?,
            TypeDescriptor::Integer(int_size) => match int_size {
                hdf5_metno::types::IntSize::U1 => {
                    copy_to_group::<i8>(&attr, &group, &type_desc, new_name)?
                }
                hdf5_metno::types::IntSize::U2 => {
                    copy_to_group::<i16>(&attr, &group, &type_desc, new_name)?
                }
                hdf5_metno::types::IntSize::U4 => {
                    copy_to_group::<i32>(&attr, &group, &type_desc, new_name)?
                }
                hdf5_metno::types::IntSize::U8 => {
                    copy_to_group::<i64>(&attr, &group, &type_desc, new_name)?
                }
            },
            TypeDescriptor::Unsigned(int_size) => match int_size {
                hdf5_metno::types::IntSize::U1 => {
                    copy_to_group::<u8>(&attr, &group, &type_desc, new_name)?
                }
                hdf5_metno::types::IntSize::U2 => {
                    copy_to_group::<u16>(&attr, &group, &type_desc, new_name)?
                }
                hdf5_metno::types::IntSize::U4 => {
                    copy_to_group::<u32>(&attr, &group, &type_desc, new_name)?
                }
                hdf5_metno::types::IntSize::U8 => {
                    copy_to_group::<u64>(&attr, &group, &type_desc, new_name)?
                }
            },
            TypeDescriptor::Float(float_size) => match float_size {
                hdf5_metno::types::FloatSize::U4 => {
                    copy_to_group::<f32>(&attr, &group, &type_desc, new_name)?
                }
                hdf5_metno::types::FloatSize::U8 => {
                    copy_to_group::<f64>(&attr, &group, &type_desc, new_name)?
                }
            },
            TypeDescriptor::Enum(_) => {
                let data: Vec<u8> = attr.read_raw()?;
                let new_attr = group
                    .new_attr_builder()
                    .empty_as(&type_desc)
                    .create(new_name)?;
                new_attr.write_raw(&data)?;
            }
            TypeDescriptor::Compound(_) => {
                let data: Vec<u8> = attr.read_raw()?;
                let new_attr = group
                    .new_attr_builder()
                    .empty_as(&type_desc)
                    .create(new_name)?;
                new_attr.write_raw(&data)?;
            }
            TypeDescriptor::Reference(_) => {
                let data: ObjectReference2 = attr.read_scalar()?;
                let new_attr = group
                    .new_attr_builder()
                    .empty_as(&type_desc)
                    .create(new_name)?;
                new_attr.write_scalar(&data)?;
            }
            TypeDescriptor::FixedUnicode(size) => match size {
                0..255 => copy_to_group::<FixedUnicode<255>>(&attr, &group, &type_desc, new_name)?,
                255..4096 => {
                    copy_to_group::<FixedUnicode<4096>>(&attr, &group, &type_desc, new_name)?
                }
                _ => copy_to_group::<VarLenUnicode>(&attr, &group, &type_desc, new_name)?,
            },
            TypeDescriptor::VarLenArray(_) => {
                return Err(AppError::EditError(
                    "Edit of VarLenArray types are unsupported".to_string(),
                ))
            }
            TypeDescriptor::FixedArray(_, _) => {
                return Err(AppError::EditError(
                    "Edit of FixedArray types are unsupported".to_string(),
                ))
            }
            TypeDescriptor::FixedAscii(size) => match size {
                0..255 => copy_to_group::<FixedAscii<255>>(&attr, &group, &type_desc, new_name)?,
                255..4096 => {
                    copy_to_group::<FixedAscii<4096>>(&attr, &group, &type_desc, new_name)?
                }
                _ => copy_to_group::<VarLenAscii>(&attr, &group, &type_desc, new_name)?,
            },
            TypeDescriptor::VarLenAscii => {
                copy_to_group::<VarLenAscii>(&attr, &group, &type_desc, new_name)?
            }
            TypeDescriptor::VarLenUnicode => {
                copy_to_group::<VarLenUnicode>(&attr, &group, &type_desc, new_name)?
            }
        }
        group.delete_attr(old_name)?;

        Ok(())
    }
}

fn copy_to_group<T: H5Type>(
    attr: &Attribute,
    grp: &Group,
    td: &TypeDescriptor,
    new_name: &str,
) -> Result<(), hdf5_metno::Error> {
    if attr.is_scalar() {
        let data: T = attr.read_scalar()?;
        let new_attr = grp.new_attr_builder().empty_as(td).create(new_name)?;
        new_attr.write_scalar(&data)?;
    } else {
        let data = attr.read::<T, IxDyn>()?;
        grp.new_attr_builder().with_data_as(&data, td).create(new_name)?;
    }
    Ok(())
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
        let type_desc = attr.dtype()?.to_descriptor()?;
        match type_desc {
            TypeDescriptor::Integer(int_size) => match int_size {
                hdf5_metno::types::IntSize::U1 => {
                    let int_value = i8::from_str(&new_value)
                        .map_err(|e| AppError::EditError(format!("Failed to convert to i8: {}", e)))?;
                    attr.write_scalar(&int_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::IntSize::U2 => {
                    let int_value = i16::from_str(&new_value)
                        .map_err(|e| AppError::EditError(format!("Failed to convert to i16: {}", e)))?;
                    attr.write_scalar(&int_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::IntSize::U4 => {
                    let int_value = i32::from_str(&new_value)
                        .map_err(|e| AppError::EditError(format!("Failed to convert to i32: {}", e)))?;
                    attr.write_scalar(&int_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::IntSize::U8 => {
                    let int_value = i64::from_str(&new_value)
                        .map_err(|e| AppError::EditError(format!("Failed to convert to i64: {}", e)))?;
                    attr.write_scalar(&int_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
            },
            TypeDescriptor::Unsigned(size) => match size {
                hdf5_metno::types::IntSize::U1 => {
                    let uint_value = u8::from_str(&new_value)
                        .map_err(|e| AppError::EditError(format!("Failed to convert to u8: {}", e)))?;
                    attr.write_scalar(&uint_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::IntSize::U2 => {
                    let uint_value = u16::from_str(&new_value)
                        .map_err(|e| AppError::EditError(format!("Failed to convert to u16: {}", e)))?;
                    attr.write_scalar(&uint_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::IntSize::U4 => {
                    let uint_value = u32::from_str(&new_value)
                        .map_err(|e| AppError::EditError(format!("Failed to convert to u32: {}", e)))?;
                    attr.write_scalar(&uint_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::IntSize::U8 => {
                    let uint_value = u64::from_str(&new_value)
                        .map_err(|e| AppError::EditError(format!("Failed to convert to u64: {}", e)))?;
                    attr.write_scalar(&uint_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
            },
            TypeDescriptor::Float(float_size) => match float_size {
                hdf5_metno::types::FloatSize::U4 => {
                    let float_value = f32::from_str(&new_value)
                        .map_err(|e| AppError::EditError(format!("Failed to convert to f32: {}", e)))?;
                    attr.write_scalar(&float_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::FloatSize::U8 => {
                    let float_value = f64::from_str(&new_value)
                        .map_err(|e| AppError::EditError(format!("Failed to convert to f64: {}", e)))?;
                    attr.write_scalar(&float_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
            },
            TypeDescriptor::Boolean => {
                let bool_value = bool::from_str(&new_value)
                    .map_err(|e| AppError::EditError(format!("Failed to convert to bool: {}", e)))?;
                attr.write_scalar(&bool_value).map_err(|e| {
                    AppError::EditError(format!("Failed to write attribute: {}", e))
                })?;
            }
            TypeDescriptor::Enum(_) => {
                return Err(AppError::EditError(
                    "Editing enum attributes is not supported".to_string(),
                ));
            }
            TypeDescriptor::Compound(_) => {
                return Err(AppError::EditError(
                    "Editing compound attributes is not supported".to_string(),
                ));
            }
            TypeDescriptor::FixedArray(_, _) => {
                return Err(AppError::EditError(
                    "Editing array attributes is not supported".to_string(),
                ));
            }
            TypeDescriptor::FixedAscii(_) => {
                return Err(AppError::EditWarning("Editing FixedAscii attributes is disabled due to performance and dependency concerns. \nIf you truly wish to edit this attribute, delete it and create it with desired type such as vlen string".to_string()));
            }
            TypeDescriptor::FixedUnicode(_) => {
                return Err(AppError::EditWarning("Editing FixedUnicode attributes is disabled due to performance and dependency concerns. \nIf you truly wish to edit this attribute, delete it and create it with desired type such as vlen string".to_string()));
            }
            TypeDescriptor::VarLenArray(_) => {
                return Err(AppError::EditError(
                    "Editing array attributes is not supported".to_string(),
                ));
            }
            TypeDescriptor::VarLenAscii => {
                let ascii = VarLenAscii::from_ascii(&new_value).map_err(|e| {
                    AppError::EditError(format!("Failed to convert to VarLenAscii: {}", e))
                })?;
                attr.write_scalar(&ascii).map_err(|e| {
                    AppError::EditError(format!("Failed to write attribute: {}", e))
                })?;
            }
            TypeDescriptor::VarLenUnicode => {
                let unicode = VarLenUnicode::from_str(&new_value).map_err(|e| {
                    AppError::EditError(format!("Failed to convert to VarLenUnicode: {}", e))
                })?;
                attr.write_scalar(&unicode).map_err(|e| {
                    AppError::EditError(format!("Failed to write attribute: {}", e))
                })?;
            }
            TypeDescriptor::Reference(_) => {
                return Err(AppError::EditError(
                    "Editing reference attributes is not supported".to_string(),
                ));
            }
        }
        match &mut self.computed_attributes {
            Some(computed_attributes) => {
                computed_attributes.update_value_inplace(
                    attr_name,
                    new_value,
                    type_desc.to_string(),
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
