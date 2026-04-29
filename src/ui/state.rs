use std::{
    cell::RefCell,
    io::BufReader,
    rc::Rc,
    sync::{mpsc::Sender, Arc, RwLock},
};

use arboard::Clipboard;
use hdf5_metno::{ByteReader, Dataset, File, Hyperslab, Selection, SliceOrIndex};
use image::ImageFormat;
use ratatui_image::thread::ThreadProtocol;

use crate::{
    data::DatasetPlotingData,
    data::PreviewSelection,
    error::AppError,
    h5f::{H5FNode, HasPath, ImageType, Node},
    search::Searcher,
    ui::mchart::MultiChartState,
};

use super::{
    command::{Command, CommandState},
    input::EventResult,
    tree_view::TreeItem,
};

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

pub struct ChartPreviewLoadRequest {
    pub source: ChartPreviewSource,
    pub segment_state: SegmentState,
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

pub struct ImgState {
    pub protocol: Option<ThreadProtocol>,
    pub tx_load_imgfs: Sender<(BufReader<ByteReader>, ImageFormat)>,
    pub tx_load_imgfsvlen: Sender<(Dataset, i32, ImageFormat)>,
    pub tx_load_img: Sender<(Dataset, i32, ImageType)>,
    pub ds: Option<String>,
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

#[derive(Debug, Clone)]
pub enum AttributeViewSelection {
    Name,
    Value,
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

pub struct AppState<'a> {
    pub readonly: bool,
    pub root: Rc<RefCell<H5FNode>>,
    pub treeview: Vec<TreeItem<'a>>,
    pub file: File,
    pub editing: bool,
    pub edit_pause: Arc<RwLock<()>>,
    pub tree_view_cursor: usize,
    pub clipboard: Clipboard,
    pub copying: bool,
    pub toast: AppToast,
    pub focus: Focus,
    pub multi_chart: MultiChartState,
    pub mode: Mode,
    pub searcher: Option<Searcher>,
    pub pending_chord: Option<PendingChord>,
    pub show_tree_view: bool,
    pub stacked_tree_layout: bool,
    pub content_mode: ContentShowMode,
    pub img_state: ImgState,
    pub matrix_view_state: MatrixViewState,
    pub chart_preview_state: ChartPreviwState,
    pub segment_state: SegmentState,
    pub command_state: CommandState,
}

type Result<T> = std::result::Result<T, AppError>;
impl AppState<'_> {
    fn remember_main_focus(&mut self, last_focused: LastFocused) {
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
                        .min(row_selected_shape - self.matrix_view_state.rows_currently_available);
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

    pub fn execute_command(&mut self, command: &Command) -> Result<EventResult> {
        match command {
            super::command::Command::Increment(increment) => self.down(*increment),
            super::command::Command::Decrement(decrement) => self.up(*decrement),
            super::command::Command::Seek(seek) => self.set(*seek),
            super::command::Command::Noop => Ok(EventResult::Redraw),
        }
    }

    pub fn reexecute_command(&mut self) -> Result<EventResult> {
        let last_command = &self.command_state.last_command.clone();
        self.execute_command(last_command)
    }

    pub fn right(&mut self, inc: isize) -> Result<EventResult> {
        match self.content_mode {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => self.down(1),
                SegmentType::Chart => Ok(EventResult::Continue),
                SegmentType::NoSegment => {
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
                    self.matrix_view_state.col_offset = (self.matrix_view_state.col_offset
                        + inc as usize)
                        .min(col_selected_shape - self.matrix_view_state.cols_currently_available);
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
                SegmentType::Image => self.up(1),
                SegmentType::Chart => Ok(EventResult::Continue),
                SegmentType::NoSegment => {
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
