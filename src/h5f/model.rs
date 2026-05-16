use std::{cell::RefCell, rc::Rc};

use hdf5_metno::{types::TypeDescriptor, Dataset, File, Group};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::{
    configure,
    ui::{
        render::MatrixRenderType,
        state::{AttributeCursor, ContentShowMode},
    },
};

use super::{
    attrs::{ComputedAttributes, HasPath, RenderedAttributeRow},
    meta::{DatasetMeta, GroupMeta},
};

#[derive(Debug, Clone)]
pub enum NodeType {
    Dataset,
    Group,
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Node {
    File(File),
    Group(Group, GroupMeta),
    Dataset(Dataset, DatasetMeta),
    Broken(NodeType, String, String),
}

impl Node {
    pub fn render(&self, longest_name: u16) -> RenderedAttributeRow {
        let min_first_panel = match longest_name {
            0..8 => 8,
            8..=u16::MAX => longest_name,
        };
        let path = self.path();
        let name_styled = Span::styled(
            "path",
            Style::default()
                .fg(configure::themed_color(|colors| {
                    colors.metadata.property_name
                }))
                .bold(),
        );
        let extra_name_space = min_first_panel as usize - "path".len();
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
        let path_styled = Span::styled(
            path,
            Style::default()
                .fg(configure::themed_color(|colors| {
                    colors.metadata.property_value
                }))
                .bold(),
        );
        let path_line = Line::from(vec![path_styled]);
        let empty_line = Line::from(vec![Span::raw("")]);
        RenderedAttributeRow::property("path", (name_line, path_line, empty_line))
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
    pub selected_indexes: Vec<usize>,
}

impl H5FNode {
    pub fn new(node_type: Node) -> Self {
        let selected_indexes = match &node_type {
            Node::Dataset(_, meta) => vec![0; meta.shape.len()],
            _ => vec![],
        };
        let selected_col = if selected_indexes.len() > 1 { 1 } else { 0 };
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
            selected_col,
            line_offset: 0,
            col_offset: 0,
            selected_indexes,
        }
    }

    pub fn sync_selection_rank(&mut self, rank: usize) {
        self.selected_indexes.resize(rank, 0);

        if rank == 0 {
            self.selected_dim = 0;
            self.selected_x = 0;
            self.selected_row = 0;
            self.selected_col = 0;
            return;
        }

        let last = rank - 1;
        self.selected_dim = self.selected_dim.min(last);
        self.selected_x = self.selected_x.min(last);
        self.selected_row = self.selected_row.min(last);

        if rank == 1 {
            self.selected_col = 0;
        } else {
            self.selected_col = self.selected_col.min(last);
            if self.selected_col == self.selected_row {
                self.selected_col = (self.selected_row + 1).min(last);
            }
        }
    }

    pub fn icon(&self) -> String {
        if let Node::Broken(_, _, _) = &self.node {
            return configure::configured_symbol(|symbols| symbols.tree.broken_node_icon)
                .to_string();
        }
        if self.is_compound_container() {
            return configure::configured_symbol(|symbols| symbols.tree.compound_container_icon)
                .to_string();
        }
        if self.is_compound_leaf() {
            return configure::configured_symbol(|symbols| symbols.tree.compound_leaf_icon)
                .to_string();
        }
        match self.is_group() {
            true => {
                let Node::Group(_, meta) = &self.node else {
                    return "?".to_string();
                };
                if meta.is_link {
                    configure::configured_symbol(|symbols| symbols.tree.link_marker).to_string()
                } else {
                    " ".to_string()
                }
            }
            false => {
                let Node::Dataset(_, meta) = &self.node else {
                    return "? ".to_string();
                };
                if meta.is_link {
                    configure::configured_symbol(|symbols| symbols.tree.dataset_link_icon)
                        .to_string()
                } else {
                    configure::configured_symbol(|symbols| symbols.tree.dataset_icon).to_string()
                }
            }
        }
    }

    pub fn content_show_modes(&self) -> Vec<ContentShowMode> {
        let mut result = vec![];

        match &self.node {
            Node::File(_) => {
                result.push(ContentShowMode::Preview);
            }
            Node::Broken(_, _, _) => {}
            Node::Group(_, _) => {
                result.push(ContentShowMode::Preview);
            }
            Node::Dataset(_, dataset_meta)
                if dataset_meta.is_compound_leaf()
                    && matches!(dataset_meta.matrixable, Some(MatrixRenderType::Strings)) =>
            {
                if dataset_meta.shape.iter().any(|x| *x > 1) {
                    result.push(ContentShowMode::Matrix);
                } else {
                    result.push(ContentShowMode::Preview);
                }
            }
            Node::Dataset(_, dataset_meta)
                if matches!(dataset_meta.matrixable, Some(MatrixRenderType::Opaque)) =>
            {
                if dataset_meta.shape.iter().any(|x| *x > 1) {
                    result.push(ContentShowMode::Matrix);
                } else {
                    result.push(ContentShowMode::Preview);
                }
            }
            Node::Dataset(_, dataset_meta) if dataset_meta.is_compound_container() => {
                result.push(ContentShowMode::Preview);
                if dataset_meta.supports_compound_root_matrix() {
                    result.push(ContentShowMode::Matrix);
                }
            }
            Node::Dataset(_, dataset_meta)
                if dataset_meta.is_compound_leaf()
                    && matches!(
                        dataset_meta.type_descriptor,
                        TypeDescriptor::FixedArray(_, _)
                    ) =>
            {
                if dataset_meta.shape.iter().any(|x| *x > 1) {
                    result.push(ContentShowMode::Matrix);
                }
            }
            Node::Dataset(_, dataset_meta) => match dataset_meta.matrixable {
                Some(matrix_renderable) => match matrix_renderable {
                    MatrixRenderType::Float64 => {
                        if dataset_meta.shape.iter().any(|x| *x > 1) {
                            result.push(ContentShowMode::Matrix);
                        }
                        if dataset_meta.shape.iter().filter(|x| **x > 1).count() >= 2 {
                            result.push(ContentShowMode::Heatmap);
                        }
                        result.push(ContentShowMode::Preview);
                    }
                    MatrixRenderType::Opaque => {
                        if dataset_meta.shape.iter().any(|x| *x > 1) {
                            result.push(ContentShowMode::Matrix);
                        } else {
                            result.push(ContentShowMode::Preview);
                        }
                    }
                    MatrixRenderType::Uint64 => {
                        if dataset_meta.shape.iter().any(|x| *x > 1) {
                            result.push(ContentShowMode::Matrix);
                        }
                        if dataset_meta.shape.iter().filter(|x| **x > 1).count() >= 2 {
                            result.push(ContentShowMode::Heatmap);
                        }
                        result.push(ContentShowMode::Preview);
                    }
                    MatrixRenderType::Int64 => {
                        if dataset_meta.shape.iter().any(|x| *x > 1) {
                            result.push(ContentShowMode::Matrix);
                        }
                        if dataset_meta.shape.iter().filter(|x| **x > 1).count() >= 2 {
                            result.push(ContentShowMode::Heatmap);
                        }
                        result.push(ContentShowMode::Preview);
                    }
                    MatrixRenderType::Strings => {
                        if dataset_meta.shape.iter().any(|x| *x > 1) {
                            result.push(ContentShowMode::Matrix);
                        } else {
                            result.push(ContentShowMode::Preview);
                        }
                    }
                    MatrixRenderType::Enum => {
                        if dataset_meta.shape.iter().any(|x| *x > 1) {
                            result.push(ContentShowMode::Matrix);
                        }
                        result.push(ContentShowMode::Preview);
                    }
                    MatrixRenderType::ByteArray => {
                        if dataset_meta.shape.iter().any(|x| *x > 1) {
                            result.push(ContentShowMode::Matrix);
                        }
                        if dataset_meta.image.is_some()
                            || !dataset_meta.shape.iter().any(|x| *x > 1)
                        {
                            result.push(ContentShowMode::Preview);
                        }
                    }
                    MatrixRenderType::Compound => {}
                },
                None => result.push(ContentShowMode::Preview),
            },
        }
        result
    }

    pub fn is_group(&self) -> bool {
        matches!(self.node, Node::Group(_, _))
    }

    pub fn is_compound_container(&self) -> bool {
        matches!(&self.node, Node::Dataset(_, meta) if meta.is_compound_container())
    }

    pub fn is_compound_leaf(&self) -> bool {
        matches!(&self.node, Node::Dataset(_, meta) if meta.is_compound_leaf())
    }

    pub fn is_expandable(&self) -> bool {
        self.is_group() || self.is_compound_container()
    }
}

pub struct H5F {
    pub root: Rc<RefCell<H5FNode>>,
    pub file: File,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{H5FNode, Node};
    use crate::{
        h5f::{CompoundFieldProjection, DatasetMeta, Encoding},
        ui::{render::MatrixRenderType, state::ContentShowMode},
    };
    use hdf5_metno::types::{CompoundType, TypeDescriptor};
    use ndarray::arr2;

    #[test]
    fn file_nodes_support_preview_mode() {
        let temp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let file = hdf5_metno::File::create(temp.path()).expect("failed to create hdf5 file");
        let node = H5FNode::new(Node::File(file));

        assert_eq!(node.content_show_modes(), vec![ContentShowMode::Preview]);
    }

    #[test]
    fn projected_multi_value_string_leaves_are_matrix_only() {
        let temp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let file = hdf5_metno::File::create(temp.path()).expect("failed to create hdf5 file");
        let dataset = file
            .new_dataset_builder()
            .with_data(&[1_i16, 2_i16])
            .create("values")
            .expect("failed to create dataset");
        let node = H5FNode::new(Node::Dataset(
            dataset,
            DatasetMeta {
                link_name: None,
                display_name: "labels".to_string(),
                shape: vec![2],
                data_type: "[2]string (len 8)".to_string(),
                unsupported_reason: None,
                type_descriptor: TypeDescriptor::FixedAscii(8),
                data_bytesize: 16,
                storage_required: 16,
                total_bytes: 16,
                total_elems: 2,
                chunk_shape: None,
                hl: None,
                matrixable: Some(MatrixRenderType::Strings),
                encoding: Encoding::AsciiFixed,
                image: None,
                enum_render_overrides: None,
                is_link: false,
                filename: file.filename(),
                compound_projection: Some(CompoundFieldProjection {
                    field_path: vec![],
                    field_type: TypeDescriptor::FixedAscii(8),
                    virtual_path: "/values/labels".to_string(),
                }),
            },
        ));

        assert_eq!(node.content_show_modes(), vec![ContentShowMode::Matrix]);
    }

    #[test]
    fn multi_value_string_datasets_are_matrix_only() {
        let temp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let file = hdf5_metno::File::create(temp.path()).expect("failed to create hdf5 file");
        let dataset = file
            .new_dataset_builder()
            .with_data(&[1_i16, 2_i16])
            .create("values")
            .expect("failed to create dataset");
        let node = H5FNode::new(Node::Dataset(
            dataset,
            DatasetMeta {
                link_name: None,
                display_name: "labels".to_string(),
                shape: vec![2],
                data_type: "string".to_string(),
                unsupported_reason: None,
                type_descriptor: TypeDescriptor::VarLenUnicode,
                data_bytesize: 8,
                storage_required: 16,
                total_bytes: 16,
                total_elems: 2,
                chunk_shape: None,
                hl: None,
                matrixable: Some(MatrixRenderType::Strings),
                encoding: Encoding::UTF8,
                image: None,
                enum_render_overrides: None,
                is_link: false,
                filename: file.filename(),
                compound_projection: None,
            },
        ));

        assert_eq!(node.content_show_modes(), vec![ContentShowMode::Matrix]);
    }

    #[test]
    fn one_dimensional_compound_roots_support_preview_and_matrix() {
        let temp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let file = hdf5_metno::File::create(temp.path()).expect("failed to create hdf5 file");
        let dataset = file
            .new_dataset_builder()
            .with_data(&[1_i16, 2_i16])
            .create("values")
            .expect("failed to create dataset");
        let node = H5FNode::new(Node::Dataset(
            dataset,
            DatasetMeta {
                link_name: None,
                display_name: "records".to_string(),
                shape: vec![2],
                data_type: "{id, label}".to_string(),
                unsupported_reason: None,
                type_descriptor: TypeDescriptor::Compound(CompoundType {
                    fields: vec![],
                    size: 16,
                }),
                data_bytesize: 16,
                storage_required: 32,
                total_bytes: 32,
                total_elems: 2,
                chunk_shape: None,
                hl: None,
                matrixable: None,
                encoding: Encoding::Unknown,
                image: None,
                enum_render_overrides: None,
                is_link: false,
                filename: file.filename(),
                compound_projection: Some(crate::h5f::root_compound_projection(
                    "/values",
                    CompoundType {
                        fields: vec![],
                        size: 16,
                    },
                )),
            },
        ));

        assert_eq!(
            node.content_show_modes(),
            vec![ContentShowMode::Preview, ContentShowMode::Matrix]
        );
    }

    #[test]
    fn multi_axis_compound_roots_support_preview_and_matrix() {
        let temp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let file = hdf5_metno::File::create(temp.path()).expect("failed to create hdf5 file");
        let dataset = file
            .new_dataset_builder()
            .with_data(&[1_i16, 2_i16])
            .create("values")
            .expect("failed to create dataset");
        let node = H5FNode::new(Node::Dataset(
            dataset,
            DatasetMeta {
                link_name: None,
                display_name: "records".to_string(),
                shape: vec![2, 2],
                data_type: "{id, label}".to_string(),
                unsupported_reason: None,
                type_descriptor: TypeDescriptor::Compound(CompoundType {
                    fields: vec![],
                    size: 16,
                }),
                data_bytesize: 16,
                storage_required: 64,
                total_bytes: 64,
                total_elems: 4,
                chunk_shape: None,
                hl: None,
                matrixable: None,
                encoding: Encoding::Unknown,
                image: None,
                enum_render_overrides: None,
                is_link: false,
                filename: file.filename(),
                compound_projection: Some(crate::h5f::root_compound_projection(
                    "/values",
                    CompoundType {
                        fields: vec![],
                        size: 16,
                    },
                )),
            },
        ));

        assert_eq!(
            node.content_show_modes(),
            vec![ContentShowMode::Preview, ContentShowMode::Matrix]
        );
    }

    #[test]
    fn multi_value_opaque_datasets_are_matrix_only() {
        let temp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let file = hdf5_metno::File::create(temp.path()).expect("failed to create hdf5 file");
        let dataset = file
            .new_dataset_builder()
            .with_data(&[1_i16, 2_i16])
            .create("values")
            .expect("failed to create dataset");
        let node = H5FNode::new(Node::Dataset(
            dataset,
            DatasetMeta {
                link_name: None,
                display_name: "opaque".to_string(),
                shape: vec![16],
                data_type: "opaque[32 bytes]".to_string(),
                unsupported_reason: Some("Unsupported datatype class".to_string()),
                type_descriptor: TypeDescriptor::VarLenAscii,
                data_bytesize: 32,
                storage_required: 512,
                total_bytes: 512,
                total_elems: 16,
                chunk_shape: None,
                hl: None,
                matrixable: Some(MatrixRenderType::Opaque),
                encoding: Encoding::Unknown,
                image: None,
                enum_render_overrides: None,
                is_link: false,
                filename: file.filename(),
                compound_projection: None,
            },
        ));

        assert_eq!(node.content_show_modes(), vec![ContentShowMode::Matrix]);
    }

    #[test]
    fn multi_value_varlen_byte_arrays_are_matrix_only_without_image_attrs() {
        let temp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let file = hdf5_metno::File::create(temp.path()).expect("failed to create hdf5 file");
        let dataset = file
            .new_dataset_builder()
            .with_data(&[1_i16, 2_i16])
            .create("values")
            .expect("failed to create dataset");
        let node = H5FNode::new(Node::Dataset(
            dataset,
            DatasetMeta {
                link_name: None,
                display_name: "bytes".to_string(),
                shape: vec![2],
                data_type: "[]u8".to_string(),
                unsupported_reason: None,
                type_descriptor: TypeDescriptor::VarLenArray(Box::new(TypeDescriptor::Unsigned(
                    hdf5_metno::types::IntSize::U1,
                ))),
                data_bytesize: 8,
                storage_required: 16,
                total_bytes: 16,
                total_elems: 2,
                chunk_shape: None,
                hl: None,
                matrixable: Some(MatrixRenderType::ByteArray),
                encoding: Encoding::Unknown,
                image: None,
                enum_render_overrides: None,
                is_link: false,
                filename: file.filename(),
                compound_projection: None,
            },
        ));

        assert_eq!(node.content_show_modes(), vec![ContentShowMode::Matrix]);
    }

    #[test]
    fn rank_two_numeric_datasets_support_heatmap_mode() {
        let temp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let file = hdf5_metno::File::create(temp.path()).expect("failed to create hdf5 file");
        let dataset = file
            .new_dataset_builder()
            .with_data(arr2(&[[1.0_f64, 2.0_f64], [3.0_f64, 4.0_f64]]).view())
            .create("values")
            .expect("failed to create dataset");
        let node = H5FNode::new(Node::Dataset(
            dataset,
            DatasetMeta {
                link_name: None,
                display_name: "values".to_string(),
                shape: vec![2, 2],
                data_type: "float64".to_string(),
                unsupported_reason: None,
                type_descriptor: TypeDescriptor::Float(hdf5_metno::types::FloatSize::U8),
                data_bytesize: 8,
                storage_required: 32,
                total_bytes: 32,
                total_elems: 4,
                chunk_shape: None,
                hl: None,
                matrixable: Some(MatrixRenderType::Float64),
                encoding: Encoding::UTF8,
                image: None,
                enum_render_overrides: None,
                is_link: false,
                filename: file.filename(),
                compound_projection: None,
            },
        ));

        assert_eq!(
            node.content_show_modes(),
            vec![
                ContentShowMode::Matrix,
                ContentShowMode::Heatmap,
                ContentShowMode::Preview
            ]
        );
    }

    #[test]
    fn rank_one_numeric_datasets_do_not_support_heatmap_mode() {
        let temp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let file = hdf5_metno::File::create(temp.path()).expect("failed to create hdf5 file");
        let dataset = file
            .new_dataset_builder()
            .with_data(&[1.0_f64, 2.0_f64])
            .create("values")
            .expect("failed to create dataset");
        let node = H5FNode::new(Node::Dataset(
            dataset,
            DatasetMeta {
                link_name: None,
                display_name: "values".to_string(),
                shape: vec![2],
                data_type: "float64".to_string(),
                unsupported_reason: None,
                type_descriptor: TypeDescriptor::Float(hdf5_metno::types::FloatSize::U8),
                data_bytesize: 8,
                storage_required: 16,
                total_bytes: 16,
                total_elems: 2,
                chunk_shape: None,
                hl: None,
                matrixable: Some(MatrixRenderType::Float64),
                encoding: Encoding::UTF8,
                image: None,
                enum_render_overrides: None,
                is_link: false,
                filename: file.filename(),
                compound_projection: None,
            },
        ));

        assert_eq!(
            node.content_show_modes(),
            vec![ContentShowMode::Matrix, ContentShowMode::Preview]
        );
    }
}
