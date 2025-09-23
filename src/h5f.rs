use std::{cell::RefCell, rc::Rc};

use hdf5_metno::{types::VarLenUnicode, Attribute, Dataset, File, Group, LinkType};
use ratatui::{
    style::{Style, Stylize},
    text::{Line, Span},
};

use crate::{
    color_consts,
    sprint_attributes::sprint_attribute,
    sprint_typedesc::{
        encoding_from_dtype, is_image, is_type_matrixable, sprint_typedescriptor, MatrixRenderType,
    },
    ui::state::ContentShowMode,
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
    pub fn render(&self, longest_name: u16) -> Vec<(Line<'static>, Line<'static>)> {
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

        let mut lines: Vec<(Line<'static>, Line<'static>)> = vec![];
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
            lines.push((name_line, value_line));
        }

        lines
    }
}

impl DatasetMeta {
    pub fn render(&self, longest_name: u16) -> Vec<(Line<'static>, Line<'static>)> {
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

        let mut lines: Vec<(Line<'static>, Line<'static>)> = vec![];
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
            lines.push((name_line, value_line));
        }

        lines
    }
}

pub trait HasAttributes {
    fn attribute(&self, name: &str) -> Result<Attribute, hdf5_metno::Error>;
    fn attribute_names(&self) -> Result<Vec<String>, hdf5_metno::Error>;
    fn attributes(&self) -> Result<Vec<(String, Attribute)>, hdf5_metno::Error>;
}

pub trait HasChildren {
    fn get_soft_groups(&self) -> Result<Vec<Group>, hdf5_metno::Error>;
    fn get_hard_groups(&self) -> Result<Vec<Group>, hdf5_metno::Error>;
    fn get_hard_datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error>;
    fn get_external_datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error>;
    fn get_soft_datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error>;
    fn get_external_groups(&self) -> Result<Vec<Group>, hdf5_metno::Error>;
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

    fn get_external_datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error> {
        let external_datasets = self.iter_visit_default(vec![], |group, name, link, objects| {
            if LinkType::External == link.link_type {
                match group.dataset(name) {
                    Ok(ds) => objects.push(ds),
                    Err(_) => {
                        // Ignore it and move on
                        // TODO: Push a link broken thingie
                        return true;
                    }
                }
            }
            true
        })?;
        Ok(external_datasets)
    }

    fn get_external_groups(&self) -> Result<Vec<Group>, hdf5_metno::Error> {
        let external_groups = self.iter_visit_default(vec![], |group, name, link, objects| {
            if LinkType::External == link.link_type {
                match group.group(name) {
                    Ok(g) => objects.push(g),
                    Err(_) => {
                        return true; // we simply ignore it, and move on.
                    }
                }
            }
            true
        })?;
        Ok(external_groups)
    }
}

pub trait HasName {
    fn name(&self) -> String;
}

impl HasName for Node {
    fn name(&self) -> String {
        match self {
            Node::File(file) => file.name().split('/').next_back().unwrap_or("").to_string(),
            Node::Group(_, meta) => meta.display_name.clone(),
            Node::Dataset(_, meta) => meta.display_name.clone(),
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
        }
    }

    fn attribute(&self, name: &str) -> Result<Attribute, hdf5_metno::Error> {
        match self {
            Node::File(file) => file.attr(name),
            Node::Group(group, _) => group.attr(name),
            Node::Dataset(dataset, _) => dataset.attr(name),
        }
    }
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
pub enum Node {
    File(File),
    Group(Group, GroupMeta),
    Dataset(Dataset, DatasetMeta),
}

impl Node {
    pub fn render(&self, longest_name: u16) -> (Line<'static>, Line<'static>) {
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
        (name_line, path_line)
    }
}

#[derive(Debug)]
pub struct ComputedAttributes {
    pub longest_name_length: u16,
    #[allow(dead_code)]
    pub attributes: Vec<(String, Attribute)>,
    pub rendered_attributes: Vec<(Line<'static>, Line<'static>)>,
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
            .collect::<Vec<(Line<'static>, Line<'static>)>>();

        Ok(Self {
            longest_name_length,
            attributes,
            rendered_attributes,
        })
    }

    fn render_attributes(
        attributes: &Vec<(String, Attribute)>,
        name_area_width: usize,
    ) -> Vec<(Line<'static>, Line<'static>)> {
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
            rendered_attributes.push((name_line, value_line));
        }
        rendered_attributes
    }
}

#[derive(Debug)]
pub struct H5FNode {
    pub expanded: bool,
    pub node: Node,
    pub computed_attributes: Option<ComputedAttributes>,
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
}

pub enum GrpType {
    Soft(Group),
    Hard(Group),
    External(Group),
}

impl H5FNode {
    pub fn new(node_type: Node) -> Self {
        Self {
            expanded: false,
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

    pub fn icon(&self) -> String {
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

    pub fn expand(&mut self) -> Result<(), hdf5_metno::Error> {
        self.read_children()?;
        if !self.expanded {
            self.expand_toggle()?;
        }
        Ok(())
    }

    pub fn collapse(&mut self) {
        self.expanded = false;
    }

    pub fn expand_toggle(&mut self) -> Result<(), hdf5_metno::Error> {
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

    pub fn full_path(&self) -> String {
        match &self.node {
            Node::File(f) => f.filename().split("/").last().unwrap_or("").to_string(),
            Node::Group(g, _) => g.filename().split("/").last().unwrap_or("").to_string(),
            Node::Dataset(ds, _) => ds.filename().split("/").last().unwrap_or("").to_string(),
        }
    }

    pub fn name(&self) -> String {
        self.node.name()
    }

    pub fn expand_path(&mut self, relative_path: &str) -> Result<(), hdf5_metno::Error> {
        self.expand()?;
        let child_mame = relative_path.split('/').next();

        match child_mame {
            Some(n) => {
                for child in &self.children {
                    let child_name = match child.try_borrow() {
                        Ok(c) => c.name(),
                        Err(_) => return Ok(()),
                    };
                    if child_name == n {
                        let mut child_node = child.borrow_mut();
                        if relative_path.len() > n.len() + 1 {
                            child_node.expand_path(&relative_path[n.len() + 1..])?;
                        }
                        return Ok(());
                    }
                }
                panic!(
                    "Child not found {} {}",
                    child_mame.unwrap_or("N/A"),
                    relative_path
                );
            }
            None => Ok(()),
        }
    }

    fn read_children(&mut self) -> Result<(), hdf5_metno::Error> {
        if self.read {
            return Ok(());
        }
        if matches!(self.node, Node::Dataset(_, _)) {
            return Ok(());
        }

        let has_children = match &self.node {
            Node::File(file) => file,
            Node::Group(group, _) => group,
            Node::Dataset(_, _) => unreachable!("It should be guarded by the previous if"),
        };

        let mut groups = vec![];
        for g in has_children.get_hard_groups()? {
            groups.push(GrpType::Hard(g));
        }
        for g in has_children.get_external_groups()? {
            groups.push(GrpType::External(g));
        }
        for g in has_children.get_soft_groups()? {
            groups.push(GrpType::Soft(g));
        }
        let mut datasets = vec![];
        for d in has_children.get_hard_datasets()? {
            datasets.push(DSType::Hard(d));
        }
        for d in has_children.get_external_datasets()? {
            datasets.push(DSType::External(d));
        }
        for d in has_children.get_soft_datasets()? {
            datasets.push(DSType::Soft(d));
        }

        let mut children = Vec::new();
        for wrapped_g in groups {
            let (g, is_link) = match wrapped_g {
                GrpType::Hard(g) => (g, false),
                GrpType::External(g) => (g, true),
                GrpType::Soft(g) => (g, true),
            };
            let display_name = g.name().split('/').next_back().unwrap_or("").to_string();

            let meta = GroupMeta {
                is_link,
                display_name,
                filename: g.filename().to_string(),
            };
            let node = Rc::new(RefCell::new(H5FNode::new(Node::Group(g, meta))));

            children.push(node);
        }
        for wrapped_ds in datasets {
            let (d, is_link) = match wrapped_ds {
                DSType::Hard(ds) => (ds, false),
                DSType::External(ds) => (ds, true),
                DSType::Soft(ds) => (ds, true),
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
}

impl H5F {
    pub fn open(file_path: String) -> Result<Self, hdf5_metno::Error> {
        let file = hdf5_metno::file::File::open(&file_path)?;

        let root = Rc::new(RefCell::new(H5FNode::new(Node::File(file))));

        root.borrow_mut().read_children()?;
        root.borrow_mut().expand_toggle()?;

        let s = Self { root };
        Ok(s)
    }
}
