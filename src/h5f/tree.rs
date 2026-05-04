use std::{cell::RefCell, rc::Rc};

use hdf5_metno::{
    plist::file_access::FileCloseDegree, types::VarLenUnicode, Dataset, File, Group, LinkType,
};

use crate::{
    error::AppError,
    sprint_typedesc::{encoding_from_dtype, is_image, is_type_matrixable, sprint_typedescriptor},
};

use super::{
    attrs::HasName,
    compound::root_compound_projection,
    meta::{CompoundFieldProjection, DatasetMeta, GroupMeta},
    model::{H5FNode, Node, NodeType, H5F},
};

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

fn highlight_hint_from_name(name: &str) -> Option<String> {
    name.rsplit_once('.')
        .map(|(_, ext)| ext.trim())
        .filter(|ext| !ext.is_empty())
        .map(|ext| ext.to_ascii_lowercase())
}

fn resolve_highlight_hint(attr_hint: Option<String>, dataset_name: &str) -> Option<String> {
    attr_hint
        .map(|hint| hint.trim().to_ascii_lowercase())
        .filter(|hint| !hint.is_empty())
        .or_else(|| highlight_hint_from_name(dataset_name))
}

impl HasChildren for Group {
    fn get_soft_groups(&self) -> Result<Vec<Group>, hdf5_metno::Error> {
        let soft_groups = self.iter_visit_default(vec![], |group, name, link, objects| {
            if LinkType::Soft == link.link_type {
                match group.group(name) {
                    Ok(g) => objects.push(g),
                    Err(_) => return true,
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
                    Err(_) => return true,
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
                    Err(_) => return true,
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
                    Err(_) => return true,
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
                    objects.push(ExternalObject::Dataset(ds));
                } else if let Ok(grp) = group.group(name) {
                    objects.push(ExternalObject::Group(grp));
                } else {
                    objects.push(ExternalObject::LinkBroken(
                        name.to_string(),
                        group.filename().to_string(),
                    ));
                }
            }
            true
        })?;
        Ok(external_datasets)
    }
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
    pub fn full_path(&self) -> String {
        if let Some(ref name) = self.display_name {
            return name.clone();
        }
        match &self.node {
            Node::File(f) => f
                .filename()
                .split('/')
                .next_back()
                .unwrap_or("")
                .to_string(),
            Node::Group(g, _) => g
                .filename()
                .split('/')
                .next_back()
                .unwrap_or("")
                .to_string(),
            Node::Dataset(ds, _) => ds
                .filename()
                .split('/')
                .next_back()
                .unwrap_or("")
                .to_string(),
            Node::Broken(_, path, _) => path.clone(),
        }
    }

    pub fn name(&self) -> String {
        self.node.name()
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
            if child_node.is_expandable() {
                child_node.read_children()?;
            }
        }
        Ok(())
    }

    pub fn ensure_expanded(&mut self) -> Result<(), hdf5_metno::Error> {
        self.read_children()?;
        if !self.expanded {
            self.expanded = true;
        }

        for child in &self.children {
            let mut child_node = child.borrow_mut();
            if child_node.is_expandable() {
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

    pub fn expand_path(&mut self, relative_path: &str) -> Result<Option<usize>, AppError> {
        self.ensure_expanded()?;
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
        if matches!(self.node, Node::Broken(_, _, _)) {
            return Ok(());
        }

        if let Node::Dataset(dataset, meta) = &self.node {
            if !meta.is_compound_container() {
                return Ok(());
            }
            let children = synthetic_compound_children(dataset, meta)?;
            self.children = children;
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
            children.push(Rc::new(RefCell::new(H5FNode::new(Node::Group(g, meta)))));
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
                children.push(Rc::new(RefCell::new(H5FNode::new(broken_node))));
                continue;
            }
            let d = match d {
                Some(ds) => ds,
                None => continue,
            };
            let display_name = d.name().split('/').next_back().unwrap_or("").to_string();

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
            let compound_projection = match &dtype_desc {
                hdf5_metno::types::TypeDescriptor::Compound(compound) => {
                    Some(root_compound_projection(&d.name(), compound.clone()))
                }
                _ => None,
            };
            let meta = build_dataset_meta(
                &d,
                display_name,
                is_link,
                link_name,
                dtype_desc,
                compound_projection,
                shape,
                data_bytesize,
                total_elems,
            )?;
            children.push(Rc::new(RefCell::new(H5FNode::new(Node::Dataset(d, meta)))));
        }
        self.children = children;
        Ok(())
    }
}

fn synthetic_compound_children(
    dataset: &Dataset,
    meta: &DatasetMeta,
) -> Result<Vec<Rc<RefCell<H5FNode>>>, hdf5_metno::Error> {
    let Some(children) = super::compound_children(meta) else {
        return Ok(vec![]);
    };
    let mut out = Vec::with_capacity(children.len());
    for projection in children {
        let child_display_name = projection
            .field_path
            .last()
            .map(|segment| segment.name.clone())
            .unwrap_or_else(|| meta.display_name.clone());
        let child_meta = build_dataset_meta(
            dataset,
            child_display_name,
            meta.is_link,
            meta.link_name.clone(),
            projection.field_type.clone(),
            Some(projection),
            meta.shape.clone(),
            meta.data_bytesize,
            meta.total_elems,
        )?;
        out.push(Rc::new(RefCell::new(H5FNode::new(Node::Dataset(
            dataset.clone(),
            child_meta,
        )))));
    }
    Ok(out)
}

fn build_dataset_meta(
    dataset: &Dataset,
    display_name: String,
    is_link: bool,
    link_name: Option<String>,
    type_descriptor: hdf5_metno::types::TypeDescriptor,
    compound_projection: Option<CompoundFieldProjection>,
    shape: Vec<usize>,
    data_bytesize: usize,
    total_elems: usize,
) -> Result<DatasetMeta, hdf5_metno::Error> {
    fn projected_matrixable(
        type_descriptor: &hdf5_metno::types::TypeDescriptor,
    ) -> Option<crate::sprint_typedesc::MatrixRenderType> {
        match type_descriptor {
            hdf5_metno::types::TypeDescriptor::FixedArray(_, _) => {
                Some(crate::sprint_typedesc::MatrixRenderType::Strings)
            }
            _ => is_type_matrixable(type_descriptor),
        }
    }

    let is_compound_container = compound_projection
        .as_ref()
        .and_then(|projection| projection.current_compound_type())
        .is_some();
    let total_bytes = data_bytesize * total_elems;
    let storage_required = dataset.storage_size();
    let chunk_shape = dataset.chunk();
    let image = if compound_projection.is_some() {
        None
    } else {
        is_image(dataset)
    };
    let filename = dataset.filename().to_string();
    let hl = if compound_projection.is_some() {
        None
    } else {
        resolve_highlight_hint(
            dataset
                .attr("HIGHLIGHT")
                .ok()
                .and_then(|a| a.read_scalar::<VarLenUnicode>().ok().map(|v| v.to_string())),
            &display_name,
        )
    };

    Ok(DatasetMeta {
        hl,
        shape,
        data_type: sprint_typedescriptor(&type_descriptor),
        type_descriptor: type_descriptor.clone(),
        display_name,
        data_bytesize,
        total_bytes,
        storage_required,
        total_elems,
        link_name,
        chunk_shape,
        matrixable: if is_compound_container {
            None
        } else if compound_projection.is_some() {
            projected_matrixable(&type_descriptor)
        } else {
            is_type_matrixable(&type_descriptor)
        },
        encoding: encoding_from_dtype(&type_descriptor),
        image,
        is_link,
        filename,
        compound_projection,
    })
}

impl H5F {
    pub fn open(file_path: String, linked: bool, write: bool) -> Result<Self, hdf5_metno::Error> {
        let builder = File::with_options()
            .with_fapl(|fapl| fapl.fclose_degree(FileCloseDegree::Strong))
            .clone();
        let file = if write {
            builder.open_rw(&file_path)?
        } else {
            builder.open(&file_path)?
        };

        let member_count = file.member_names()?.len();
        let mut h5node = H5FNode::new(Node::File(file.clone()));
        if linked {
            h5node.display_name = Some(format!(" ({member_count}) linked ").to_string());
        }

        let root = Rc::new(RefCell::new(h5node));

        root.borrow_mut().read_children()?;
        root.borrow_mut().expand_toggle()?;

        Ok(Self { root, file })
    }
}

#[cfg(test)]
mod tests {
    use super::{highlight_hint_from_name, resolve_highlight_hint};

    #[test]
    fn highlight_attribute_takes_precedence_over_extension() {
        assert_eq!(
            resolve_highlight_hint(Some("json".to_string()), "demo.py"),
            Some("json".to_string())
        );
    }

    #[test]
    fn dataset_extension_is_used_when_attribute_is_missing() {
        assert_eq!(
            resolve_highlight_hint(None, "pipeline.yml"),
            Some("yml".to_string())
        );
    }

    #[test]
    fn blank_attribute_still_falls_back_to_extension() {
        assert_eq!(
            resolve_highlight_hint(Some("   ".to_string()), "demo.py"),
            Some("py".to_string())
        );
    }

    #[test]
    fn names_without_extensions_do_not_get_highlighting() {
        assert_eq!(highlight_hint_from_name("messages"), None);
    }
}
