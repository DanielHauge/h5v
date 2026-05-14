use std::time::SystemTime;

use crate::{error::FixedStringOverflow, h5f::AttributeCreateType};

#[allow(dead_code)]
#[cfg(target_os = "windows")]
pub fn display_path(path: &str) -> String {
    path.replace('/', "\\")
}

#[allow(dead_code)]
#[cfg(not(target_os = "windows"))]
pub fn display_path(path: &str) -> String {
    path.to_string()
}

#[derive(Debug, Clone)]
pub enum LastFocused {
    Attributes,
    Content,
}

#[derive(Debug, Clone)]
pub enum Focus {
    Tree(LastFocused),
    Attributes,
    Content,
}

#[derive(Debug, Clone)]
pub enum Mode {
    Normal,
    Search,
    Help,
    Command,
    MultiChart,
    AttributeCreateDialog,
    AttributeDeleteDialog,
    FixedStringOverflowDialog,
    FixedStringResizeDialog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingChord {
    CtrlW,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum ContentShowMode {
    Preview,
    Matrix,
    Heatmap,
}

impl ContentShowMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "preview" => Some(Self::Preview),
            "matrix" => Some(Self::Matrix),
            "heatmap" => Some(Self::Heatmap),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Matrix => "matrix",
            Self::Heatmap => "heatmap",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeViewSelection {
    Name,
    Value,
}

#[derive(Clone)]
pub struct AttributeEditRequest {
    pub attr_name: String,
    pub content: String,
    pub selection: AttributeViewSelection,
    pub edit_name_hint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeCreateField {
    Name,
    Type,
    Value,
}

#[derive(Clone)]
pub struct AttributeCreateDialogState {
    pub name: String,
    pub name_cursor: usize,
    pub attr_type: AttributeCreateType,
    pub value: String,
    pub value_cursor: usize,
    pub active_field: AttributeCreateField,
}

#[derive(Clone)]
pub struct AttributeDeleteDialogState {
    pub attr_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedStringOverflowChoice {
    Cancel,
    ChangeToVarLen,
    ChangeSize,
}

#[derive(Clone)]
pub struct FixedStringOverflowDialogState {
    pub request: AttributeEditRequest,
    pub new_value: String,
    pub overflow: FixedStringOverflow,
    pub selected_choice: FixedStringOverflowChoice,
    pub size_input: String,
}

#[derive(Debug, Clone)]
pub struct AttributeCursor {
    pub attribute_index: usize,
    pub attribute_view_selection: AttributeViewSelection,
    pub attribute_offset: usize,
}

impl Default for AttributeCursor {
    fn default() -> Self {
        Self {
            attribute_index: 0,
            attribute_view_selection: AttributeViewSelection::Name,
            attribute_offset: 0,
        }
    }
}

#[derive(Clone)]
pub struct MatrixViewState {
    pub col_offset: usize,
    pub row_offset: usize,
    pub rows_currently_available: usize,
    pub cols_currently_available: usize,
    pub cursor_row: usize,
    pub cursor_col: usize,
}

pub enum AppToast {
    Empty,
    Info(String),
    Warning(String),
    Error(String),
}

pub struct FileWatchState {
    pub path: String,
    pub linked: bool,
    pub last_known_modified: Option<SystemTime>,
    pub pending_external_change: bool,
}
