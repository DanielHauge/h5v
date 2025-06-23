use std::{cell::RefCell, rc::Rc};

use hdf5_metno::{Attribute, Dataset, File, Group};
use ratatui::{
    style::{Style, Stylize},
    text::{Line, Span},
};

use crate::{
    color_consts,
    search::Searcher,
    sprint_attributes::sprint_attribute,
    sprint_typedesc::{encoding_from_dtype, is_image, is_type_numerical, sprint_typedescriptor},
    ui::state::ContentShowMode,
};

#[derive(Debug)]
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

#[derive(Debug)]
pub enum ImageType {
    Jpeg,
    Png,
    Grayscale,
    Bitmap,
    Truecolor(InterlaceMode),
    Indexed(InterlaceMode),
}

#[derive(Debug)]
pub struct DatasetMeta {
    pub shape: Vec<usize>,
    pub data_type: String,
    #[allow(dead_code)]
    data_bytesize: usize,
    storage_required: u64,
    total_bytes: usize,
    total_elems: usize,
    chunk_shape: Option<Vec<usize>>,
    pub numerical: bool,
    pub encoding: Encoding,
    pub image: Option<ImageType>,
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
    fn get_groups(&self) -> Result<Vec<Group>, hdf5_metno::Error>;
    fn get_datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error>;
}

impl HasChildren for Group {
    fn get_groups(&self) -> Result<Vec<Group>, hdf5_metno::Error> {
        self.groups()
    }

    fn get_datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error> {
        self.datasets()
    }
}

impl HasChildren for File {
    fn get_groups(&self) -> Result<Vec<Group>, hdf5_metno::Error> {
        self.groups()
    }

    fn get_datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error> {
        self.datasets()
    }
}

pub trait HasName {
    fn name(&self) -> String;
}

impl HasName for Node {
    fn name(&self) -> String {
        match self {
            Node::File(file) => file.name().split("/").last().unwrap_or("").to_string(),
            Node::Group(group) => group.name().split("/").last().unwrap_or("").to_string(),
            Node::Dataset(dataset, _) => dataset.name().split("/").last().unwrap_or("").to_string(),
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
            Node::Group(group) => group.name(),
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
            Node::Group(group) => Ok(group.attr_names()?),
            Node::Dataset(dataset, _) => Ok(dataset.attr_names()?),
        }
    }

    fn attribute(&self, name: &str) -> Result<Attribute, hdf5_metno::Error> {
        match self {
            Node::File(file) => file.attr(name),
            Node::Group(group) => group.attr(name),
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

#[derive(Debug)]
pub enum Node {
    File(File),
    Group(Group),
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
    pub searcher: Rc<RefCell<Searcher>>,
}

impl H5FNode {
    pub fn new(node_type: Node, searcher: Rc<RefCell<Searcher>>) -> Self {
        Self {
            expanded: false,
            node: node_type,
            read: false,
            children: vec![],
            view_loaded: 50,
            computed_attributes: None,
            searcher,
        }
    }

    pub fn render(&self) -> Line<'static> {
        let icon = match self.is_group() {
            true => " ",
            false => "󰈚 ",
        };
        let icon_color = match self.is_group() {
            true => color_consts::GROUP_COLOR,
            false => color_consts::DATASET_FILE_COLOR,
        };

        let icon_span = Span::styled(icon, Style::default().fg(icon_color));
        let name_color = match self.is_group() {
            true => color_consts::VARIABLE_BLUE,
            false => color_consts::DATASET_COLOR,
        };
        Line::from(vec![
            icon_span,
            Span::styled(" ", Style::default().fg(color_consts::LINES_COLOR)),
            Span::styled(self.name(), Style::default().fg(name_color).bold()),
        ])
    }

    pub fn content_show_modes(&self) -> Vec<ContentShowMode> {
        let mut result = vec![];

        match &self.node {
            Node::File(_) => {}
            Node::Group(_) => {}
            Node::Dataset(_, dataset_meta) => {
                result.push(ContentShowMode::Preview);
                if dataset_meta.numerical {
                    result.push(ContentShowMode::Matrix);
                }
            }
        }
        result
    }

    pub fn is_group(&self) -> bool {
        matches!(self.node, Node::Group(_))
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
            Node::Group(g) => g.filename().split("/").last().unwrap_or("").to_string(),
            Node::Dataset(ds, _) => ds.filename().split("/").last().unwrap_or("").to_string(),
        }
    }

    pub fn name(&self) -> String {
        self.node.name()
    }

    pub fn index(&mut self, recursive: bool) -> Result<(), hdf5_metno::Error> {
        self.read_children()?;
        if recursive {
            for child in &self.children {
                self.searcher.borrow_mut().add(H5FNodeRef::from(child));
                let mut child_node = child.borrow_mut();
                if child_node.is_group() {
                    child_node.index(recursive)?;
                }
            }
        }

        Ok(())
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
            Node::Group(group) => group,
            Node::Dataset(_, _) => unreachable!("It should be guarded by the previous if"),
        };
        let groups = has_children.get_groups()?;
        let datasets = has_children.get_datasets()?;
        let mut children = Vec::new();
        for g in groups {
            let node = Rc::new(RefCell::new(H5FNode::new(
                Node::Group(g),
                self.searcher.clone(),
            )));

            children.push(node);
        }
        for d in datasets {
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
            let numerical = is_type_numerical(&dtype_desc);
            let encoding = encoding_from_dtype(&dtype_desc);
            let total_bytes = data_bytesize * total_elems;
            let storage_required = d.storage_size();
            let chunk_shape = d.chunk();
            let image = is_image(&d);

            let meta = DatasetMeta {
                shape,
                data_type,
                data_bytesize,
                total_bytes,
                storage_required,
                total_elems,
                chunk_shape,
                numerical,
                encoding,
                image,
            };
            let node_ds = Node::Dataset(d, meta);
            let node = Rc::new(RefCell::new(H5FNode::new(node_ds, self.searcher.clone())));

            children.push(node);
        }
        self.children = children;
        Ok(())
    }
}

#[derive(Debug)]
pub struct H5FNodeRef {
    pub name: String,
    pub node: Rc<RefCell<H5FNode>>,
    pub rendered: Line<'static>,
}

impl From<&Rc<RefCell<H5FNode>>> for H5FNodeRef {
    fn from(node: &Rc<RefCell<H5FNode>>) -> Self {
        let name = node.borrow().name();
        let node = Rc::clone(node);
        let rendered = node.borrow().render();
        Self {
            name,
            node,
            rendered,
        }
    }
}

impl AsRef<str> for H5FNodeRef {
    fn as_ref(&self) -> &str {
        self.name.as_str()
    }
}

pub struct H5F {
    pub root: Rc<RefCell<H5FNode>>,
}

impl H5F {
    pub fn open(
        file_path: String,
        searcher: Rc<RefCell<Searcher>>,
    ) -> Result<Self, hdf5_metno::Error> {
        let file = hdf5_metno::file::File::open(&file_path)?;

        let root = Rc::new(RefCell::new(H5FNode::new(Node::File(file), searcher)));

        root.borrow_mut().read_children()?;
        root.borrow_mut().expand_toggle()?;

        let s = Self { root };
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use crate::{h5f::HasName, search::Searcher};

    use super::H5F;

    fn new_searcher() -> Rc<RefCell<Searcher>> {
        Rc::new(RefCell::new(Searcher::new()))
    }

    #[test]
    fn test_h5f_open() {
        let h5f = H5F::open("example-femm-3d.h5".to_string(), new_searcher());
        assert!(h5f.is_ok());
    }

    #[test]
    fn test_h5f_open_fail() {
        let h5f = H5F::open("none.h5".to_string(), new_searcher());
        assert!(h5f.is_err());
    }

    #[test]
    fn test_h5f_read_root_path() {
        let h5f = H5F::open("example-femm-3d.h5".to_string(), new_searcher()).unwrap();
        let root_node = &h5f.root.borrow().node;
        let root_group = root_node.name();
        assert_eq!(root_group, "");
    }

    #[test]
    fn test_h5f_expand() {
        let h5f = H5F::open("example-femm-3d.h5".to_string(), new_searcher()).unwrap();
        assert_eq!(h5f.root.borrow().children.len(), 1);
        h5f.root.borrow_mut().expand_toggle().unwrap();
        assert_eq!(h5f.root.borrow().children.len(), 1);
    }

    #[test]
    fn test_h5f_read_children() {
        let h5f = H5F::open("example-femm-3d.h5".to_string(), new_searcher()).unwrap();
        h5f.root.borrow_mut().expand_toggle().unwrap();
        let root_children = &h5f.root.borrow().children;
        let root_child = &root_children[0];
        assert_eq!(root_child.borrow().name(), "data");
    }

    #[test]
    fn test_h5f_read_ds() {
        let h5f = H5F::open("example-femm-3d.h5".to_string(), new_searcher()).unwrap();
        h5f.root.borrow_mut().expand_toggle().unwrap();
        let grp_data = &mut h5f.root.borrow_mut().children[0];
        assert_eq!(grp_data.borrow().name(), "data");
        grp_data.borrow_mut().expand_toggle().unwrap();
        let grp_1 = &mut grp_data.borrow_mut().children[0];
        assert_eq!(grp_1.borrow().name(), "1");
        grp_1.borrow_mut().expand_toggle().unwrap();
        let grp_meshes = &mut grp_1.borrow_mut().children[0];
        assert_eq!(grp_meshes.borrow().name(), "meshes");
        grp_meshes.borrow_mut().expand_toggle().unwrap();
        let grp_b = &mut grp_meshes.borrow_mut().children[0];
        assert_eq!(grp_b.borrow().name(), "B");
        grp_b.borrow_mut().expand_toggle().unwrap();
        let ds_b = &mut grp_b.borrow_mut().children[0];
        assert_eq!(ds_b.borrow().name(), "x");
        let ds_node = &ds_b.borrow().node;
        let ds_b_meta = match ds_node {
            super::Node::Dataset(_, meta) => meta,
            _ => panic!("It should be a dataset"),
        };
        assert_eq!(ds_b_meta.shape_string(), "47 x 47 x 47 = 103823");
        assert_eq!(ds_b_meta.data_type_string(), "f64");
        assert_eq!(ds_b_meta.size_string(), "811.12 KB");
        assert_eq!(
            ds_b_meta.chunk_shape_string(),
            Some("32 x 16 x 16 = 8192".to_string())
        );
    }
}
