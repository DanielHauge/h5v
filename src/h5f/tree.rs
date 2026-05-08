use std::{cell::RefCell, rc::Rc};

use hdf5_metno::{
    plist::file_access::FileCloseDegree,
    types::{TypeDescriptor, VarLenUnicode},
    Dataset, File, Group, LinkType,
};
use ratatui::style::Color;

use crate::{
    error::AppError,
    h5f::read_string_attr_values,
    sprint_typedesc::{encoding_from_dtype, is_image, is_type_matrixable, sprint_typedescriptor},
};

use super::{
    attrs::HasName,
    compound::root_compound_projection,
    meta::{CompoundFieldProjection, DatasetMeta, EnumRenderOverrides, GroupMeta},
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

const ENUM_SYMBOLS_ATTR: &str = "SYMBOLS";
const ENUM_COLORS_ATTR: &str = "COLORS";

fn sanitize_attr_name_segment(segment: &str) -> Option<String> {
    let mut out = String::new();
    let mut last_was_separator = false;
    for ch in segment.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
            last_was_separator = false;
        } else if !last_was_separator {
            out.push('_');
            last_was_separator = true;
        }
    }

    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn enum_render_attr_names(
    base_name: &str,
    compound_projection: Option<&CompoundFieldProjection>,
) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(prefix) = compound_projection.and_then(|projection| {
        projection
            .field_path
            .iter()
            .filter_map(|segment| sanitize_attr_name_segment(&segment.name))
            .reduce(|acc, item| format!("{acc}_{item}"))
    }) {
        names.push(format!("{prefix}_{base_name}"));
    }
    names.push(base_name.to_string());
    names
}

fn read_named_string_attr_values(dataset: &Dataset, attr_names: &[String]) -> Option<Vec<String>> {
    attr_names.iter().find_map(|attr_name| {
        dataset
            .attr(attr_name)
            .ok()
            .and_then(|attr| read_string_attr_values(&attr).ok())
    })
}

fn parse_enum_color(value: &str) -> Option<Color> {
    crate::color_consts::parse_color(value)
}

fn resolve_enum_render_overrides(
    dataset: &Dataset,
    compound_projection: Option<&CompoundFieldProjection>,
    type_descriptor: &TypeDescriptor,
) -> Option<EnumRenderOverrides> {
    if !matches!(type_descriptor, TypeDescriptor::Enum(_)) {
        return None;
    }

    let symbols = read_named_string_attr_values(
        dataset,
        &enum_render_attr_names(ENUM_SYMBOLS_ATTR, compound_projection),
    )
    .map(|values| {
        values
            .into_iter()
            .map(|value| {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            })
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();

    let colors = read_named_string_attr_values(
        dataset,
        &enum_render_attr_names(ENUM_COLORS_ATTR, compound_projection),
    )
    .map(|values| {
        values
            .into_iter()
            .map(|value| parse_enum_color(&value))
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();

    let overrides = EnumRenderOverrides { colors, symbols };
    (!overrides.is_empty()).then_some(overrides)
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
                preview_expr: g
                    .attr("H5V_PREVIEW_EXPR")
                    .ok()
                    .and_then(|a| a.read_scalar::<VarLenUnicode>().ok().map(|v| v.to_string())),
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
            let (dtype_desc, unsupported_reason) = match dtype.to_descriptor() {
                Ok(dtype_desc) => (dtype_desc, None),
                Err(err) => (TypeDescriptor::VarLenAscii, Some(err.to_string())),
            };

            let mut shape = d.shape();
            let total_elems = d.size();
            if shape.is_empty() {
                shape.push(total_elems);
                shape.push(1);
            }
            let compound_projection = match (&dtype_desc, unsupported_reason.as_ref()) {
                (_, Some(_)) => None,
                (hdf5_metno::types::TypeDescriptor::Compound(compound), None) => {
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
                unsupported_reason,
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
            None,
        )?;
        out.push(Rc::new(RefCell::new(H5FNode::new(Node::Dataset(
            dataset.clone(),
            child_meta,
        )))));
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
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
    unsupported_reason: Option<String>,
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
    let is_unsupported = unsupported_reason.is_some();
    let total_bytes = data_bytesize * total_elems;
    let storage_required = dataset.storage_size();
    let chunk_shape = dataset.chunk();
    let image = if is_unsupported || compound_projection.is_some() {
        None
    } else {
        is_image(dataset)
    };
    let filename = dataset.filename().to_string();
    let hl = if is_unsupported || compound_projection.is_some() {
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
    let enum_render_overrides = if is_unsupported {
        None
    } else {
        resolve_enum_render_overrides(dataset, compound_projection.as_ref(), &type_descriptor)
    };

    Ok(DatasetMeta {
        hl,
        shape,
        data_type: unsupported_reason
            .as_ref()
            .map(|_| format!("opaque[{data_bytesize} bytes]"))
            .unwrap_or_else(|| sprint_typedescriptor(&type_descriptor)),
        unsupported_reason,
        type_descriptor: type_descriptor.clone(),
        display_name,
        data_bytesize,
        total_bytes,
        storage_required,
        total_elems,
        link_name,
        chunk_shape,
        matrixable: if is_unsupported {
            Some(crate::sprint_typedesc::MatrixRenderType::Opaque)
        } else if is_compound_container {
            None
        } else if compound_projection.is_some() {
            projected_matrixable(&type_descriptor)
        } else {
            is_type_matrixable(&type_descriptor)
        },
        encoding: if is_unsupported {
            super::meta::Encoding::Unknown
        } else {
            encoding_from_dtype(&type_descriptor)
        },
        image,
        enum_render_overrides,
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
            h5node.display_name = Some(crate::compat::linked_root_suffix(member_count));
        }

        let root = Rc::new(RefCell::new(h5node));

        root.borrow_mut().read_children()?;
        root.borrow_mut().expand_toggle()?;

        Ok(Self { root, file })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use std::str::FromStr;

    use hdf5_metno::types::{EnumMember, EnumType, IntSize, TypeDescriptor, VarLenUnicode};
    use ratatui::style::Color;

    use super::{
        build_dataset_meta, enum_render_attr_names, highlight_hint_from_name, parse_enum_color,
        resolve_enum_render_overrides, resolve_highlight_hint,
    };

    fn sample_enum() -> EnumType {
        EnumType {
            size: IntSize::U1,
            signed: false,
            members: vec![
                EnumMember {
                    name: "Green".to_string(),
                    value: 1,
                },
                EnumMember {
                    name: "Amber".to_string(),
                    value: 2,
                },
                EnumMember {
                    name: "Red".to_string(),
                    value: 3,
                },
            ],
        }
    }

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

    #[test]
    fn field_scoped_enum_attr_names_fall_back_to_global_names() {
        let names = enum_render_attr_names(
            "SYMBOLS",
            Some(&crate::h5f::CompoundFieldProjection {
                field_path: vec![
                    crate::h5f::CompoundFieldPathSegment {
                        name: "status".to_string(),
                        offset: 0,
                    },
                    crate::h5f::CompoundFieldPathSegment {
                        name: "evaluation-level".to_string(),
                        offset: 1,
                    },
                ],
                field_type: TypeDescriptor::Enum(sample_enum()),
                virtual_path: "/scan/status/evaluation-level".to_string(),
            }),
        );
        assert_eq!(
            names,
            vec![
                "STATUS_EVALUATION_LEVEL_SYMBOLS".to_string(),
                "SYMBOLS".to_string()
            ]
        );
    }

    #[test]
    fn parses_named_and_hex_enum_colors() {
        assert_eq!(parse_enum_color("amber"), Some(Color::Rgb(255, 191, 0)));
        assert_eq!(parse_enum_color("#00ff7f"), Some(Color::Rgb(0, 255, 127)));
        assert_eq!(parse_enum_color(""), None);
    }

    #[test]
    fn resolve_enum_overrides_reads_dataset_string_attrs() {
        let temp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let file = hdf5_metno::File::create(temp.path()).expect("failed to create hdf5 file");
        let dataset = file
            .new_dataset_builder()
            .with_data(&[0_u8, 1_u8, 2_u8])
            .create("values")
            .expect("failed to create dataset");

        let symbols = vec![
            VarLenUnicode::from_str("✓").expect("failed to create unicode symbol"),
            VarLenUnicode::from_str("⚠").expect("failed to create unicode symbol"),
            VarLenUnicode::from_str("✗").expect("failed to create unicode symbol"),
        ];
        dataset
            .new_attr_builder()
            .with_data(&symbols)
            .create("SYMBOLS")
            .expect("failed to create symbols attr");

        let colors = vec![
            VarLenUnicode::from_str("green").expect("failed to create color"),
            VarLenUnicode::from_str("amber").expect("failed to create color"),
            VarLenUnicode::from_str("#ff0000").expect("failed to create color"),
        ];
        dataset
            .new_attr_builder()
            .with_data(&colors)
            .create("COLORS")
            .expect("failed to create colors attr");

        let overrides =
            resolve_enum_render_overrides(&dataset, None, &TypeDescriptor::Enum(sample_enum()))
                .expect("expected enum render overrides");

        assert_eq!(
            overrides.symbols,
            vec![
                Some("✓".to_string()),
                Some("⚠".to_string()),
                Some("✗".to_string())
            ]
        );
        assert_eq!(
            overrides.colors,
            vec![
                Some(Color::Green),
                Some(Color::Rgb(255, 191, 0)),
                Some(Color::Rgb(255, 0, 0))
            ]
        );
    }

    #[test]
    fn unsupported_dataset_meta_disables_preview_features() {
        let temp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let file = hdf5_metno::File::create(temp.path()).expect("failed to create hdf5 file");
        let dataset = file
            .new_dataset_builder()
            .with_data(&[1_u8, 2_u8, 3_u8])
            .create("values")
            .expect("failed to create dataset");

        let meta = build_dataset_meta(
            &dataset,
            "values".to_string(),
            false,
            None,
            TypeDescriptor::VarLenAscii,
            None,
            dataset.shape(),
            1,
            dataset.size(),
            Some("Unsupported datatype class".to_string()),
        )
        .expect("failed to build dataset meta");

        assert_eq!(meta.data_type, "opaque[1 bytes]".to_string());
        assert_eq!(
            meta.unsupported_reason.as_deref(),
            Some("Unsupported datatype class")
        );
        assert_eq!(
            meta.matrixable,
            Some(crate::sprint_typedesc::MatrixRenderType::Opaque)
        );
        assert!(meta.image.is_none());
        assert!(meta.enum_render_overrides.is_none());
    }
}
