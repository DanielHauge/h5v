use std::{cell::RefCell, rc::Rc};

use hdf5_metno::{Attribute, Dataset, File, Group};

use crate::sprint_typedesc::sprint_typedescriptor;

#[derive(Debug)]
pub struct DatasetMeta {
    shape: Vec<usize>,
    data_type: String,
    data_bytesize: usize,
    total_bytes: usize,
    total_elems: usize,
    chunk_shape: Option<Vec<usize>>,
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
        Ok(self.groups()?)
    }

    fn get_datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error> {
        Ok(self.datasets()?)
    }
}

impl HasChildren for File {
    fn get_groups(&self) -> Result<Vec<Group>, hdf5_metno::Error> {
        Ok(self.groups()?)
    }

    fn get_datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error> {
        Ok(self.datasets()?)
    }
}

pub trait HasName {
    fn name(&self) -> String;
}

impl HasName for Node {
    fn name(&self) -> String {
        match self {
            Node::File(file) => file.name().split("/").last().unwrap().to_string(),
            Node::Group(group) => group.name().split("/").last().unwrap().to_string(),
            Node::Dataset(dataset, _) => dataset.name().split("/").last().unwrap().to_string(),
        }
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
        let size_str = match size {
            0..1024 => format!("{} B", size),
            1024..1048576 => format!("{:.2} KB", size as f64 / 1024.0),
            1048576..1073741824 => format!("{:.2} MB", size as f64 / 1024.0 / 1024.0),
            _ => format!("{:.2} GB", size as f64 / 1024.0 / 1024.0 / 1024.0),
        };
        size_str
    }
}

#[derive(Debug)]
pub enum Node {
    File(File),
    Group(Group),
    Dataset(Dataset, DatasetMeta),
}

#[derive(Debug)]
pub struct ComputedAttributes {
    pub longest_name_length: usize,
    pub attributes: Vec<(String, Attribute)>,
}
impl ComputedAttributes {
    pub fn new(node: &Node) -> Result<Self, hdf5_metno::Error> {
        let attributes = node.attributes()?;

        Ok(Self {
            longest_name_length: 0,
            attributes,
        })
    }

    pub fn add(&mut self, name: String, attr: Attribute) {
        self.longest_name_length = self.longest_name_length.max(name.len());
        self.attributes.push((name, attr));
    }
}

#[derive(Debug)]
pub struct H5FNode {
    pub expanded: bool,
    pub node: Node,
    pub computed_attributes: Option<ComputedAttributes>,
    pub read: bool,
    pub children: Vec<Rc<RefCell<H5FNode>>>,
}

impl H5FNode {
    pub fn new(node_type: Node) -> Self {
        Self {
            expanded: false,
            node: node_type,
            read: false,
            children: vec![],
            computed_attributes: None,
        }
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

    pub fn expand_toggle(&mut self) -> Result<(), hdf5_metno::Error> {
        if self.expanded {
            self.expanded = false;
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
            Node::File(f) => f.filename().split("/").last().unwrap().to_string(),
            Node::Group(g) => g.filename().split("/").last().unwrap().to_string(),
            Node::Dataset(ds, _) => ds.filename().split("/").last().unwrap().to_string(),
        }
    }

    pub fn name(&self) -> String {
        self.node.name()
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
            let node = H5FNode::new(Node::Group(g));
            let node = Rc::new(RefCell::new(node));
            children.push(node);
        }
        for d in datasets {
            let dtype = d.dtype()?;
            let data_bytesize = dtype.size();
            let dtype_desc = dtype.to_descriptor()?;

            let shape = d.shape();
            let total_elems = d.size();
            let total_bytes = total_elems * data_bytesize;
            let data_type = sprint_typedescriptor(dtype_desc);

            let chunk_shape = d.chunk();

            let meta = DatasetMeta {
                shape,
                data_type,
                data_bytesize,
                total_bytes,
                total_elems,
                chunk_shape,
            };
            let node_ds = Node::Dataset(d, meta);
            let node = H5FNode::new(node_ds);
            let node = Rc::new(RefCell::new(node));
            children.push(node);
        }
        self.children = children;
        Ok(())
    }
}

pub struct H5F {
    pub root: H5FNode,
}

impl H5F {
    pub fn open(file_path: String) -> Result<Self, hdf5_metno::Error> {
        let file = hdf5_metno::file::File::open(&file_path)?;
        let mut h5fnode = H5FNode::new(Node::File(file));
        h5fnode.read_children()?;
        h5fnode.expand_toggle()?;

        let s = Self { root: h5fnode };
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use crate::h5f::HasName;

    use super::H5F;

    #[test]
    fn test_h5f_open() {
        let h5f = H5F::open("example-femm-3d.h5".to_string());
        assert!(h5f.is_ok());
    }

    #[test]
    fn test_h5f_open_fail() {
        let h5f = H5F::open("none.h5".to_string());
        assert!(h5f.is_err());
    }

    #[test]
    fn test_h5f_read_root_path() {
        let h5f = H5F::open("example-femm-3d.h5".to_string()).unwrap();
        let root_node = h5f.root.node;
        let root_group = root_node.name();
        assert_eq!(root_group, "");
    }

    #[test]
    fn test_h5f_expand() {
        let mut h5f = H5F::open("example-femm-3d.h5".to_string()).unwrap();
        assert_eq!(h5f.root.children.len(), 1);
        h5f.root.expand_toggle().unwrap();
        assert_eq!(h5f.root.children.len(), 1);
    }

    #[test]
    fn test_h5f_read_children() {
        let mut h5f = H5F::open("example-femm-3d.h5".to_string()).unwrap();
        h5f.root.expand_toggle().unwrap();
        let root_children = &h5f.root.children;
        let root_child = &root_children[0];
        assert_eq!(root_child.borrow().name(), "data");
    }

    #[test]
    fn test_h5f_read_ds() {
        let mut h5f = H5F::open("example-femm-3d.h5".to_string()).unwrap();
        h5f.root.expand_toggle().unwrap();
        let grp_data = &mut h5f.root.children[0];
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
