use std::{
    cell::RefCell,
    fs,
    io::BufReader,
    rc::Rc,
    sync::{mpsc::Sender, Arc, RwLock},
    time::{Duration, Instant, SystemTime},
};

use arboard::Clipboard;
use hdf5_metno::{ByteReader, Dataset, File};
use image::ImageFormat;
use ratatui_image::thread::ThreadProtocol;

use crate::{
    configure,
    data::{DatasetPlotingData, PreviewSelection, Previewable, SliceSelection},
    error::{AppError, FixedStringOverflow},
    h5f::{plot_projected, AttributeCreateType, H5FNode, HasPath, ImageType, Node},
    search::Searcher,
    ui::mchart::{ChartSource, DatasetChartKind, DatasetChartSource, MultiChartState, Point},
};

use super::{
    command::{execute_command, CommandState},
    input::EventResult,
    preview::chart::MAX_SEGMENT_SIZE,
    tree_view::TreeItem,
};

mod heatmap;
mod help_state;
mod ui_layout;
use heatmap::{clamp_heatmap_viewport, heatmap_anchor_fraction, heatmap_partition};
#[allow(unused_imports)]
pub use heatmap::{
    HeatmapCachedPage, HeatmapColormap, HeatmapCustomRangeMode, HeatmapDragState,
    HeatmapLegendSummary, HeatmapLoadPriority, HeatmapLoadRequest, HeatmapLoadedPage,
    HeatmapNormalization, HeatmapPageWindow, HeatmapRangeBound, HeatmapRangeMode,
    HeatmapRegionSelection, HeatmapRenderKey, HeatmapRenderState, HeatmapSegmentAxis,
    HeatmapSelectedCells, HeatmapSettingField, HeatmapSettings, HeatmapSliceSummary,
    HeatmapStoredFloat, HeatmapViewport, HEATMAP_SETTING_FIELDS,
};
pub use help_state::{
    HelpCommandSection, HelpCustomizationSection, HelpKeymapSection, HelpTab, HelpViewState,
};
pub use ui_layout::{
    AttributesHitbox, ContentTabHitbox, HeatmapSettingHitbox, MatrixCellHitbox, MatrixRowHitbox,
    MetadataCellHitbox, TreeHitbox, UiLayoutState,
};

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

pub struct ChartPreviewLoadRequest {
    pub ds_path: String,
    pub source: ChartPreviewSource,
    pub segment_state: SegmentState,
    pub selection: PreviewSelection,
    pub width: u16,
    pub height: u16,
}

pub enum ChartPreviewSource {
    Dataset {
        ds: Dataset,
        selection: PreviewSelection,
    },
    Precomputed {
        data_preview: DatasetPlotingData,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardImageData {
    pub width: usize,
    pub height: usize,
    pub bytes: Vec<u8>,
}

pub struct ChartPreviwState {
    pub ds_loaded: Option<String>,
    pub protocol: Option<ThreadProtocol>,
    pub clipboard_image: Option<ClipboardImageData>,
    pub error: Option<String>,
    pub ds_selection: Option<PreviewSelection>,
    pub tx_load_chartpreview: Sender<ChartPreviewLoadRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageLoadKey {
    pub ds_path: String,
    pub idx: i32,
    pub window_axis: Option<ImageWindowAxis>,
    pub window_start: usize,
    pub window_len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageWindowAxis {
    Rows,
    Cols,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageWindowState {
    pub ds_path: String,
    pub axis: ImageWindowAxis,
    pub start: usize,
    pub len: usize,
    pub total: usize,
}

impl ImageWindowState {
    pub fn end(&self) -> usize {
        self.start + self.len
    }

    pub fn label(&self) -> &'static str {
        match self.axis {
            ImageWindowAxis::Rows => "rows",
            ImageWindowAxis::Cols => "cols",
        }
    }

    pub fn centered_start(total: usize, len: usize, target: usize) -> usize {
        let max_start = total.saturating_sub(len);
        target.saturating_sub(len / 2).min(max_start)
    }

    pub fn shift_by(&mut self, delta: isize) {
        let max_start = self.total.saturating_sub(self.len);
        let next = self.start as isize + delta;
        self.start = next.clamp(0, max_start as isize) as usize;
    }

    pub fn center_on(&mut self, idx: usize) {
        self.start =
            Self::centered_start(self.total, self.len, idx.min(self.total.saturating_sub(1)));
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChartPreviewKey {
    pub ds_path: String,
    pub selection: PreviewSelection,
}

pub struct RawImageLoadRequest {
    pub key: ImageLoadKey,
    pub reader: BufReader<ByteReader>,
    pub format: ImageFormat,
}

pub struct VarLenImageLoadRequest {
    pub key: ImageLoadKey,
    pub dataset: Dataset,
    pub format: ImageFormat,
}

pub struct DatasetImageLoadRequest {
    pub key: ImageLoadKey,
    pub dataset: Dataset,
    pub image_type: ImageType,
    pub window: Option<ImageWindowState>,
}

pub struct ImgState {
    pub protocol: Option<ThreadProtocol>,
    pub tx_load_imgfs: Sender<RawImageLoadRequest>,
    pub tx_load_imgfsvlen: Sender<VarLenImageLoadRequest>,
    pub tx_load_img: Sender<DatasetImageLoadRequest>,
    pub ds: Option<String>,
    pub current_key: Option<ImageLoadKey>,
    pub clipboard_image: Option<ClipboardImageData>,
    pub window: Option<ImageWindowState>,
    pub error: Option<String>,
    pub idx_to_load: i32,
    pub idx_loaded: i32,
}

pub trait IsFromDsReq {
    fn get_ds_name(&self) -> Option<String>;
}

pub trait IsFromDs {
    fn is_from_ds(&self, node: &Node) -> bool;
}

impl<T: IsFromDsReq> IsFromDs for T {
    fn is_from_ds(&self, node: &Node) -> bool {
        let ds_name = match self.get_ds_name() {
            Some(name) => name,
            None => return false,
        };
        node.path() == ds_name
    }
}

impl IsFromDsReq for ChartPreviwState {
    fn get_ds_name(&self) -> Option<String> {
        self.ds_loaded.clone()
    }
}

impl IsFromDsReq for ImgState {
    fn get_ds_name(&self) -> Option<String> {
        self.ds.clone()
    }
}

impl ImgState {
    pub fn current_request_key(&self) -> Option<ImageLoadKey> {
        self.current_key.clone()
    }
}

impl ChartPreviwState {
    pub fn current_request_key(&self) -> Option<ChartPreviewKey> {
        Some(ChartPreviewKey {
            ds_path: self.ds_loaded.clone()?,
            selection: self.ds_selection.clone()?,
        })
    }
}

#[derive(Debug, Clone)]
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

#[derive(Clone)]
pub enum SegmentType {
    Image,
    Chart,
    NoSegment,
}

#[derive(Clone)]
pub struct SegmentState {
    pub idx: i32,
    pub segumented: SegmentType,
    pub segment_count: i32,
}

impl SegmentState {
    fn max_index(&self) -> Option<i32> {
        (self.segment_count > 0).then_some(self.segment_count.saturating_sub(1))
    }
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

pub struct AppState<'a> {
    pub readonly: bool,
    pub root: Rc<RefCell<H5FNode>>,
    pub treeview: Vec<TreeItem<'a>>,
    pub file: Option<File>,
    pub editing: bool,
    pub edit_pause: Arc<RwLock<()>>,
    pub tree_view_cursor: usize,
    pub clipboard: Option<Clipboard>,
    pub clipboard_init_error: Option<String>,
    pub copying: bool,
    pub toast: AppToast,
    pub configuration_warning: Option<String>,
    pub file_watch: FileWatchState,
    pub compatibility_mode: bool,
    pub focus: Focus,
    pub multi_chart: MultiChartState,
    pub mode: Mode,
    pub command_return_mode: Mode,
    pub help_return_mode: Mode,
    pub searcher: Option<Searcher>,
    pub help: HelpViewState,
    pub pending_chord: Option<PendingChord>,
    pub binding_command_depth: usize,
    pub show_tree_view: bool,
    pub stacked_tree_layout: bool,
    pub image_protocol_enabled: bool,
    pub image_cell_size: (u16, u16),
    pub preview_debounce_generation: u64,
    pub preview_debounce_until: Option<Instant>,
    pub preview_debounce_path: Option<String>,
    pub content_mode: ContentShowMode,
    pub img_state: ImgState,
    pub matrix_view_state: MatrixViewState,
    pub heatmap_viewport_region: Option<HeatmapRegionSelection>,
    pub heatmap_region: Option<HeatmapRegionSelection>,
    pub heatmap_render: HeatmapRenderState,
    pub chart_preview_state: ChartPreviwState,
    pub segment_state: SegmentState,
    pub command_state: CommandState,
    pub attribute_create_dialog: Option<AttributeCreateDialogState>,
    pub attribute_delete_dialog: Option<AttributeDeleteDialogState>,
    pub fixed_string_overflow_dialog: Option<FixedStringOverflowDialogState>,
    pub ui_layout: UiLayoutState,
}

pub(crate) fn preview_selection_for_node(
    node: &mut H5FNode,
    shape: &[usize],
    segment_idx: i32,
) -> Option<PreviewSelection> {
    let total_dims = shape.len();
    node.sync_selection_rank(total_dims);
    let x_selectable_dims: Vec<usize> = shape
        .iter()
        .enumerate()
        .filter(|(_, len)| **len > 1)
        .map(|(idx, _)| idx)
        .collect();

    if x_selectable_dims.is_empty() {
        return None;
    }

    for (idx, selected_index) in node.selected_indexes.iter_mut().enumerate() {
        if !x_selectable_dims.contains(&idx) {
            *selected_index = 0;
        }
    }

    if !x_selectable_dims.contains(&node.selected_x) {
        let first_selectable_dim = x_selectable_dims.first().copied()?;
        node.selected_x = first_selectable_dim;
    }
    if node.selected_dim == node.selected_x {
        node.selected_dim = x_selectable_dims
            .iter()
            .find(|&&x| x != node.selected_x)
            .copied()
            .unwrap_or(0);
    }

    let slice = if shape[node.selected_x] > MAX_SEGMENT_SIZE {
        let start = MAX_SEGMENT_SIZE * segment_idx.max(0) as usize;
        let end = (start + MAX_SEGMENT_SIZE).min(shape[node.selected_x]);
        SliceSelection::FromTo(start, end)
    } else {
        SliceSelection::All
    };

    Some(PreviewSelection {
        x: node.selected_x,
        index: node.selected_indexes.get(..total_dims)?.to_vec(),
        slice,
    })
}

type Result<T> = std::result::Result<T, AppError>;
impl AppState<'_> {
    const PREVIEW_DEBOUNCE_DELAY: Duration = Duration::from_millis(90);

    fn active_heatmap_shape(&self) -> Option<(usize, usize)> {
        let tree_item = self.treeview.get(self.tree_view_cursor)?;
        let node = tree_item.node.borrow();
        let Node::Dataset(_, attr) = &node.node else {
            return None;
        };
        Some((attr.shape[node.selected_row], attr.shape[node.selected_col]))
    }

    fn base_heatmap_viewport(&self, rows: usize, cols: usize) -> HeatmapViewport {
        self.heatmap_render.viewport.unwrap_or(HeatmapViewport {
            row_start: 0,
            row_len: rows.max(1),
            col_start: 0,
            col_len: cols.max(1),
        })
    }

    fn current_heatmap_visible_viewport(&self) -> Option<HeatmapViewport> {
        let (rows, cols) = self.active_heatmap_shape()?;
        let base = clamp_heatmap_viewport(self.base_heatmap_viewport(rows, cols), rows, cols);
        let Some(window) = self.heatmap_render.segment.as_ref() else {
            return Some(base);
        };
        let (start, end) = window.current_range();
        Some(match window.axis {
            HeatmapSegmentAxis::Rows => HeatmapViewport {
                row_start: base.row_start + start,
                row_len: end.saturating_sub(start).max(1),
                col_start: base.col_start,
                col_len: base.col_len,
            },
            HeatmapSegmentAxis::Cols => HeatmapViewport {
                row_start: base.row_start,
                row_len: base.row_len,
                col_start: base.col_start + start,
                col_len: end.saturating_sub(start).max(1),
            },
        })
    }

    fn invalidate_heatmap_render(&mut self, clear_segment: bool) {
        self.heatmap_viewport_region = None;
        self.heatmap_region = None;
        self.heatmap_render.drag_state = None;
        self.heatmap_render.current_key = None;
        self.heatmap_render.current_selection = None;
        self.heatmap_render.current_slice_summary = None;
        if clear_segment {
            self.heatmap_render.segment = None;
        }
    }

    fn heatmap_viewport_for_cell(&self, row: usize, col: usize) -> Option<HeatmapViewport> {
        let visible = self.current_heatmap_visible_viewport()?;
        let viewport_rows = self.matrix_view_state.rows_currently_available.max(1);
        let viewport_cols = self.matrix_view_state.cols_currently_available.max(1);
        let (display_y0, display_y1) = heatmap_partition(visible.row_len, viewport_rows, row);
        let (display_x0, display_x1) = heatmap_partition(visible.col_len, viewport_cols, col);
        let (src_y0, src_y1) = if self.heatmap_render.settings.invert_y {
            (
                visible.row_len.saturating_sub(display_y1),
                visible.row_len.saturating_sub(display_y0),
            )
        } else {
            (display_y0, display_y1)
        };
        let (src_x0, src_x1) = if self.heatmap_render.settings.invert_x {
            (
                visible.col_len.saturating_sub(display_x1),
                visible.col_len.saturating_sub(display_x0),
            )
        } else {
            (display_x0, display_x1)
        };
        Some(HeatmapViewport {
            row_start: visible.row_start + src_y0,
            row_len: src_y1.saturating_sub(src_y0).max(1),
            col_start: visible.col_start + src_x0,
            col_len: src_x1.saturating_sub(src_x0).max(1),
        })
    }

    fn centered_heatmap_viewport(
        &self,
        current: HeatmapViewport,
        row_len: usize,
        col_len: usize,
        max_rows: usize,
        max_cols: usize,
    ) -> HeatmapViewport {
        let center_row = current.row_start + current.row_len / 2;
        let center_col = current.col_start + current.col_len / 2;
        clamp_heatmap_viewport(
            HeatmapViewport {
                row_start: center_row.saturating_sub(row_len / 2),
                row_len,
                col_start: center_col.saturating_sub(col_len / 2),
                col_len,
            },
            max_rows,
            max_cols,
        )
    }

    pub fn heatmap_select_cell(&mut self, row: usize, col: usize) -> bool {
        let next = match self.heatmap_render.selected_cells {
            None => HeatmapSelectedCells::single(row, col),
            Some(existing)
                if existing.is_single()
                    && (existing.row_start != row || existing.col_start != col) =>
            {
                HeatmapSelectedCells::normalized(existing.row_start, existing.col_start, row, col)
            }
            Some(existing) if !existing.is_single() => {
                self.heatmap_render.selected_cells = None;
                self.invalidate_heatmap_render(false);
                return true;
            }
            _ => HeatmapSelectedCells::single(row, col),
        };
        if self.heatmap_render.selected_cells == Some(next) {
            return false;
        }
        self.heatmap_render.selected_cells = Some(next);
        self.invalidate_heatmap_render(false);
        true
    }

    pub fn clear_heatmap_selection(&mut self) -> bool {
        if self.heatmap_render.selected_cells.is_none() {
            return false;
        }
        self.heatmap_render.selected_cells = None;
        self.invalidate_heatmap_render(false);
        true
    }

    pub fn reset_heatmap_view(&mut self) -> bool {
        if self.heatmap_render.viewport.is_none() && self.heatmap_render.selected_cells.is_none() {
            return false;
        }
        self.heatmap_render.viewport = None;
        self.heatmap_render.selected_cells = None;
        self.invalidate_heatmap_render(true);
        true
    }

    pub fn zoom_heatmap(&mut self, focus_cell: Option<(usize, usize)>, zoom_in: bool) -> bool {
        let Some((rows, cols)) = self.active_heatmap_shape() else {
            return false;
        };
        let full = HeatmapViewport {
            row_start: 0,
            row_len: rows.max(1),
            col_start: 0,
            col_len: cols.max(1),
        };
        let visible = self.current_heatmap_visible_viewport().unwrap_or(full);
        let next = if zoom_in {
            if self.heatmap_render.selected_cells.is_some() {
                let Some(region) = self.heatmap_region.as_ref() else {
                    return false;
                };
                clamp_heatmap_viewport(
                    HeatmapViewport {
                        row_start: region.y,
                        row_len: region.height.max(1),
                        col_start: region.x,
                        col_len: region.width.max(1),
                    },
                    rows,
                    cols,
                )
            } else if let Some((row, col)) = focus_cell {
                let Some(cell_view) = self.heatmap_viewport_for_cell(row, col) else {
                    return false;
                };
                clamp_heatmap_viewport(cell_view, rows, cols)
            } else {
                self.centered_heatmap_viewport(
                    visible,
                    (visible.row_len / 2).max(1),
                    (visible.col_len / 2).max(1),
                    rows,
                    cols,
                )
            }
        } else {
            self.centered_heatmap_viewport(
                visible,
                (visible.row_len.saturating_mul(2)).min(rows.max(1)),
                (visible.col_len.saturating_mul(2)).min(cols.max(1)),
                rows,
                cols,
            )
        };
        let next_viewport = if next == full { None } else { Some(next) };
        if self.heatmap_render.viewport == next_viewport
            && self.heatmap_render.selected_cells.is_none()
        {
            return false;
        }
        self.heatmap_render.viewport = next_viewport;
        self.heatmap_render.selected_cells = None;
        self.invalidate_heatmap_render(true);
        true
    }

    pub fn zoom_heatmap_step(&mut self, focus_cell: Option<(usize, usize)>, zoom_in: bool) -> bool {
        let Some((rows, cols)) = self.active_heatmap_shape() else {
            return false;
        };
        let full = HeatmapViewport {
            row_start: 0,
            row_len: rows.max(1),
            col_start: 0,
            col_len: cols.max(1),
        };
        let visible = self.current_heatmap_visible_viewport().unwrap_or(full);
        let viewport_rows = self.matrix_view_state.rows_currently_available.max(1);
        let viewport_cols = self.matrix_view_state.cols_currently_available.max(1);
        let (focus_row, focus_col, focus_row_frac, focus_col_frac) =
            if let Some((row, col)) = focus_cell {
                if let Some(cell_view) = self.heatmap_viewport_for_cell(row, col) {
                    (
                        cell_view.row_start + cell_view.row_len / 2,
                        cell_view.col_start + cell_view.col_len / 2,
                        heatmap_anchor_fraction(
                            row,
                            viewport_rows,
                            self.heatmap_render.settings.invert_y,
                        ),
                        heatmap_anchor_fraction(
                            col,
                            viewport_cols,
                            self.heatmap_render.settings.invert_x,
                        ),
                    )
                } else {
                    (
                        visible.row_start + visible.row_len / 2,
                        visible.col_start + visible.col_len / 2,
                        0.5,
                        0.5,
                    )
                }
            } else {
                (
                    visible.row_start + visible.row_len / 2,
                    visible.col_start + visible.col_len / 2,
                    0.5,
                    0.5,
                )
            };
        let next_row_len = if zoom_in {
            (visible.row_len.saturating_mul(4) / 5).max(1)
        } else {
            (visible.row_len.saturating_mul(5) / 4).min(rows.max(1))
        };
        let next_col_len = if zoom_in {
            (visible.col_len.saturating_mul(4) / 5).max(1)
        } else {
            (visible.col_len.saturating_mul(5) / 4).min(cols.max(1))
        };
        let next = clamp_heatmap_viewport(
            HeatmapViewport {
                row_start: focus_row
                    .saturating_sub((focus_row_frac * next_row_len as f64).floor() as usize),
                row_len: next_row_len,
                col_start: focus_col
                    .saturating_sub((focus_col_frac * next_col_len as f64).floor() as usize),
                col_len: next_col_len,
            },
            rows,
            cols,
        );
        let next_viewport = if next == full { None } else { Some(next) };
        if self.heatmap_render.viewport == next_viewport {
            return false;
        }
        self.heatmap_render.viewport = next_viewport;
        self.heatmap_render.selected_cells = None;
        self.invalidate_heatmap_render(true);
        true
    }

    pub fn pan_heatmap_by(&mut self, dx: isize, dy: isize) -> bool {
        let Some((rows, cols)) = self.active_heatmap_shape() else {
            return false;
        };
        let Some(visible) = self.current_heatmap_visible_viewport() else {
            return false;
        };
        let step_row = (visible.row_len / 4).max(1) as isize;
        let step_col = (visible.col_len / 4).max(1) as isize;
        let next = clamp_heatmap_viewport(
            HeatmapViewport {
                row_start: visible
                    .row_start
                    .saturating_add_signed(dy.saturating_mul(step_row)),
                row_len: visible.row_len,
                col_start: visible
                    .col_start
                    .saturating_add_signed(dx.saturating_mul(step_col)),
                col_len: visible.col_len,
            },
            rows,
            cols,
        );
        let next_viewport = if next.row_start == 0
            && next.col_start == 0
            && next.row_len == rows.max(1)
            && next.col_len == cols.max(1)
        {
            None
        } else {
            Some(next)
        };
        if self.heatmap_render.viewport == next_viewport {
            return false;
        }
        self.heatmap_render.viewport = next_viewport;
        self.heatmap_render.selected_cells = None;
        self.invalidate_heatmap_render(true);
        true
    }

    pub fn start_heatmap_drag(&mut self, row: usize, col: usize) -> bool {
        let Some(visible_viewport) = self.current_heatmap_visible_viewport() else {
            return false;
        };
        self.heatmap_render.drag_state = Some(HeatmapDragState {
            anchor_row: row,
            anchor_col: col,
            visible_viewport,
        });
        true
    }

    pub fn finish_heatmap_drag(&mut self, row: usize, col: usize) -> bool {
        let Some(drag_state) = self.heatmap_render.drag_state.take() else {
            return false;
        };
        let Some((rows, cols)) = self.active_heatmap_shape() else {
            return false;
        };
        let viewport_rows = self.matrix_view_state.rows_currently_available.max(1);
        let viewport_cols = self.matrix_view_state.cols_currently_available.max(1);
        let delta_cols = col as isize - drag_state.anchor_col as isize;
        let delta_rows = row as isize - drag_state.anchor_row as isize;
        let sample_delta_x = ((delta_cols as f64 / viewport_cols as f64)
            * drag_state.visible_viewport.col_len as f64)
            .round() as isize;
        let sample_delta_y = ((delta_rows as f64 / viewport_rows as f64)
            * drag_state.visible_viewport.row_len as f64)
            .round() as isize;
        let next = clamp_heatmap_viewport(
            HeatmapViewport {
                row_start: drag_state
                    .visible_viewport
                    .row_start
                    .saturating_add_signed(-sample_delta_y),
                row_len: drag_state.visible_viewport.row_len,
                col_start: drag_state
                    .visible_viewport
                    .col_start
                    .saturating_add_signed(-sample_delta_x),
                col_len: drag_state.visible_viewport.col_len,
            },
            rows,
            cols,
        );
        let next_viewport = if next.row_start == 0
            && next.col_start == 0
            && next.row_len == rows.max(1)
            && next.col_len == cols.max(1)
        {
            None
        } else {
            Some(next)
        };
        if self.heatmap_render.viewport == next_viewport {
            return false;
        }
        self.heatmap_render.viewport = next_viewport;
        self.heatmap_render.selected_cells = None;
        self.invalidate_heatmap_render(true);
        true
    }

    pub fn end_heatmap_drag(&mut self) {
        self.heatmap_render.drag_state = None;
    }

    fn normalized_node_path(path: &str) -> &str {
        path.trim_start_matches('/')
    }

    fn current_file_modified(&self) -> Option<SystemTime> {
        fs::metadata(&self.file_watch.path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
    }

    pub fn clipboard_unavailable_message(&self) -> String {
        match &self.clipboard_init_error {
            Some(error) => format!("Clipboard is unavailable on this system: {error}"),
            None => "Clipboard is unavailable on this system".to_string(),
        }
    }

    pub fn set_clipboard_text(&mut self, text: String) -> std::result::Result<(), String> {
        let Some(clipboard) = self.clipboard.as_mut() else {
            return Err(self.clipboard_unavailable_message());
        };
        clipboard.set_text(text).map_err(|error| error.to_string())
    }

    pub fn sync_file_watch(&mut self) {
        self.file_watch.last_known_modified = self.current_file_modified();
        self.file_watch.pending_external_change = false;
    }

    pub fn acknowledge_file_write(&mut self) {
        self.sync_file_watch();
    }

    pub fn register_file_watch_change(&mut self) -> Option<AppToast> {
        if self.file_watch.pending_external_change {
            return None;
        }

        let current_modified = self.current_file_modified();
        if current_modified == self.file_watch.last_known_modified {
            return None;
        }

        self.file_watch.pending_external_change = true;
        Some(match current_modified {
            Some(_) => AppToast::Info("File changed on disk - press Ctrl-R to reload".to_string()),
            None => AppToast::Warning(
                "File changed or is unavailable on disk - press Ctrl-R to retry reload".to_string(),
            ),
        })
    }

    pub fn selected_tree_path(&self) -> Option<String> {
        self.treeview
            .get(self.tree_view_cursor)
            .map(|item| item.node.borrow().node.path())
    }

    pub fn select_tree_node_by_path(&mut self, path: &str) -> Result<()> {
        let normalized = Self::normalized_node_path(path);
        if normalized.is_empty() {
            self.tree_view_cursor = 0;
            return Ok(());
        }

        let previous_cursor = self.tree_view_cursor;
        let mut current = self.root.clone();
        for segment in normalized.split('/') {
            let next_and_index = {
                let mut node = current.borrow_mut();
                node.ensure_expanded()?;
                node.children.iter().enumerate().find_map(|(index, child)| {
                    let name = child.borrow().name();
                    (name == segment).then(|| (index, child.clone()))
                })
            };
            let Some((index, next)) = next_and_index else {
                self.compute_tree_view();
                self.tree_view_cursor = self.treeview.len().saturating_sub(1).min(previous_cursor);
                return Err(AppError::ChildNotFound(path.to_string()));
            };
            current.borrow_mut().view_loaded = (index + 50) as u32;
            current = next;
        }
        self.compute_tree_view();
        let Some((index, _)) = self
            .treeview
            .iter()
            .enumerate()
            .find(|(_, item)| Rc::ptr_eq(&item.node, &current))
        else {
            self.tree_view_cursor = self.treeview.len().saturating_sub(1).min(previous_cursor);
            return Err(AppError::ChildNotFound(path.to_string()));
        };
        self.tree_view_cursor = index;
        Ok(())
    }

    pub fn select_attribute_by_name(&mut self, attr_name: &str) -> Result<()> {
        let tree_item = self
            .treeview
            .get(self.tree_view_cursor)
            .ok_or_else(|| AppError::EditError("No selected tree item".to_string()))?;
        let mut node = tree_item.node.borrow_mut();
        let attributes = node.read_attributes()?;
        let Some(index) = attributes
            .rendered_rows
            .iter()
            .position(|row| row.key.as_deref() == Some(attr_name))
        else {
            return Err(AppError::ChildNotFound(attr_name.to_string()));
        };
        node.attributes_view_cursor.attribute_index = index;
        node.attributes_view_cursor.attribute_view_selection = AttributeViewSelection::Value;
        Ok(())
    }

    pub fn navigate_to_attribute_target(
        &mut self,
        path: &str,
        attr_name: Option<&str>,
    ) -> Result<()> {
        self.select_tree_node_by_path(path)?;
        if let Some(attr_name) = attr_name {
            self.focus = Focus::Attributes;
            self.select_attribute_by_name(attr_name)?;
        } else {
            self.focus = Focus::Tree(LastFocused::Attributes);
        }
        Ok(())
    }

    pub fn begin_preview_debounce(&mut self, path: String) -> u64 {
        self.preview_debounce_generation = self.preview_debounce_generation.wrapping_add(1);
        self.preview_debounce_until = Some(Instant::now() + Self::PREVIEW_DEBOUNCE_DELAY);
        self.preview_debounce_path = Some(path);
        self.preview_debounce_generation
    }

    pub fn clear_preview_debounce(&mut self) {
        self.preview_debounce_until = None;
        self.preview_debounce_path = None;
    }

    pub fn resolve_preview_debounce(&mut self, generation: u64) -> bool {
        if self.preview_debounce_generation != generation {
            return false;
        }
        let Some(until) = self.preview_debounce_until else {
            return false;
        };
        if Instant::now() < until {
            return false;
        }
        self.clear_preview_debounce();
        true
    }

    pub fn should_debounce_preview(&self, node: &Node) -> bool {
        if !matches!(self.mode, Mode::Normal) || !matches!(self.focus, Focus::Tree(_)) {
            return false;
        }
        let Some(until) = self.preview_debounce_until else {
            return false;
        };
        if Instant::now() >= until {
            return false;
        }
        self.preview_debounce_path.as_deref() == Some(node.path().as_str())
    }

    pub fn active_image_window_mut(&mut self) -> Option<&mut ImageWindowState> {
        let selected_path = self.selected_tree_path()?;
        let window = self.img_state.window.as_mut()?;
        (window.ds_path == selected_path).then_some(window)
    }

    fn remember_main_focus(&mut self, last_focused: LastFocused) {
        self.focus = Focus::Tree(last_focused);
    }

    pub fn focus_tree_from_current(&mut self) {
        let last_focused = match &self.focus {
            Focus::Tree(last_focused) => last_focused.clone(),
            Focus::Attributes => LastFocused::Attributes,
            Focus::Content => LastFocused::Content,
        };
        self.focus = Focus::Tree(last_focused);
    }

    pub fn help_next_tab(&mut self) -> bool {
        let next = self.help.selected_tab.step(1);
        if next == self.help.selected_tab {
            return false;
        }
        self.help.selected_tab = next;
        true
    }

    pub fn help_prev_tab(&mut self) -> bool {
        let next = self.help.selected_tab.step(-1);
        if next == self.help.selected_tab {
            return false;
        }
        self.help.selected_tab = next;
        true
    }

    pub fn help_next_section(&mut self) -> bool {
        match self.help.selected_tab {
            HelpTab::Keymap => {
                let next = self.help.keymap_section.step(1);
                if next == self.help.keymap_section {
                    return false;
                }
                self.help.keymap_section = next;
                true
            }
            HelpTab::Commands => {
                let next = self.help.command_section.step(1);
                if next == self.help.command_section {
                    return false;
                }
                self.help.command_section = next;
                true
            }
            HelpTab::Configuration => {
                let next = self.help.customization_section.step(1);
                if next == self.help.customization_section {
                    return false;
                }
                self.help.customization_section = next;
                true
            }
            _ => false,
        }
    }

    pub fn help_prev_section(&mut self) -> bool {
        match self.help.selected_tab {
            HelpTab::Keymap => {
                let next = self.help.keymap_section.step(-1);
                if next == self.help.keymap_section {
                    return false;
                }
                self.help.keymap_section = next;
                true
            }
            HelpTab::Commands => {
                let next = self.help.command_section.step(-1);
                if next == self.help.command_section {
                    return false;
                }
                self.help.command_section = next;
                true
            }
            HelpTab::Configuration => {
                let next = self.help.customization_section.step(-1);
                if next == self.help.customization_section {
                    return false;
                }
                self.help.customization_section = next;
                true
            }
            _ => false,
        }
    }

    pub fn help_first_section(&mut self) -> bool {
        match self.help.selected_tab {
            HelpTab::Keymap => {
                if self.help.keymap_section == HelpKeymapSection::Global {
                    false
                } else {
                    self.help.keymap_section = HelpKeymapSection::Global;
                    true
                }
            }
            HelpTab::Commands => {
                if self.help.command_section == HelpCommandSection::Navigation {
                    false
                } else {
                    self.help.command_section = HelpCommandSection::Navigation;
                    true
                }
            }
            HelpTab::Configuration => {
                if self.help.customization_section == HelpCustomizationSection::Configuration {
                    false
                } else {
                    self.help.customization_section = HelpCustomizationSection::Configuration;
                    true
                }
            }
            _ => false,
        }
    }

    pub fn help_last_section(&mut self) -> bool {
        match self.help.selected_tab {
            HelpTab::Keymap => {
                if self.help.keymap_section == HelpKeymapSection::MultiChart {
                    false
                } else {
                    self.help.keymap_section = HelpKeymapSection::MultiChart;
                    true
                }
            }
            HelpTab::Commands => {
                if self.help.command_section == HelpCommandSection::Input {
                    false
                } else {
                    self.help.command_section = HelpCommandSection::Input;
                    true
                }
            }
            HelpTab::Configuration => {
                if self.help.customization_section == HelpCustomizationSection::Scripting {
                    false
                } else {
                    self.help.customization_section = HelpCustomizationSection::Scripting;
                    true
                }
            }
            _ => false,
        }
    }

    pub fn focus_left(&mut self) {
        if !self.show_tree_view {
            return;
        }
        if self.stacked_tree_layout {
            match self.focus {
                Focus::Attributes => self.remember_main_focus(LastFocused::Attributes),
                Focus::Content => self.remember_main_focus(LastFocused::Content),
                Focus::Tree(_) => {}
            }
            return;
        }
        match self.focus {
            Focus::Attributes => self.remember_main_focus(LastFocused::Attributes),
            Focus::Content => self.remember_main_focus(LastFocused::Content),
            Focus::Tree(_) => {}
        }
    }

    pub fn focus_right(&mut self) {
        if !self.show_tree_view {
            return;
        }
        if self.stacked_tree_layout {
            match self.focus {
                Focus::Tree(LastFocused::Attributes) => self.focus = Focus::Attributes,
                Focus::Tree(LastFocused::Content) => self.focus = Focus::Content,
                Focus::Attributes | Focus::Content => {}
            }
            return;
        }
        match self.focus {
            Focus::Tree(LastFocused::Attributes) => self.focus = Focus::Attributes,
            Focus::Tree(LastFocused::Content) => self.focus = Focus::Content,
            Focus::Attributes | Focus::Content => {}
        }
    }

    pub fn focus_up(&mut self) {
        if !self.show_tree_view {
            return;
        }
        if self.stacked_tree_layout {
            match self.focus {
                Focus::Content => self.focus = Focus::Attributes,
                Focus::Attributes => self.remember_main_focus(LastFocused::Attributes),
                Focus::Tree(_) => {}
            }
            return;
        }
        match self.focus {
            Focus::Content => self.focus = Focus::Attributes,
            Focus::Tree(_) => self.focus = Focus::Attributes,
            Focus::Attributes => {}
        }
    }

    pub fn focus_down(&mut self) {
        if !self.show_tree_view {
            return;
        }
        if self.stacked_tree_layout {
            match self.focus {
                Focus::Tree(_) => self.focus = Focus::Attributes,
                Focus::Attributes => self.focus = Focus::Content,
                Focus::Content => {}
            }
            return;
        }
        match self.focus {
            Focus::Attributes => self.focus = Focus::Content,
            Focus::Tree(_) => self.focus = Focus::Content,
            Focus::Content => {}
        }
    }

    pub fn toggle_tree_view(&mut self) {
        self.show_tree_view = !self.show_tree_view;
        self.pending_chord = None;
        if self.show_tree_view {
            self.focus = Focus::Tree(LastFocused::Content);
        } else {
            self.focus = Focus::Content;
        }
    }

    pub fn swap_content_show_mode(&mut self, available: Vec<ContentShowMode>) {
        let available = self.filter_runtime_content_modes(available);
        let ordered = crate::configure::ordered_content_modes(&available);
        if ordered.is_empty() {
            return;
        }
        let current_index = ordered
            .iter()
            .position(|mode| *mode == self.content_mode)
            .unwrap_or(0);
        self.set_content_mode(ordered[(current_index + 1) % ordered.len()]);
    }

    pub fn set_content_mode(&mut self, mode: ContentShowMode) {
        if self.content_mode == ContentShowMode::Heatmap && mode != ContentShowMode::Heatmap {
            self.end_heatmap_drag();
        }
        self.content_mode = mode;
    }

    pub fn content_show_mode_eval(&self, available: Vec<ContentShowMode>) -> ContentShowMode {
        let available = self.filter_runtime_content_modes(available);
        if available.contains(&self.content_mode) {
            self.content_mode
        } else {
            crate::configure::ordered_content_modes(&available)
                .first()
                .copied()
                .unwrap_or(ContentShowMode::Preview)
        }
    }

    pub fn active_content_mode(&self) -> ContentShowMode {
        let available = self
            .treeview
            .get(self.tree_view_cursor)
            .and_then(|item| {
                item.node
                    .try_borrow()
                    .ok()
                    .map(|node| node.content_show_modes())
            })
            .unwrap_or_else(|| vec![self.content_mode]);
        self.content_show_mode_eval(available)
    }

    pub fn filter_runtime_content_modes(
        &self,
        available: Vec<ContentShowMode>,
    ) -> Vec<ContentShowMode> {
        if !self.compatibility_mode && self.image_protocol_enabled {
            available
        } else {
            available
                .into_iter()
                .filter(|mode| *mode != ContentShowMode::Heatmap)
                .collect()
        }
    }

    pub fn change_row(&mut self, delta: isize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Matrix | ContentShowMode::Heatmap => {
                let current_node = &self.treeview[self.tree_view_cursor];
                let mut current_node = current_node.node.borrow_mut();
                if let Node::Dataset(_, dsattr) = &current_node.node {
                    let shape = dsattr.shape.clone();
                    if shape.len() == 2 {
                        let temp = current_node.selected_row;
                        current_node.selected_row = current_node.selected_col;
                        current_node.selected_col = temp;
                        return Ok(EventResult::Redraw);
                    }
                    let new_selected_row = ((current_node.selected_row as isize + delta)
                        % shape.len() as isize) as usize
                        % shape.len();
                    if new_selected_row != current_node.selected_col {
                        current_node.selected_row = new_selected_row;
                        return Ok(EventResult::Redraw);
                    }
                    current_node.selected_row = ((current_node.selected_row as isize + delta + 1)
                        % shape.len() as isize)
                        as usize
                        % shape.len();

                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Continue)
                }
            }
            _ => Ok(EventResult::Continue),
        }
    }

    pub fn capture_multichart_item(&self) -> Result<Option<(ChartSource, Vec<Point>)>> {
        let current_node = &self.treeview[self.tree_view_cursor];
        let mut node = current_node.node.borrow_mut();
        match &node.node {
            Node::Group(_, meta) => {
                let Some(expression) = meta.preview_expr.as_deref() else {
                    return Ok(None);
                };
                let item = self
                    .multi_chart
                    .capture_expression_chart_item(expression, self.file.as_ref())
                    .map_err(AppError::InvalidCommand)?;
                Ok(Some(item))
            }
            Node::Dataset(_, dsattr) if dsattr.is_compound_container() => Ok(None),
            Node::Dataset(ds, dsattr) => {
                let ds = ds.clone();
                let meta = dsattr.clone();
                let shape = dsattr.shape.clone();
                let Some(selection) =
                    preview_selection_for_node(&mut node, &shape, self.segment_state.idx)
                else {
                    return Ok(None);
                };
                let data = if meta.is_compound_leaf() {
                    plot_projected(&ds, &meta, &selection)?.data
                } else {
                    ds.plot(&selection)?.data
                };
                let source = ChartSource::DatasetSelection(DatasetChartSource {
                    dataset_path: ds.name(),
                    display_path: meta.virtual_path().unwrap_or(&ds.name()).to_string(),
                    selection,
                    shape,
                    kind: if meta.is_compound_leaf() {
                        DatasetChartKind::CompoundLeaf
                    } else {
                        DatasetChartKind::Dataset
                    },
                });
                Ok(Some((source, data)))
            }
            _ => Ok(None),
        }
    }

    pub fn change_selected_dimension(&mut self, delta: isize) -> Result<EventResult> {
        let active_mode = self.active_content_mode();
        let current_node = &self.treeview[self.tree_view_cursor];
        let mut node = current_node.node.borrow_mut();
        let shape_len = match &node.node {
            Node::Dataset(_, dsattr) => dsattr.shape.len(),
            _ => return Ok(EventResult::Continue),
        };
        node.sync_selection_rank(shape_len);
        let current_shape_len = shape_len as isize;
        let next = node.selected_dim as isize + delta;
        let new_selected_dim = if next < 0 {
            (current_shape_len - 1) as usize
        } else if next >= current_shape_len {
            0_usize
        } else {
            next as usize
        };
        match active_mode {
            ContentShowMode::Preview => {
                if new_selected_dim != node.selected_x {
                    node.selected_dim = new_selected_dim;
                } else {
                    let next_next = new_selected_dim as isize + delta;
                    let next_next = if next_next < 0 {
                        (current_shape_len - 1) as usize
                    } else if next_next >= current_shape_len {
                        0_usize
                    } else {
                        next_next as usize
                    };
                    node.selected_dim = next_next.clamp(0, current_shape_len as usize);
                }
                Ok(EventResult::Redraw)
            }
            ContentShowMode::Matrix | ContentShowMode::Heatmap => {
                if new_selected_dim != node.selected_col && new_selected_dim != node.selected_row {
                    node.selected_dim = new_selected_dim;
                } else {
                    let next_next = new_selected_dim as isize + delta;
                    let next_next = if next_next < 0 {
                        (current_shape_len - 1) as usize
                    } else if next_next >= current_shape_len {
                        0_usize
                    } else {
                        next_next as usize
                    };
                    if next_next != node.selected_col && next_next != node.selected_row {
                        node.selected_dim = next_next.clamp(0, current_shape_len as usize);
                    } else {
                        let next_next_next = next_next as isize + delta;
                        let next_next_next = if next_next_next < 0 {
                            (current_shape_len - 1) as usize
                        } else if next_next_next >= current_shape_len {
                            0_usize
                        } else {
                            next_next_next as usize
                        };
                        node.selected_dim = next_next_next.clamp(0, current_shape_len as usize);
                    }
                }
                Ok(EventResult::Redraw)
            }
        }
    }

    pub fn change_selected_index(&mut self, delta: isize) -> Result<EventResult> {
        let current_node = &self.treeview[self.tree_view_cursor];
        let mut node = current_node.node.borrow_mut();
        let shape = match &node.node {
            Node::Dataset(_, dsattr) => dsattr.shape.clone(),
            _ => return Ok(EventResult::Continue),
        };
        node.sync_selection_rank(shape.len());
        let x_shape = shape[node.selected_dim];
        let current_selected_dim = node.selected_indexes[node.selected_dim] as isize;
        let new_current_x_index =
            (current_selected_dim + delta).clamp(0, x_shape as isize - 1) as usize;
        let selected_x = node.selected_dim;
        node.selected_indexes[selected_x] = new_current_x_index;

        Ok(EventResult::Redraw)
    }

    pub fn change_col(&mut self, delta: isize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Matrix | ContentShowMode::Heatmap => {
                let current_node = &self.treeview[self.tree_view_cursor];
                let mut current_node = current_node.node.borrow_mut();
                if let Node::Dataset(_, dsattr) = &current_node.node {
                    let shape = dsattr.shape.clone();
                    if shape.len() == 2 {
                        // Just swap row and col.
                        let temp = current_node.selected_row;
                        current_node.selected_row = current_node.selected_col;
                        current_node.selected_col = temp;
                        return Ok(EventResult::Redraw);
                    }
                    let new_selected_col = ((current_node.selected_col as isize + delta)
                        % shape.len() as isize) as usize
                        % shape.len();
                    if new_selected_col != current_node.selected_row {
                        current_node.selected_col = new_selected_col;
                        return Ok(EventResult::Redraw);
                    }
                    current_node.selected_col = ((current_node.selected_col as isize + delta + 1)
                        % shape.len() as isize)
                        as usize
                        % shape.len();

                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Continue)
                }
            }
            _ => Ok(EventResult::Continue),
        }
    }

    pub fn change_x(&mut self, delta: isize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => {
                let current_node = &self.treeview[self.tree_view_cursor];
                let mut current_node = current_node.node.borrow_mut();
                if let Node::Dataset(_, dsattr) = &current_node.node {
                    let shape = dsattr.shape.clone();
                    current_node.selected_x = ((current_node.selected_x as isize + delta)
                        % shape.len() as isize)
                        as usize
                        % shape.len();
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Continue)
                }
            }
            _ => Ok(EventResult::Continue),
        }
    }

    pub fn up(&mut self, dec: usize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    if self.img_state.idx_to_load >= (dec as i32)
                        && self.img_state.idx_to_load - dec as i32 >= 0
                    {
                        self.img_state.idx_to_load -= dec as i32;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                SegmentType::Chart => {
                    let Some(max_index) = self.segment_state.max_index() else {
                        self.segment_state.idx = 0;
                        return Ok(EventResult::Continue);
                    };
                    self.segment_state.idx = self
                        .segment_state
                        .idx
                        .saturating_sub(dec as i32)
                        .clamp(0, max_index);
                    Ok(EventResult::Redraw)
                }
                SegmentType::NoSegment => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) * dec.max(1)) as isize;
                        window.shift_by(-step);
                        return Ok(EventResult::Redraw);
                    }
                    self.img_state.idx_to_load = self.segment_state.idx;
                    let current_node = &self.treeview[self.tree_view_cursor];
                    let mut node = current_node.node.borrow_mut();
                    let new_offset = node.line_offset as isize - dec as isize;
                    let new_offset = if new_offset < 0 {
                        0
                    } else {
                        new_offset as usize
                    };
                    node.line_offset = new_offset;

                    Ok(EventResult::Redraw)
                }
            },
            ContentShowMode::Matrix => {
                let current_node = &self.treeview[self.tree_view_cursor];
                let node = &current_node.node.borrow_mut();
                let current_node = &node.node;
                if self.matrix_view_state.row_offset == 0 {
                    return Ok(EventResult::Redraw);
                }
                if let Node::Dataset(_, dsattr) = current_node {
                    let row_selected_shape = dsattr.shape[node.selected_row];
                    self.matrix_view_state.row_offset =
                        (self.matrix_view_state.row_offset.saturating_sub(dec)).min(
                            row_selected_shape
                                .saturating_sub(self.matrix_view_state.rows_currently_available),
                        );
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Redraw)
                }
            }
            ContentShowMode::Heatmap => {
                if dec > 1 {
                    if let Some(window) = self.heatmap_render.segment.as_mut() {
                        let next_page = window
                            .page
                            .saturating_sub(1)
                            .clamp(0, window.page_count.saturating_sub(1));
                        if next_page != window.page {
                            window.page = next_page;
                            self.heatmap_render.current_key = None;
                            return Ok(EventResult::Redraw);
                        }
                    }
                }
                self.heatmap_render.selected_setting = self
                    .heatmap_render
                    .selected_setting
                    .saturating_sub(1)
                    .min(HEATMAP_SETTING_FIELDS.len().saturating_sub(1));
                Ok(EventResult::Redraw)
            }
        }
    }

    pub fn down(&mut self, inc: usize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    let Some(max_index) = self.segment_state.max_index() else {
                        self.img_state.idx_to_load = 0;
                        return Ok(EventResult::Continue);
                    };
                    let proposed = self.img_state.idx_to_load.saturating_add(inc as i32);
                    if proposed <= max_index {
                        self.img_state.idx_to_load = proposed;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                SegmentType::Chart => {
                    let Some(max_index) = self.segment_state.max_index() else {
                        self.segment_state.idx = 0;
                        return Ok(EventResult::Continue);
                    };
                    self.segment_state.idx = self
                        .segment_state
                        .idx
                        .saturating_add(inc as i32)
                        .clamp(0, max_index);
                    Ok(EventResult::Redraw)
                }
                SegmentType::NoSegment => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) * inc.max(1)) as isize;
                        window.shift_by(step);
                        return Ok(EventResult::Redraw);
                    }
                    self.img_state.idx_to_load = self.segment_state.idx;

                    self.img_state.idx_to_load = self.segment_state.idx;
                    let current_node = &self.treeview[self.tree_view_cursor];
                    let mut node = current_node.node.borrow_mut();
                    let new_offset = node.line_offset + inc;
                    node.line_offset = new_offset;
                    Ok(EventResult::Redraw)
                }
            },
            ContentShowMode::Matrix => {
                let node = &self.treeview[self.tree_view_cursor].node.borrow_mut();
                let current_node = &node.node;
                if let Node::Dataset(_, dsattr) = current_node {
                    let row_selected_shape = dsattr.shape[node.selected_row];
                    self.matrix_view_state.row_offset = (self.matrix_view_state.row_offset + inc)
                        .min(
                            row_selected_shape
                                .saturating_sub(self.matrix_view_state.rows_currently_available),
                        );
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Redraw)
                }
            }
            ContentShowMode::Heatmap => {
                if inc > 1 {
                    if let Some(window) = self.heatmap_render.segment.as_mut() {
                        let next_page = window
                            .page
                            .saturating_add(1)
                            .clamp(0, window.page_count.saturating_sub(1));
                        if next_page != window.page {
                            window.page = next_page;
                            self.heatmap_render.current_key = None;
                            return Ok(EventResult::Redraw);
                        }
                    }
                }
                self.heatmap_render.selected_setting = self
                    .heatmap_render
                    .selected_setting
                    .saturating_add(1)
                    .min(HEATMAP_SETTING_FIELDS.len().saturating_sub(1));
                Ok(EventResult::Redraw)
            }
        }
    }

    pub fn set(&mut self, idx: usize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    if let Some(window) = self.active_image_window_mut() {
                        window.center_on(idx);
                        return Ok(EventResult::Redraw);
                    }
                    if idx < self.segment_state.segment_count.max(0) as usize {
                        self.img_state.idx_to_load = idx as i32;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                SegmentType::Chart => {
                    let Some(max_index) = self.segment_state.max_index() else {
                        self.segment_state.idx = 0;
                        return Ok(EventResult::Continue);
                    };
                    if idx > 0 {
                        self.segment_state.idx = ((idx - 1) as i32).clamp(0, max_index);
                        Ok(EventResult::Redraw)
                    } else {
                        self.segment_state.idx = 0;
                        Ok(EventResult::Redraw)
                    }
                }
                SegmentType::NoSegment => {
                    if let Some(window) = self.active_image_window_mut() {
                        window.center_on(idx);
                        return Ok(EventResult::Redraw);
                    }
                    self.img_state.idx_to_load = idx as i32;
                    Ok(EventResult::Redraw)
                }
            },
            ContentShowMode::Matrix => {
                let node = &self.treeview[self.tree_view_cursor].node.borrow_mut();
                let current_node = &node.node;
                if let Node::Dataset(_, dsattr) = current_node {
                    let row_selected_shape = dsattr.shape[node.selected_row];
                    self.matrix_view_state.row_offset = idx.min(
                        row_selected_shape
                            .saturating_sub(self.matrix_view_state.rows_currently_available),
                    );
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Redraw)
                }
            }
            ContentShowMode::Heatmap => {
                self.heatmap_render.selected_setting =
                    idx.min(HEATMAP_SETTING_FIELDS.len().saturating_sub(1));
                Ok(EventResult::Redraw)
            }
        }
    }

    pub fn reexecute_command(&mut self) -> Result<EventResult> {
        let Some(last_command) = self.command_state.last_command.clone() else {
            return Ok(EventResult::Toast(
                AppToast::Info("No previous command to repeat".to_string()),
                false,
            ));
        };
        execute_command(self, &last_command)
    }

    pub fn heatmap_range_modes(&self) -> Vec<HeatmapRangeMode> {
        let mut modes = HeatmapRangeMode::default_modes();
        for mode in configure::current_heatmap_range_modes()
            .into_iter()
            .chain(self.heatmap_render.session_range_modes.iter().cloned())
        {
            if !modes.contains(&mode) {
                modes.push(mode);
            }
        }
        modes
    }

    pub fn sync_heatmap_configuration(&mut self) {
        let available = self.heatmap_range_modes();
        let mut configured = configure::current_heatmap_default_settings();
        if !available.contains(&configured.range) {
            configured.range = available.first().cloned().unwrap_or(HeatmapRangeMode::Auto);
        }
        self.heatmap_render.settings = configured;
        self.heatmap_render.current_key = None;
    }

    pub fn add_session_heatmap_range_mode(&mut self, mode: HeatmapRangeMode) -> Result<()> {
        let label = mode.label();
        if self
            .heatmap_range_modes()
            .iter()
            .any(|existing| existing.label().eq_ignore_ascii_case(&label))
        {
            return Err(AppError::InvalidCommand(format!(
                "Heatmap range mode '{label}' already exists"
            )));
        }
        self.heatmap_render.session_range_modes.push(mode.clone());
        self.heatmap_render.settings.range = mode;
        self.heatmap_render.current_key = None;
        Ok(())
    }

    fn adjust_heatmap_range_mode(&mut self, delta: isize) {
        let available = self.heatmap_range_modes();
        if available.is_empty() {
            return;
        }
        let current_index = available
            .iter()
            .position(|mode| *mode == self.heatmap_render.settings.range)
            .unwrap_or_else(|| {
                available
                    .iter()
                    .position(|mode| *mode == configure::current_heatmap_default_range())
                    .unwrap_or(0)
            });
        let next_index =
            (current_index as isize + delta.signum()).rem_euclid(available.len() as isize) as usize;
        self.heatmap_render.settings.range = available[next_index].clone();
        self.heatmap_render.current_key = None;
    }

    pub fn right(&mut self, inc: isize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) as isize) * inc.max(1);
                        window.shift_by(step);
                        Ok(EventResult::Redraw)
                    } else {
                        self.down(1)
                    }
                }
                SegmentType::Chart => Ok(EventResult::Continue),
                SegmentType::NoSegment => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) as isize) * inc.max(1);
                        window.shift_by(step);
                        return Ok(EventResult::Redraw);
                    }
                    let current_node = &self.treeview[self.tree_view_cursor];
                    let mut node = current_node.node.borrow_mut();
                    let new_col_offset = node.col_offset.saturating_add(inc).max(0);
                    node.col_offset = new_col_offset;
                    Ok(EventResult::Redraw)
                }
            },
            ContentShowMode::Matrix => {
                let node = &self.treeview[self.tree_view_cursor].node.borrow_mut();
                let current_node = &node.node;
                if let Node::Dataset(_, dsattr) = current_node {
                    let col_selected_shape = dsattr.shape[node.selected_col];
                    self.matrix_view_state.col_offset =
                        (self.matrix_view_state.col_offset + inc as usize).min(
                            col_selected_shape
                                .saturating_sub(self.matrix_view_state.cols_currently_available),
                        );
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Redraw)
                }
            }
            ContentShowMode::Heatmap => {
                let field = HEATMAP_SETTING_FIELDS
                    .get(self.heatmap_render.selected_setting)
                    .copied()
                    .unwrap_or(HeatmapSettingField::Colormap);
                if matches!(field, HeatmapSettingField::Range) {
                    self.adjust_heatmap_range_mode(inc);
                } else {
                    self.heatmap_render.settings.adjust(field, inc);
                    self.heatmap_render.current_key = None;
                }
                Ok(EventResult::Redraw)
            }
        }
    }

    pub fn left(&mut self, inc: isize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) as isize) * inc.max(1);
                        window.shift_by(-step);
                        Ok(EventResult::Redraw)
                    } else {
                        self.up(1)
                    }
                }
                SegmentType::Chart => Ok(EventResult::Continue),
                SegmentType::NoSegment => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) as isize) * inc.max(1);
                        window.shift_by(-step);
                        return Ok(EventResult::Redraw);
                    }
                    let current_node = &self.treeview[self.tree_view_cursor];
                    let mut node = current_node.node.borrow_mut();
                    let new_col_offset = node.col_offset.saturating_sub(inc).max(0);
                    node.col_offset = new_col_offset;
                    Ok(EventResult::Redraw)
                }
            },
            ContentShowMode::Matrix => {
                let current_node = &self.treeview[self.tree_view_cursor];
                let node = &current_node.node.borrow_mut();
                let current_node = &node.node;
                if self.matrix_view_state.col_offset == 0 {
                    return Ok(EventResult::Redraw);
                }
                if let Node::Dataset(_, dsattr) = current_node {
                    let col_selected_shape = dsattr.shape[node.selected_col];
                    self.matrix_view_state.col_offset = (self
                        .matrix_view_state
                        .col_offset
                        .saturating_sub(inc as usize))
                    .min(
                        col_selected_shape
                            .saturating_sub(self.matrix_view_state.cols_currently_available),
                    );
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Redraw)
                }
            }
            ContentShowMode::Heatmap => {
                let field = HEATMAP_SETTING_FIELDS
                    .get(self.heatmap_render.selected_setting)
                    .copied()
                    .unwrap_or(HeatmapSettingField::Colormap);
                if matches!(field, HeatmapSettingField::Range) {
                    self.adjust_heatmap_range_mode(-inc);
                } else {
                    self.heatmap_render.settings.adjust(field, -inc);
                    self.heatmap_render.current_key = None;
                }
                Ok(EventResult::Redraw)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        heatmap_anchor_fraction, HelpCommandSection, HelpCustomizationSection, HelpKeymapSection,
        HelpTab, HelpViewState,
    };

    #[test]
    fn heatmap_anchor_fraction_uses_display_position_when_not_inverted() {
        assert!((heatmap_anchor_fraction(0, 10, false) - 0.05).abs() < f64::EPSILON);
        assert!((heatmap_anchor_fraction(9, 10, false) - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn heatmap_anchor_fraction_flips_for_inverted_axes() {
        assert!((heatmap_anchor_fraction(0, 10, true) - 0.95).abs() < f64::EPSILON);
        assert!((heatmap_anchor_fraction(9, 10, true) - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn help_tab_navigation_wraps() {
        assert_eq!(HelpTab::Keymap.step(-1), HelpTab::Configuration);
        assert_eq!(HelpTab::Configuration.step(1), HelpTab::Keymap);
    }

    #[test]
    fn help_sidebar_navigation_wraps() {
        assert_eq!(
            HelpKeymapSection::Global.step(-1),
            HelpKeymapSection::MultiChart
        );
        assert_eq!(
            HelpCommandSection::Input.step(1),
            HelpCommandSection::Navigation
        );
        assert_eq!(
            HelpCustomizationSection::Configuration.step(-1),
            HelpCustomizationSection::Scripting
        );
    }

    #[test]
    fn help_view_defaults_to_keymap_navigation() {
        let help = HelpViewState::default();
        assert_eq!(help.selected_tab, HelpTab::Keymap);
        assert_eq!(help.keymap_section, HelpKeymapSection::Global);
        assert_eq!(help.command_section, HelpCommandSection::Navigation);
        assert_eq!(
            help.customization_section,
            HelpCustomizationSection::Configuration
        );
    }
}
