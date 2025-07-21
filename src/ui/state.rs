use std::{cell::RefCell, io::BufReader, rc::Rc, sync::mpsc::Sender};

use cli_clipboard::ClipboardContext;
use hdf5_metno::{ByteReader, Dataset};
use image::ImageFormat;
use ratatui_image::thread::ThreadProtocol;

use crate::{
    error::AppError,
    h5f::{H5FNode, ImageType, Node},
    search::Searcher,
};

use super::{command::CommandState, tree_view::TreeItem};

#[derive(Debug, Clone)]
pub enum LastFocused {
    Attributes,
    Content,
    Tree,
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

pub struct SegmentState {
    pub idx: i32,
    pub segumented: bool,
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
    pub mode: Mode,
    pub indexed: bool,
    pub searcher: Rc<RefCell<Searcher>>,
    pub show_tree_view: bool,
    pub content_mode: ContentShowMode,
    pub selected_x_dim: usize,
    pub selected_y_dim: usize,
    pub selected_indexes: [usize; 15], // WARN: Will we ever need more than 15 dimensions?
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

    pub fn available_content_show_modes(&self) -> Vec<ContentShowMode> {
        let current_node = &self.treeview[self.tree_view_cursor].node;
        let available_content_modes = current_node.borrow().content_show_modes();
        available_content_modes
    }

    pub fn swap_content_show_mode(&mut self) {
        let available_content_modes = self.available_content_show_modes();
        if available_content_modes.is_empty() {
            return;
        }
        match self.content_mode {
            ContentShowMode::Preview
                if available_content_modes.contains(&ContentShowMode::Matrix) =>
            {
                self.content_mode = ContentShowMode::Matrix;
            }
            _ => {
                self.content_mode = ContentShowMode::Preview;
            }
        }
    }

    pub fn content_show_mode_eval(&self) -> ContentShowMode {
        let available_content_modes = self.available_content_show_modes();
        if available_content_modes.contains(&self.content_mode) {
            self.content_mode
        } else {
            available_content_modes[0]
        }
    }

    pub fn execute_command(&mut self) -> Result<()> {
        let command = self.command_state.parse_command()?;
        match command {
            super::command::Command::Increment(increment) => {
                self.img_state.idx_to_load += increment as i32;
            }
            super::command::Command::Decrement(decrement) => {
                self.img_state.idx_to_load -= decrement as i32;
            }
            super::command::Command::Seek(seek) => {
                self.img_state.idx_to_load = seek as i32;
            }
        }
        Ok(())
    }
}
