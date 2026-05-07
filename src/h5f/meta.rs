use hdf5_metno::types::{CompoundField, CompoundType, TypeDescriptor};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::{color_consts, sprint_typedesc::MatrixRenderType};

use super::RenderedAttributeRow;

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
pub struct CompoundFieldPathSegment {
    pub name: String,
    pub offset: usize,
}

#[derive(Debug, Clone)]
pub struct CompoundFieldProjection {
    pub field_path: Vec<CompoundFieldPathSegment>,
    pub field_type: TypeDescriptor,
    pub virtual_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumRenderOverrides {
    pub colors: Vec<Option<Color>>,
    pub symbols: Vec<Option<String>>,
}

impl EnumRenderOverrides {
    pub fn is_empty(&self) -> bool {
        self.colors.iter().all(Option::is_none) && self.symbols.iter().all(Option::is_none)
    }
}

#[derive(Debug, Clone)]
pub struct GroupMeta {
    pub is_link: bool,
    pub filename: String,
    pub display_name: String,
    pub preview_expr: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DatasetMeta {
    pub link_name: Option<String>,
    pub display_name: String,
    pub shape: Vec<usize>,
    pub data_type: String,
    pub unsupported_reason: Option<String>,
    pub type_descriptor: TypeDescriptor,
    #[allow(dead_code)]
    pub(crate) data_bytesize: usize,
    pub(crate) storage_required: u64,
    pub(crate) total_bytes: usize,
    pub(crate) total_elems: usize,
    pub(crate) chunk_shape: Option<Vec<usize>>,
    pub hl: Option<String>,
    pub matrixable: Option<MatrixRenderType>,
    pub encoding: Encoding,
    pub image: Option<ImageType>,
    pub enum_render_overrides: Option<EnumRenderOverrides>,
    pub is_link: bool,
    pub filename: String,
    pub compound_projection: Option<CompoundFieldProjection>,
}

impl CompoundFieldProjection {
    pub fn current_compound_type(&self) -> Option<&CompoundType> {
        match &self.field_type {
            TypeDescriptor::Compound(compound) => Some(compound),
            _ => None,
        }
    }

    pub fn absolute_offset(&self) -> usize {
        self.field_path.iter().map(|segment| segment.offset).sum()
    }

    pub fn child(&self, field: &CompoundField) -> Self {
        let mut field_path = self.field_path.clone();
        field_path.push(CompoundFieldPathSegment {
            name: field.name.clone(),
            offset: field.offset,
        });
        Self {
            virtual_path: format!("{}/{}", self.virtual_path, field.name),
            field_path,
            field_type: field.ty.clone(),
        }
    }
}

impl GroupMeta {
    pub fn render(&self, longest_name: u16) -> Vec<RenderedAttributeRow> {
        let min_first_panel = match longest_name {
            0..8 => 8,
            8..=u16::MAX => longest_name,
        };
        let mut data_set_attrs = vec![];

        if self.is_link {
            let name = "link";
            let external_value = Span::styled(
                self.filename.clone(),
                Style::default()
                    .fg(color_consts::BUILT_IN_VALUE_COLOR)
                    .bold(),
            );
            data_set_attrs.push((name, external_value));
        }

        let mut lines = vec![];
        for (name, value) in data_set_attrs {
            let name_len = name.len();
            let name_span = Span::styled(
                name,
                Style::default()
                    .fg(color_consts::VARIABLE_BLUE_BUILTIN)
                    .bold(),
            );
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
            let name_line = Line::from(vec![name_span, name_helper_line, equals_sign]);

            let value_line = Line::from(vec![value]);
            let empty_line = Line::from(vec![Span::raw("")]);
            lines.push(RenderedAttributeRow::property(
                name,
                (name_line, value_line, empty_line),
            ));
        }

        lines
    }
}

pub static SYSTEM_PROPERTIES: [&str; 8] = [
    "type", "size", "shape", "chunk", "link", "path", "origin", "field",
];

impl DatasetMeta {
    pub fn is_opaque(&self) -> bool {
        self.unsupported_reason.is_some()
    }

    pub fn virtual_path(&self) -> Option<&str> {
        self.compound_projection
            .as_ref()
            .map(|projection| projection.virtual_path.as_str())
    }

    pub fn is_compound_container(&self) -> bool {
        self.compound_projection
            .as_ref()
            .and_then(CompoundFieldProjection::current_compound_type)
            .is_some()
    }

    pub fn is_compound_leaf(&self) -> bool {
        self.compound_projection.is_some() && !self.is_compound_container()
    }

    pub fn render(&self, longest_name: u16) -> Vec<RenderedAttributeRow> {
        let min_first_panel = match longest_name {
            0..8 => 8,
            8..=u16::MAX => longest_name,
        };
        let mut data_set_attrs = vec![];
        let type_value = Span::styled(
            self.data_type_string(),
            Style::default()
                .fg(color_consts::BUILT_IN_VALUE_COLOR)
                .bold(),
        );
        data_set_attrs.push(("type", type_value));

        let size_value = Span::styled(
            self.size_string(),
            Style::default()
                .fg(color_consts::BUILT_IN_VALUE_COLOR)
                .bold(),
        );
        data_set_attrs.push(("size", size_value));
        let shape_value = Span::styled(
            self.shape_string(),
            Style::default()
                .fg(color_consts::BUILT_IN_VALUE_COLOR)
                .bold(),
        );
        data_set_attrs.push(("shape", shape_value));
        if let Some(chunk_shape) = &self.chunk_shape_string() {
            let chunk_value = Span::styled(
                chunk_shape.to_string(),
                Style::default()
                    .fg(color_consts::BUILT_IN_VALUE_COLOR)
                    .bold(),
            );
            data_set_attrs.push(("chunk", chunk_value));
        }

        if self.is_link {
            let external_value = Span::styled(
                self.filename.clone(),
                Style::default()
                    .fg(color_consts::BUILT_IN_VALUE_COLOR)
                    .bold(),
            );
            data_set_attrs.push(("link", external_value));
        }
        if let Some(l_name) = &self.link_name {
            let link_value_span = Span::styled(
                l_name.clone(),
                Style::default()
                    .fg(color_consts::BUILT_IN_VALUE_COLOR)
                    .bold(),
            );
            data_set_attrs.push(("origin", link_value_span));
        }
        if let Some(virtual_path) = self.virtual_path() {
            let field_value = Span::styled(
                virtual_path.to_string(),
                Style::default()
                    .fg(color_consts::BUILT_IN_VALUE_COLOR)
                    .bold(),
            );
            data_set_attrs.push(("field", field_value));
        }

        let mut lines = vec![];
        for (name, value) in data_set_attrs {
            let name_len = name.len();
            let name_span = Span::styled(
                name,
                Style::default()
                    .fg(color_consts::VARIABLE_BLUE_BUILTIN)
                    .bold(),
            );
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
            let name_line = Line::from(vec![name_span, name_helper_line, equals_sign]);

            let value_line = Line::from(vec![value]);
            let empty_line = Line::from(vec![Span::raw("")]);
            lines.push(RenderedAttributeRow::property(
                name,
                (name_line, value_line, empty_line),
            ));
        }

        lines
    }

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

    pub fn is_empty(&self) -> bool {
        self.total_elems == 0
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
