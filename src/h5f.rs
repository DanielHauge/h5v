use hdf5_metno::{Dataset, File, Group};
use ratatui::widgets::canvas::Shape;

use crate::sprint_typedesc::sprint_typedescriptor;

pub struct DatasetMeta {
    shape: Vec<usize>,
    data_type: String,
    data_bytesize: usize,
    total_bytes: usize,
    total_elems: usize,
    chunk_shape: Option<Vec<usize>>,
}

pub trait HasChildren {
    fn Groups(&self) -> Result<Vec<Group>, hdf5_metno::Error>;
    fn Datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error>;
}

impl HasChildren for Group {
    fn Groups(&self) -> Result<Vec<Group>, hdf5_metno::Error> {
        Ok(self.groups()?)
    }

    fn Datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error> {
        Ok(self.datasets()?)
    }
}

impl HasChildren for File {
    fn Groups(&self) -> Result<Vec<Group>, hdf5_metno::Error> {
        Ok(self.groups()?)
    }

    fn Datasets(&self) -> Result<Vec<Dataset>, hdf5_metno::Error> {
        Ok(self.datasets()?)
    }
}

pub trait HasName {
    fn name(&self) -> String;
}

impl HasName for Node {
    fn name(&self) -> String {
        match self {
            Node::File(file) => file.name(),
            Node::Group(group) => group.name(),
            Node::Dataset(dataset, _) => dataset.name(),
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

pub enum Node {
    File(File),
    Group(Group),
    Dataset(Dataset, DatasetMeta),
}

pub struct H5FNode {
    pub expanded: bool,
    pub node: Node,
    pub read: bool,
    pub children: Vec<H5FNode>,
}

impl H5FNode {
    pub fn new(node_type: Node) -> Self {
        Self {
            expanded: false,
            node: node_type,
            read: false,
            children: vec![],
        }
    }

    pub fn expand(&mut self) -> Result<(), hdf5_metno::Error> {
        self.expanded = true;
        self.read_children()?;
        Ok(())
    }

    pub fn full_path(&self) -> String {
        match &self.node {
            Node::File(f) => f.filename().split("/").last().unwrap().to_string(),
            Node::Group(g) => g.filename().split("/").last().unwrap().to_string(),
            Node::Dataset(ds, _) => ds.filename().split("/").last().unwrap().to_string(),
        }
    }

    fn name(&self) -> String {
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
        let groups = has_children.Groups()?;
        let datasets = has_children.Datasets()?;
        for g in groups {
            self.children.push(H5FNode::new(Node::Group(g)));
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
            self.children.push(node);
        }
        Ok(())
    }
}

pub struct H5F {
    pub root: H5FNode,
}

impl H5F {
    pub fn open(file_path: String) -> Result<Self, hdf5_metno::Error> {
        let file = hdf5_metno::file::File::open(&file_path)?;
        Ok(Self {
            root: H5FNode::new(Node::File(file)),
        })
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
        assert_eq!(root_group, "/");
    }
    #[test]
    fn test_h5f_expand() {
        let mut h5f = H5F::open("example-femm-3d.h5".to_string()).unwrap();
        assert_eq!(h5f.root.children.len(), 0);
        h5f.root.expand().unwrap();
        assert_eq!(h5f.root.children.len(), 1);
    }

    #[test]
    fn test_h5f_read_children() {
        let mut h5f = H5F::open("example-femm-3d.h5".to_string()).unwrap();
        h5f.root.expand().unwrap();
        let root_children = &h5f.root.children;
        let root_child = &root_children[0];
        assert_eq!(root_child.name(), "/data");
    }

    #[test]
    fn test_h5f_read_ds() {
        let mut h5f = H5F::open("example-femm-3d.h5".to_string()).unwrap();
        h5f.root.expand().unwrap();
        let grp_data = &mut h5f.root.children[0];
        assert_eq!(grp_data.name(), "/data");
        grp_data.expand().unwrap();
        let grp_1 = &mut grp_data.children[0];
        assert_eq!(grp_1.name(), "/data/1");
        grp_1.expand().unwrap();
        let grp_meshes = &mut grp_1.children[0];
        assert_eq!(grp_meshes.name(), "/data/1/meshes");
        grp_meshes.expand().unwrap();
        let grp_b = &mut grp_meshes.children[0];
        assert_eq!(grp_b.name(), "/data/1/meshes/B");
        grp_b.expand().unwrap();
        let ds_b = &mut grp_b.children[0];
        assert_eq!(ds_b.name(), "/data/1/meshes/B/x");
        let ds_b_meta = match &ds_b.node {
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
