use ratatui::layout::Rect;

use super::ContentShowMode;

#[derive(Debug, Clone, Copy)]
pub struct TreeHitbox {
    pub outer: Rect,
    pub inner: Rect,
    pub row_offset: usize,
    pub visible_rows: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct MetadataCellHitbox {
    pub row_index: usize,
    pub name_area: Rect,
    pub value_area: Rect,
}

#[derive(Debug, Clone)]
pub struct AttributesHitbox {
    pub outer: Rect,
    pub inner: Rect,
    pub cells: Vec<MetadataCellHitbox>,
}

#[derive(Debug, Clone, Copy)]
pub struct ContentTabHitbox {
    pub area: Rect,
    pub mode: ContentShowMode,
}

#[derive(Debug, Clone, Copy)]
pub struct MatrixRowHitbox {
    pub area: Rect,
    pub row: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct MatrixCellHitbox {
    pub area: Rect,
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct HeatmapSettingHitbox {
    pub area: Rect,
    pub setting: usize,
}

#[derive(Debug, Clone, Default)]
pub struct UiLayoutState {
    pub tree: Option<TreeHitbox>,
    pub attributes: Option<AttributesHitbox>,
    pub content: Option<Rect>,
    pub content_tabs: Vec<ContentTabHitbox>,
    pub matrix_rows: Vec<MatrixRowHitbox>,
    pub matrix_cells: Vec<MatrixCellHitbox>,
    pub heatmap_settings: Vec<HeatmapSettingHitbox>,
}
