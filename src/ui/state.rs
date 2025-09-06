use std::{cell::RefCell, io::BufReader, rc::Rc, sync::mpsc::Sender};

use cli_clipboard::ClipboardContext;
use hdf5_metno::{ByteReader, Dataset, Hyperslab, Selection, SliceOrIndex};
use image::ImageFormat;
use ratatui_image::{picker::Picker, thread::ThreadProtocol};

use crate::{
    error::AppError,
    h5f::{H5FNode, ImageType, Node},
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

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum ContentShowMode {
    Preview,
    Matrix,
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
    pub picker: Picker,
}

impl ImgState {
    pub fn is_from_ds(&self, node: &Node) -> bool {
        if self.ds.is_none() {
            return false;
        }
        match node {
            Node::Dataset(ds, _) => {
                let name = &ds.name();
                let ds_name_str = match &self.ds {
                    Some(ds_name) => ds_name.as_str(),
                    None => {
                        return false;
                    }
                };
                if *name == ds_name_str && self.idx_to_load == self.idx_loaded {
                    return true;
                }
                false
            }
            _ => false,
        }
    }
}

pub enum AttributeViewSelection {
    Name,
    Value,
}

pub struct AttributeCursor {
    pub attribute_index: usize,
    pub attribute_view_selection: AttributeViewSelection,
}

pub struct MatrixViewState {
    pub col_offset: usize,
    pub row_offset: usize,
    pub rows_currently_available: usize,
    pub cols_currently_available: usize,
}

pub enum SegmentType {
    Image,
    Chart,
    NoSegment,
}

pub struct SegmentState {
    pub idx: i32,
    pub segumented: SegmentType,
    pub segment_count: i32,
}

pub struct AppState<'a> {
    pub root: Rc<RefCell<H5FNode>>,
    pub treeview: Vec<TreeItem<'a>>,
    pub tree_view_cursor: usize,
    pub clipboard: ClipboardContext,
    pub copying: bool,
    pub attributes_view_cursor: AttributeCursor,
    pub focus: Focus,
    pub multi_chart: MultiChartState,
    pub mode: Mode,
    pub indexed: bool,
    pub searcher: Rc<RefCell<Searcher>>,
    pub show_tree_view: bool,
    pub content_mode: ContentShowMode,
    pub img_state: ImgState,
    pub matrix_view_state: MatrixViewState,
    pub segment_state: SegmentState,
    pub command_state: CommandState,
}

type Result<T> = std::result::Result<T, AppError>;
impl AppState<'_> {
    pub fn index(&mut self) -> Result<()> {
        let mut root = self.root.borrow_mut();
        root.index(true)?;
        self.indexed = true;
        Ok(())
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

    pub fn read_1d(&self) -> Option<Vec<f64>> {
        let (ds, selection) = self.get_1d_selection()?;
        let data = ds.read_slice_1d::<f64, _>(selection).ok()?.to_vec();
        Some(data)
    }

    pub fn get_1d_selection(&self) -> Option<(Dataset, Selection)> {
        let current_node = &self.treeview[self.tree_view_cursor];
        let node = current_node.node.borrow();
        let Node::Dataset(ds, dsattr) = &node.node else {
            return None;
        };
        let selected_dim = node.selected_x;
        let mut slice: Vec<SliceOrIndex> = Vec::new();
        for (dim, _) in dsattr.shape.iter().enumerate() {
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
        Some((ds.clone(), Selection::Hyperslab(hyperslap)))
    }

    pub fn change_selected_dimension(&mut self, delta: isize) -> Result<EventResult> {
        let current_node = &self.treeview[self.tree_view_cursor];
        let mut node = current_node.node.borrow_mut();
        let Node::Dataset(_, dsattr) = &node.node else {
            return Ok(EventResult::Continue);
        };
        let current_shape_len = dsattr.shape.len() as isize;
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
        let Node::Dataset(_, dsattr) = &node.node else {
            return Ok(EventResult::Continue);
        };
        let x_shape = dsattr.shape[node.selected_dim];
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

    pub fn dec(&mut self, dec: usize) -> Result<EventResult> {
        match self.content_mode {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    if self.img_state.idx_to_load > 0 {
                        self.img_state.idx_to_load -= 1;
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

    pub fn inc(&mut self, inc: usize) -> Result<EventResult> {
        match self.content_mode {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    if self.img_state.idx_to_load < self.segment_state.segment_count - 1 {
                        self.img_state.idx_to_load += 1;
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
            super::command::Command::Increment(increment) => self.inc(*increment),
            super::command::Command::Decrement(decrement) => self.dec(*decrement),
            super::command::Command::Seek(seek) => self.set(*seek),
            super::command::Command::Noop => Ok(EventResult::Redraw),
        }
    }

    pub fn reexecute_command(&mut self) -> Result<EventResult> {
        let last_command = &self.command_state.last_command.clone();
        self.execute_command(last_command)
    }
}
