use std::{
    cell::RefCell,
    fs,
    io::BufReader,
    rc::Rc,
    sync::{mpsc::Sender, Arc, RwLock},
    time::{Duration, Instant, SystemTime},
};

use arboard::Clipboard;
use hdf5_metno::{ByteReader, Dataset, File, Hyperslab, Selection, SliceOrIndex};
use image::ImageFormat;
use ratatui::layout::Rect;
use ratatui_image::thread::ThreadProtocol;

use crate::{
    data::DatasetPlotingData,
    data::PreviewSelection,
    error::{AppError, FixedStringOverflow},
    h5f::{H5FNode, HasPath, ImageType, Node},
    search::Searcher,
    ui::mchart::MultiChartState,
};

use super::{
    command::{execute_command, CommandState},
    input::EventResult,
    tree_view::TreeItem,
};

/// Convert internal HDF5 paths (always using /) to platform-appropriate display paths.
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
    FixedStringOverflowDialog,
    FixedStringResizeDialog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingChord {
    CtrlW,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum ContentShowMode {
    Preview,
    Matrix,
}

#[derive(Debug, Clone, Copy)]
pub struct TreeHitbox {
    pub outer: Rect,
    pub inner: Rect,
    pub row_offset: usize,
    pub visible_rows: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct AttributesHitbox {
    pub outer: Rect,
    pub inner: Rect,
    pub name_area: Rect,
    pub value_area: Rect,
    pub row_offset: usize,
    pub visible_rows: usize,
    pub total_rows: usize,
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

#[derive(Debug, Clone, Default)]
pub struct UiLayoutState {
    pub tree: Option<TreeHitbox>,
    pub attributes: Option<AttributesHitbox>,
    pub content: Option<Rect>,
    pub content_tabs: Vec<ContentTabHitbox>,
    pub matrix_rows: Vec<MatrixRowHitbox>,
    pub matrix_cells: Vec<MatrixCellHitbox>,
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

pub struct ChartPreviwState {
    pub ds_loaded: Option<String>,
    pub protocol: Option<ThreadProtocol>,
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
    pub clipboard: Clipboard,
    pub copying: bool,
    pub toast: AppToast,
    pub file_watch: FileWatchState,
    pub focus: Focus,
    pub multi_chart: MultiChartState,
    pub mode: Mode,
    pub searcher: Option<Searcher>,
    pub pending_chord: Option<PendingChord>,
    pub show_tree_view: bool,
    pub stacked_tree_layout: bool,
    pub image_cell_size: (u16, u16),
    pub preview_debounce_generation: u64,
    pub preview_debounce_until: Option<Instant>,
    pub preview_debounce_path: Option<String>,
    pub content_mode: ContentShowMode,
    pub img_state: ImgState,
    pub matrix_view_state: MatrixViewState,
    pub chart_preview_state: ChartPreviwState,
    pub segment_state: SegmentState,
    pub command_state: CommandState,
    pub fixed_string_overflow_dialog: Option<FixedStringOverflowDialogState>,
    pub ui_layout: UiLayoutState,
}

type Result<T> = std::result::Result<T, AppError>;
impl AppState<'_> {
    const PREVIEW_DEBOUNCE_DELAY: Duration = Duration::from_millis(90);

    fn normalized_node_path(path: &str) -> &str {
        path.trim_start_matches('/')
    }

    fn current_file_modified(&self) -> Option<SystemTime> {
        fs::metadata(&self.file_watch.path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
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
            .rendered_attributes
            .iter()
            .position(|(name, _, _)| {
                name.to_string()
                    .trim_end_matches('=')
                    .trim_end_matches('─')
                    .trim_end()
                    == attr_name
            })
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
        if available.is_empty() {
            return;
        }
        match self.content_mode {
            ContentShowMode::Preview if available.contains(&ContentShowMode::Matrix) => {
                self.content_mode = ContentShowMode::Matrix;
            }
            _ => {
                self.content_mode = ContentShowMode::Preview;
            }
        }
    }

    pub fn content_show_mode_eval(&self, available: Vec<ContentShowMode>) -> ContentShowMode {
        if available.contains(&self.content_mode) {
            self.content_mode
        } else {
            available[0]
        }
    }

    pub fn change_row(&mut self, delta: isize) -> Result<EventResult> {
        match self.content_mode {
            ContentShowMode::Matrix => {
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

    pub fn get_1d_selection(&self) -> Option<(Dataset, crate::h5f::DatasetMeta, Selection)> {
        let current_node = &self.treeview[self.tree_view_cursor];
        let mut node = current_node.node.borrow_mut();
        let (ds, meta, shape) = match &node.node {
            Node::Dataset(ds, dsattr) => (ds.clone(), dsattr.clone(), dsattr.shape.clone()),
            _ => return None,
        };
        node.sync_selection_rank(shape.len());
        let selected_dim = node.selected_x;
        let mut slice: Vec<SliceOrIndex> = Vec::new();
        for (dim, _) in shape.iter().enumerate() {
            if dim == selected_dim {
                slice.push(SliceOrIndex::Unlimited {
                    start: 0,
                    step: 1,
                    block: 1,
                });
            } else {
                slice.push(SliceOrIndex::Index(node.selected_indexes[dim]));
            }
        }
        let hyperslap = Hyperslab::from(slice);
        Some((ds, meta, Selection::Hyperslab(hyperslap)))
    }

    pub fn change_selected_dimension(&mut self, delta: isize) -> Result<EventResult> {
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
        match self.content_mode {
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
            ContentShowMode::Matrix => {
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
        match self.content_mode {
            ContentShowMode::Matrix => {
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
        match self.content_mode {
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
        match self.content_mode {
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
                    self.segment_state.idx = self
                        .segment_state
                        .idx
                        .saturating_sub(dec as i32)
                        .clamp(0, self.segment_state.segment_count - 1);
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
        }
    }

    pub fn down(&mut self, inc: usize) -> Result<EventResult> {
        match self.content_mode {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    if self.img_state.idx_to_load <= self.segment_state.segment_count - inc as i32
                        && self.img_state.idx_to_load + inc as i32
                            <= self.segment_state.segment_count - 1
                    {
                        self.img_state.idx_to_load += inc as i32;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                SegmentType::Chart => {
                    self.segment_state.idx = self
                        .segment_state
                        .idx
                        .saturating_add(inc as i32)
                        .clamp(0, self.segment_state.segment_count - 1);
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
        }
    }

    pub fn set(&mut self, idx: usize) -> Result<EventResult> {
        match self.content_mode {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    if let Some(window) = self.active_image_window_mut() {
                        window.center_on(idx);
                        return Ok(EventResult::Redraw);
                    }
                    if idx < self.segment_state.segment_count as usize {
                        self.img_state.idx_to_load = idx as i32;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                SegmentType::Chart => {
                    if idx > 0 {
                        self.segment_state.idx =
                            ((idx - 1) as i32).clamp(0, self.segment_state.segment_count - 1);
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

    pub fn right(&mut self, inc: isize) -> Result<EventResult> {
        match self.content_mode {
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
        }
    }

    pub fn left(&mut self, inc: isize) -> Result<EventResult> {
        match self.content_mode {
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
        }
    }
}
