use std::{cell::RefCell, rc::Rc, str::FromStr};

use hdf5_metno::{
    types::{FixedAscii, FixedUnicode, TypeDescriptor, VarLenAscii, VarLenUnicode},
    Attribute, Dataset, File, Group, H5Type, LinkType, ObjectReference2,
};
use ndarray::IxDyn;
use ratatui::{
    style::Style,
    text::{Line, Span, ToSpan},
};

use crate::{
    color_consts,
    error::AppError,
    sprint_attributes::sprint_attribute,
    sprint_typedesc::{
        encoding_from_dtype, is_image, is_type_matrixable, sprint_typedescriptor, MatrixRenderType,
    },
    ui::state::{AttributeCursor, ContentShowMode},
};

#[derive(Debug, Clone)]
pub enum Encoding {
    Unknown,
    LittleEndian,
    UTF8,
    Ascii,
    UTF8Fixed,
    AsciiFixed,
}

#[derive(Debug, Clone)]
pub enum InterlaceMode {
    Pixel, // [height][width][pixel components] -> value
    Plane, // [pixel components][height][width] -> value
}

#[derive(Debug, Clone)]
pub enum ImageType {
    Jpeg,
    Png,
    Grayscale,
    Bitmap,
    Truecolor(InterlaceMode),
    Indexed(InterlaceMode),
}
#[derive(Debug, Clone)]
pub struct GroupMeta {
    pub is_link: bool,
    pub filename: String,
    pub display_name: String,
}

#[derive(Debug, Clone)]
pub struct DatasetMeta {
    pub link_name: Option<String>,
    pub display_name: String,
    pub shape: Vec<usize>,
    pub data_type: String,
    #[allow(dead_code)]
    data_bytesize: usize,
    storage_required: u64,
    total_bytes: usize,
    total_elems: usize,
    chunk_shape: Option<Vec<usize>>,
    pub hl: Option<String>,
    pub matrixable: Option<MatrixRenderType>,
    pub encoding: Encoding,
    pub image: Option<ImageType>,
    pub is_link: bool,
    pub filename: String,
}
impl GroupMeta {
    pub fn render(&self, longest_name: u16) -> Vec<(Line<'static>, Line<'static>, Line<'static>)> {
        let min_first_panel = match longest_name {
            0..8 => 8,
            8..=u16::MAX => longest_name,
        };
        let mut data_set_attrs = vec![];

        if self.is_link {
            let external_filename = Span::styled(
                "link",
                Style::default()
                    .fg(color_consts::VARIABLE_BLUE_BUILTIN)
                    .bold(),
            );
            let external_value = Span::styled(
                self.filename.clone(),
                Style::default()
                    .fg(color_consts::BUILT_IN_VALUE_COLOR)
                    .bold(),
            );
            data_set_attrs.push((external_filename, external_value));
        }

        let mut lines: Vec<(Line<'static>, Line<'static>, Line<'static>)> = vec![];
        for (name, value) in data_set_attrs {
            let name_len = name.width();
            let extra_name_space = match min_first_panel as usize - name_len {
                0..=1 => 1,
                _ => min_first_panel as usize - name_len,
            };
            let name_helper_line = Span::styled(
                "─".repeat(extra_name_space - 1),
                Style::default().fg(color_consts::LINES_COLOR),
            );
            let equals_sign =
                Span::styled("=", Style::default().fg(color_consts::EQUAL_SIGN_COLOR));
            let name_line = Line::from(vec![name, name_helper_line, equals_sign]);

            let value_line = Line::from(vec![value]);
            let empty_line = Line::from(vec![Span::raw("")]);
            lines.push((name_line, value_line, empty_line));
        }

        lines
    }
}

// type, size, shape etc.
pub static SYSTEM_ATTRIBUTES: [&str; 6] = ["type", "size", "shape", "chunk", "link", "path"];

impl DatasetMeta {
    pub fn render(&self, longest_name: u16) -> Vec<(Line<'static>, Line<'static>, Line<'static>)> {
        let min_first_panel = match longest_name {
            0..8 => 8,
            8..=u16::MAX => longest_name,
        };
        let mut data_set_attrs = vec![];
        let type_name = Span::styled(
            "type",
            Style::default()
                .fg(color_consts::VARIABLE_BLUE_BUILTIN)
                .bold(),
        );
        let type_value = Span::styled(
            self.data_type_string(),
            Style::default()
                .fg(color_consts::BUILT_IN_VALUE_COLOR)
                .bold(),
        );
        data_set_attrs.push((type_name, type_value));

        let size_name = Span::styled(
            "size",
            Style::default()
                .fg(color_consts::VARIABLE_BLUE_BUILTIN)
                .bold(),
        );
        let size_value = Span::styled(
            self.size_string(),
            Style::default()
                .fg(color_consts::BUILT_IN_VALUE_COLOR)
                .bold(),
        );
        data_set_attrs.push((size_name, size_value));
        let shape_name = Span::styled(
            "shape",
            Style::default()
                .fg(color_consts::VARIABLE_BLUE_BUILTIN)
                .bold(),
        );
        let shape_value = Span::styled(
            self.shape_string(),
            Style::default()
                .fg(color_consts::BUILT_IN_VALUE_COLOR)
                .bold(),
        );
        data_set_attrs.push((shape_name, shape_value));
        if let Some(chunk_shape) = &self.chunk_shape_string() {
            let chunk_name = Span::styled(
                "chunk",
                Style::default()
                    .fg(color_consts::VARIABLE_BLUE_BUILTIN)
                    .bold(),
            );
            let chunk_value = Span::styled(
                chunk_shape.to_string(),
                Style::default()
                    .fg(color_consts::BUILT_IN_VALUE_COLOR)
                    .bold(),
            );
            data_set_attrs.push((chunk_name, chunk_value));
        }

        if self.is_link {
            let external_filename = Span::styled(
                "link",
                Style::default()
                    .fg(color_consts::VARIABLE_BLUE_BUILTIN)
                    .bold(),
            );
            let external_value = Span::styled(
                self.filename.clone(),
                Style::default()
                    .fg(color_consts::BUILT_IN_VALUE_COLOR)
                    .bold(),
            );
            data_set_attrs.push((external_filename, external_value));
        }
        if let Some(l_name) = &self.link_name {
            let link_name_span = Span::styled(
                "origin",
                Style::default()
                    .fg(color_consts::VARIABLE_BLUE_BUILTIN)
                    .bold(),
            );
            let link_value_span = Span::styled(
                l_name.clone(),
                Style::default()
                    .fg(color_consts::BUILT_IN_VALUE_COLOR)
                    .bold(),
            );
            data_set_attrs.push((link_name_span, link_value_span));
        }

        let mut lines: Vec<(Line<'static>, Line<'static>, Line<'static>)> = vec![];
        for (name, value) in data_set_attrs {
            let name_len = name.width();
            let extra_name_space = match min_first_panel as usize - name_len {
                0..=1 => 1,
                _ => min_first_panel as usize - name_len,
            };
            let name_helper_line = Span::styled(
                "─".repeat(extra_name_space - 1),
                Style::default().fg(color_consts::LINES_COLOR),
            );
            let equals_sign =
                Span::styled("=", Style::default().fg(color_consts::EQUAL_SIGN_COLOR));
            let name_line = Line::from(vec![name, name_helper_line, equals_sign]);

            let value_line = Line::from(vec![value]);
            let empty_line = Line::from(vec![Span::raw("")]);
            lines.push((name_line, value_line, empty_line));
        }

        lines
    }
}

pub trait HasAttributes {
    fn attribute(&self, name: &str) -> Result<Attribute, hdf5_metno::Error>;
    fn attribute_names(&self) -> Result<Vec<String>, hdf5_metno::Error>;
    fn attributes(&self) -> Result<Vec<(String, Attribute)>, hdf5_metno::Error>;
    fn update_attr_name(&self, old_name: &str, new_name: &str) -> Result<(), AppError>;
    fn as_group(&self) -> Result<Group, hdf5_metno::Error>;
}

pub trait HasChildren {
    fn get_soft_groups(&self) -> Result<Vec<Group>, hdf5_metno::Error>;
    fn get_hard_groups(&self) -> Result<Vec<Group>, hdf5_metno::Error>;
    fn get_hard_datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error>;
    fn get_externals(&self) -> Result<Vec<ExternalObject>, hdf5_metno::Error>;
    fn get_soft_datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error>;
}

#[derive(Debug)]
pub enum ExternalObject {
    Dataset(Dataset),
    Group(Group),
    LinkBroken(String, String),
}

impl HasChildren for Group {
    fn get_soft_groups(&self) -> Result<Vec<Group>, hdf5_metno::Error> {
        let soft_groups = self.iter_visit_default(vec![], |group, name, link, objects| {
            if LinkType::Soft == link.link_type {
                match group.group(name) {
                    Ok(g) => objects.push(g),
                    Err(_) => {
                        // Ignore it and move on
                        return true;
                    }
                }
            }
            true
        })?;
        Ok(soft_groups)
    }

    fn get_soft_datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error> {
        let soft_datasets = self.iter_visit_default(vec![], |group, name, link, objects| {
            if LinkType::Soft == link.link_type {
                match group.dataset(name) {
                    Ok(ds) => objects.push(ds),
                    Err(_) => {
                        // Ignore it and move on
                        return true;
                    }
                }
            }
            true
        })?;
        Ok(soft_datasets)
    }

    fn get_hard_groups(&self) -> Result<Vec<Group>, hdf5_metno::Error> {
        let hard_groups = self.iter_visit_default(vec![], |group, name, link, objects| {
            if LinkType::Hard == link.link_type {
                match group.group(name) {
                    Ok(g) => objects.push(g),
                    Err(_) => {
                        // Ignore it and move on
                        return true;
                    }
                }
            }
            true
        })?;
        Ok(hard_groups)
    }

    fn get_hard_datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error> {
        let datasets = self.iter_visit_default(vec![], |group, name, link, objects| {
            if LinkType::Hard == link.link_type {
                match group.dataset(name) {
                    Ok(ds) => objects.push(ds),
                    Err(_) => {
                        // Ignore it and move on
                        return true;
                    }
                }
            }
            true
        })?;
        Ok(datasets)
    }

    fn get_externals(&self) -> Result<Vec<ExternalObject>, hdf5_metno::Error> {
        let external_datasets = self.iter_visit_default(vec![], |group, name, link, objects| {
            if LinkType::External == link.link_type {
                if let Ok(ds) = group.dataset(name) {
                    let ds = ExternalObject::Dataset(ds);
                    objects.push(ds);
                } else if let Ok(grp) = group.group(name) {
                    let grp = ExternalObject::Group(grp);
                    objects.push(grp);
                } else {
                    let name = name.to_string();
                    let broken_link =
                        ExternalObject::LinkBroken(name, group.filename().to_string());
                    objects.push(broken_link);
                }
            }
            true
        })?;
        Ok(external_datasets)
    }
}

pub trait HasName {
    fn name(&self) -> String;
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
pub trait HasPath {
    fn path(&self) -> String;
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

    fn as_group(&self) -> Result<Group, hdf5_metno::Error> {
        match self {
            Node::File(file) => file.as_group(),
            Node::Group(group, _) => group.as_group(),
            Node::Dataset(dataset, _) => dataset.as_group(),
            Node::Broken(_, _, _) => Err(hdf5_metno::Error::Internal(String::from(
                "Cannot treat broken link as group",
            ))),
        }
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
        grp.new_attr_builder()
            .with_data_as(&data, td)
            .create(new_name)?;
    }
    Ok(())
}

impl DatasetMeta {
    pub fn shape_string(&self) -> String {
        let dims_str = self
            .shape
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<String>>()
            .join(" x ");
        let dims_total_string = format!("{} = {}", dims_str, self.total_elems);
        dims_total_string
    }

    pub fn chunk_shape_string(&self) -> Option<String> {
        match &self.chunk_shape {
            Some(chunk_shape) => {
                let chunk_str = chunk_shape
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<String>>()
                    .join(" x ");
                let chunk_total_string =
                    format!("{} = {}", chunk_str, chunk_shape.iter().product::<usize>());
                Some(chunk_total_string)
            }
            None => None,
        }
    }

    pub fn data_type_string(&self) -> String {
        self.data_type.clone()
    }

    pub fn size_string(&self) -> String {
        let size = self.total_bytes;
        let total_storage = self.storage_required;
        let size_str = match size {
            0..1024 => format!("{} B", size),
            1024..1048576 => format!("{:.2} KB", size as f64 / 1024.0),
            1048576..1073741824 => format!("{:.2} MB", size as f64 / 1024.0 / 1024.0),
            _ => format!("{:.2} GB", size as f64 / 1024.0 / 1024.0 / 1024.0),
        };
        let total_storage_str = match total_storage {
            0..1024 => format!("{} B", total_storage),
            1024..1048576 => format!("{:.2} KB", total_storage as f64 / 1024.0),
            1048576..1073741824 => format!("{:.2} MB", total_storage as f64 / 1024.0 / 1024.0),
            _ => format!("{:.2} GB", total_storage as f64 / 1024.0 / 1024.0 / 1024.0),
        };
        format!("{} ({})", size_str, total_storage_str)
    }
}

#[derive(Debug, Clone)]
pub enum NodeType {
    Dataset,
    Group,
}

#[derive(Debug, Clone)]
pub enum Node {
    File(File),
    Group(Group, GroupMeta),
    Dataset(Dataset, DatasetMeta),
    Broken(NodeType, String, String),
}

impl Node {
    pub fn render(&self, longest_name: u16) -> (Line<'static>, Line<'static>, Line<'static>) {
        let min_first_panel = match longest_name {
            0..8 => 8,
            8..=u16::MAX => longest_name,
        };
        let path = self.path();
        let name_styled = Span::styled(
            "path",
            Style::default()
                .fg(color_consts::VARIABLE_BLUE_BUILTIN)
                .bold(),
        );
        let extra_name_space = min_first_panel as usize - "path".len();
        let name_helper_line = Span::styled(
            "─".repeat(extra_name_space - 1),
            Style::default().fg(color_consts::LINES_COLOR),
        );
        let equals_sign = Span::styled("=", Style::default().fg(color_consts::EQUAL_SIGN_COLOR));
        let name_line = Line::from(vec![name_styled, name_helper_line, equals_sign]);
        let path_styled = Span::styled(
            path,
            Style::default()
                .fg(color_consts::BUILT_IN_VALUE_COLOR)
                .bold(),
        );
        let path_line = Line::from(vec![path_styled]);
        let empty_line = Line::from(vec![Span::raw("")]);
        (name_line, path_line, empty_line)
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
                // Set content of value_line to new_value, but keep the style
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
            let equals_sign =
                Span::styled("=", Style::default().fg(color_consts::EQUAL_SIGN_COLOR));
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
            // Make a small grey (type) string at the end of the value line
            let type_desc = Line::styled(
                format!(" ({})", type_desc_str),
                Style::default().fg(color_consts::TYPE_DESC_COLOR),
            );

            rendered_attributes.push((name_line, value_line, type_desc));
        }
        rendered_attributes
    }
}

#[derive(Debug)]
pub struct H5FNode {
    pub display_name: Option<String>,
    pub expanded: bool,
    pub node: Node,
    pub computed_attributes: Option<ComputedAttributes>,
    pub attributes_view_cursor: AttributeCursor,
    pub read: bool,
    pub children: Vec<Rc<RefCell<H5FNode>>>,
    pub view_loaded: u32,
    pub selected_dim: usize,
    pub selected_x: usize,
    pub selected_row: usize,
    pub selected_col: usize,
    pub line_offset: usize,
    pub col_offset: isize,
    pub selected_indexes: [usize; 15], // WARN: Will we ever need more than 15 dimensions?
}

pub enum DSType {
    Soft(Dataset),
    Hard(Dataset),
    External(Dataset),
    BrokenLink(String, String),
}

pub enum GrpType {
    Soft(Group),
    Hard(Group),
    External(Group),
}

impl H5FNode {
    pub fn new(node_type: Node) -> Self {
        Self {
            display_name: None,
            expanded: false,
            attributes_view_cursor: Default::default(),
            node: node_type,
            read: false,
            children: vec![],
            view_loaded: 50,
            computed_attributes: None,
            selected_dim: 0,
            selected_x: 0,
            selected_row: 0,
            selected_col: 1,
            line_offset: 0,
            col_offset: 0,
            selected_indexes: [0; 15], // WARN: Will we ever need more than 15 dimensions?
        }
    }

    pub fn update_attribute_name(
        &mut self,
        attr_name: &str,
        new_name: &str,
    ) -> Result<(), AppError> {
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
                    let int_value = i8::from_str(&new_value).map_err(|e| {
                        AppError::EditError(format!("Failed to convert to i8: {}", e))
                    })?;
                    attr.write_scalar(&int_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::IntSize::U2 => {
                    let int_value = i16::from_str(&new_value).map_err(|e| {
                        AppError::EditError(format!("Failed to convert to i16: {}", e))
                    })?;
                    attr.write_scalar(&int_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::IntSize::U4 => {
                    let int_value = i32::from_str(&new_value).map_err(|e| {
                        AppError::EditError(format!("Failed to convert to i32: {}", e))
                    })?;
                    attr.write_scalar(&int_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::IntSize::U8 => {
                    let int_value = i64::from_str(&new_value).map_err(|e| {
                        AppError::EditError(format!("Failed to convert to i64: {}", e))
                    })?;
                    attr.write_scalar(&int_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
            },
            TypeDescriptor::Unsigned(size) => match size {
                hdf5_metno::types::IntSize::U1 => {
                    let uint_value = u8::from_str(&new_value).map_err(|e| {
                        AppError::EditError(format!("Failed to convert to u8: {}", e))
                    })?;
                    attr.write_scalar(&uint_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::IntSize::U2 => {
                    let uint_value = u16::from_str(&new_value).map_err(|e| {
                        AppError::EditError(format!("Failed to convert to u16: {}", e))
                    })?;
                    attr.write_scalar(&uint_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::IntSize::U4 => {
                    let uint_value = u32::from_str(&new_value).map_err(|e| {
                        AppError::EditError(format!("Failed to convert to u32: {}", e))
                    })?;
                    attr.write_scalar(&uint_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::IntSize::U8 => {
                    let uint_value = u64::from_str(&new_value).map_err(|e| {
                        AppError::EditError(format!("Failed to convert to u64: {}", e))
                    })?;
                    attr.write_scalar(&uint_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
            },
            TypeDescriptor::Float(float_size) => match float_size {
                hdf5_metno::types::FloatSize::U4 => {
                    let float_value = f32::from_str(&new_value).map_err(|e| {
                        AppError::EditError(format!("Failed to convert to f32: {}", e))
                    })?;
                    attr.write_scalar(&float_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
                hdf5_metno::types::FloatSize::U8 => {
                    let float_value = f64::from_str(&new_value).map_err(|e| {
                        AppError::EditError(format!("Failed to convert to f64: {}", e))
                    })?;
                    attr.write_scalar(&float_value).map_err(|e| {
                        AppError::EditError(format!("Failed to write attribute: {}", e))
                    })?;
                }
            },
            TypeDescriptor::Boolean => {
                let bool_value = bool::from_str(&new_value).map_err(|e| {
                    AppError::EditError(format!("Failed to convert to bool: {}", e))
                })?;
                attr.write_scalar(&bool_value).map_err(|e| {
                    AppError::EditError(format!("Failed to write attribute: {}", e))
                })?;
            }
            TypeDescriptor::Enum(_) => {
                // TODO: Support enums
                return Err(AppError::EditError(
                    "Editing enum attributes is not supported".to_string(),
                ));
            }
            TypeDescriptor::Compound(_) => {
                // TODO: Support compounds maybe, through json interfacing or similar?
                return Err(AppError::EditError(
                    "Editing compound attributes is not supported".to_string(),
                ));
            }
            TypeDescriptor::FixedArray(_, _) => {
                // TODO: Support arrays.
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
                //TODO: Support arrays.
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

    pub fn icon(&self) -> String {
        if let Node::Broken(_, _, _) = &self.node {
            return "*- ".to_string();
        }
        match self.is_group() {
            // true => " ".to_string(),
            true => {
                let Node::Group(_, meta) = &self.node else {
                    return "?".to_string();
                };
                if meta.is_link {
                    "🔗".to_string()
                } else {
                    " ".to_string()
                }
            }
            false => {
                let Node::Dataset(_, meta) = &self.node else {
                    return "? ".to_string();
                };
                if meta.is_link {
                    // Dont do a file icon, just a link icon
                    "󰈚🔗".to_string()
                } else {
                    "󰈚 ".to_string()
                }
            }
        }
    }

    pub fn content_show_modes(&self) -> Vec<ContentShowMode> {
        let mut result = vec![];

        match &self.node {
            Node::File(_) => {}
            Node::Broken(_, _, _) => {}
            Node::Group(_, _) => {}
            Node::Dataset(_, dataset_meta) => match dataset_meta.matrixable {
                Some(matrix_renderable) => match matrix_renderable {
                    MatrixRenderType::Float64 => {
                        if dataset_meta.shape.iter().any(|x| *x > 1) {
                            result.push(ContentShowMode::Matrix);
                        }
                        result.push(ContentShowMode::Preview);
                    }
                    MatrixRenderType::Uint64 => {
                        if dataset_meta.shape.iter().any(|x| *x > 1) {
                            result.push(ContentShowMode::Matrix);
                        }
                        result.push(ContentShowMode::Preview);
                    }
                    MatrixRenderType::Int64 => {
                        if dataset_meta.shape.iter().any(|x| *x > 1) {
                            result.push(ContentShowMode::Matrix);
                        }
                        result.push(ContentShowMode::Preview);
                    }
                    MatrixRenderType::Compound => {
                        if dataset_meta.shape.iter().any(|x| *x > 1) {
                            result.push(ContentShowMode::Matrix);
                        }
                    }
                    MatrixRenderType::Strings => {
                        if dataset_meta.shape.iter().any(|x| *x > 1) {
                            result.push(ContentShowMode::Matrix);
                        }
                        result.push(ContentShowMode::Preview);
                    }
                },
                None => result.push(ContentShowMode::Preview),
            },
        }
        result
    }

    pub fn is_group(&self) -> bool {
        matches!(self.node, Node::Group(_, _))
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

    pub fn expand(&mut self) -> Result<(), hdf5_metno::Error> {
        self.read_children()?;
        if self.expanded {
            self.expanded = false;
            self.view_loaded = 50;
            return Ok(());
        }
        self.expanded = true;

        for child in &self.children {
            let mut child_node = child.borrow_mut();
            if child_node.is_group() {
                child_node.read_children()?;
            }
        }
        Ok(())
    }

    pub fn collapse(&mut self) {
        self.expanded = false;
    }

    pub fn expand_toggle(&mut self) -> Result<(), hdf5_metno::Error> {
        if self.expanded {
            self.collapse();
        } else {
            self.expand()?;
        }
        Ok(())
    }

    pub fn full_path(&self) -> String {
        if let Some(ref name) = self.display_name {
            return name.clone();
        }
        match &self.node {
            Node::File(f) => f.filename().split("/").last().unwrap_or("").to_string(),
            Node::Group(g, _) => g.filename().split("/").last().unwrap_or("").to_string(),
            Node::Dataset(ds, _) => ds.filename().split("/").last().unwrap_or("").to_string(),
            Node::Broken(_t, path, _fname) => path.clone(),
        }
    }

    pub fn name(&self) -> String {
        self.node.name()
    }

    pub fn expand_path(&mut self, relative_path: &str) -> Result<Option<usize>, AppError> {
        self.expand()?;
        let child_mame = relative_path.split('/').next();

        match child_mame {
            Some(n) => {
                for (i, child) in self.children.iter().enumerate() {
                    let child_name = match child.try_borrow() {
                        Ok(c) => c.name(),
                        Err(_) => return Ok(Some(i)),
                    };
                    if child_name == n {
                        let mut child_node = child.borrow_mut();
                        self.view_loaded = (i + 50) as u32;
                        if relative_path.len() > n.len() + 1 {
                            return child_node.expand_path(&relative_path[n.len() + 1..]);
                        }
                        return Ok(Some(i));
                    }
                }
                Err(AppError::ChildNotFound(relative_path.to_string()))
            }
            None => Ok(None),
        }
    }

    fn read_children(&mut self) -> Result<(), hdf5_metno::Error> {
        if self.read {
            return Ok(());
        }
        match self.node {
            Node::Dataset(_, _) => return Ok(()),
            Node::Broken(_, _, _) => return Ok(()),
            _ => {}
        }
        if matches!(self.node, Node::Dataset(_, _)) {
            return Ok(());
        }

        let has_children = match &self.node {
            Node::File(file) => file,
            Node::Group(group, _) => group,
            Node::Broken(_, _, _) => unreachable!("It should be guarded by the previous match"),
            Node::Dataset(_, _) => unreachable!("It should be guarded by the previous match"),
        };

        let mut groups = vec![];
        let mut datasets = vec![];
        for g in has_children.get_hard_groups()? {
            groups.push(GrpType::Hard(g));
        }

        for external in has_children.get_externals()? {
            match external {
                ExternalObject::Dataset(dataset) => datasets.push(DSType::External(dataset)),
                ExternalObject::Group(group) => groups.push(GrpType::External(group)),
                ExternalObject::LinkBroken(fname, name) => {
                    datasets.push(DSType::BrokenLink(fname, name))
                }
            }
        }
        for g in has_children.get_soft_groups()? {
            groups.push(GrpType::Soft(g));
        }
        for d in has_children.get_hard_datasets()? {
            datasets.push(DSType::Hard(d));
        }
        for d in has_children.get_soft_datasets()? {
            datasets.push(DSType::Soft(d));
        }

        let mut children = Vec::new();
        for wrapped_g in groups {
            let (g_maybe, is_link, broken) = match wrapped_g {
                GrpType::Hard(g) => (Some(g), false, None),
                GrpType::External(g) => (Some(g), true, None),
                GrpType::Soft(g) => (Some(g), true, None),
                // GrpType::BrokenLink(name, fname) => (None, true, Some((name, fname))),
            };
            if let Some((broken_name, broken_file)) = broken {
                let node = Rc::new(RefCell::new(H5FNode::new(Node::Broken(
                    NodeType::Group,
                    broken_name,
                    broken_file,
                ))));
                children.push(node);
                continue;
            }
            let Some(g) = g_maybe else {
                continue;
            };
            let display_name = g
                .name()
                .split('/')
                .next_back()
                .unwrap_or("Hidden")
                .to_string();

            let meta = GroupMeta {
                is_link,
                display_name,
                filename: g.filename().to_string(),
            };
            let node = Rc::new(RefCell::new(H5FNode::new(Node::Group(g, meta))));

            children.push(node);
        }
        for wrapped_ds in datasets {
            let (d, is_link, is_broken) = match wrapped_ds {
                DSType::Hard(ds) => (Some(ds), false, None),
                DSType::External(ds) => (Some(ds), true, None),
                DSType::Soft(ds) => (Some(ds), true, None),
                DSType::BrokenLink(name, fname) => (
                    None,
                    true,
                    Some(Node::Broken(NodeType::Dataset, name, fname)),
                ),
            };
            if let Some(broken_node) = is_broken {
                let node = Rc::new(RefCell::new(H5FNode::new(broken_node)));
                children.push(node);
                continue;
            }
            let d = match d {
                Some(ds) => ds,
                None => continue,
            };
            let display_name = d.name().split('/').next_back().unwrap_or("").to_string();
            // let member_names = d.as_group()?.member_names()?;

            let link_name = None; // TODO: Handle link names for datasets
            let d = d.to_owned();
            let dtype = d.dtype()?;
            let data_bytesize = dtype.size();
            let dtype_desc = dtype.to_descriptor()?;

            let mut shape = d.shape();
            let total_elems = d.size();
            if shape.is_empty() {
                shape.push(total_elems);
                shape.push(1);
            }
            let data_type = sprint_typedescriptor(&dtype_desc);
            let numerical = is_type_matrixable(&dtype_desc);
            let encoding = encoding_from_dtype(&dtype_desc);
            let total_bytes = data_bytesize * total_elems;
            let storage_required = d.storage_size();
            let chunk_shape = d.chunk();
            let image = is_image(&d);
            let filename = d.filename().to_string();
            let hl = d.attr("HIGHLIGHT").ok().map(|a| {
                a.read_scalar::<VarLenUnicode>()
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            });

            let meta = DatasetMeta {
                hl,
                shape,
                data_type,
                display_name,
                data_bytesize,
                total_bytes,
                storage_required,
                total_elems,
                link_name,
                chunk_shape,
                matrixable: numerical,
                encoding,
                image,
                is_link,
                filename,
            };
            let node_ds = Node::Dataset(d, meta);
            let node = Rc::new(RefCell::new(H5FNode::new(node_ds)));

            children.push(node);
        }
        self.children = children;
        Ok(())
    }
}

pub struct H5F {
    pub root: Rc<RefCell<H5FNode>>,
    pub file: File,
}

impl H5F {
    pub fn open(file_path: String, linked: bool, write: bool) -> Result<Self, hdf5_metno::Error> {
        let file = if write {
            hdf5_metno::file::File::open_rw(&file_path)?
        } else {
            hdf5_metno::file::File::open(&file_path)?
        };

        let member_count = file.member_names()?.len();
        let mut h5node = H5FNode::new(Node::File(file.clone()));
        if linked {
            h5node.display_name = Some(format!(" ({member_count}) linked ").to_string());
        }

        let root = Rc::new(RefCell::new(h5node));

        root.borrow_mut().read_children()?;
        root.borrow_mut().expand_toggle()?;

        let s = Self { root, file };
        Ok(s)
    }
}
