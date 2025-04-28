use std::{cell::RefCell, io::BufReader, rc::Rc, sync::mpsc::Sender};

use hdf5_metno::{ByteReader, Dataset};
use image::ImageFormat;
use ratatui_image::thread::ThreadProtocol;

use crate::{
    error::AppError,
    h5f::{H5FNode, ImageType, Node},
    search::Searcher,
};

use super::tree_view::TreeItem;

pub enum Focus {
    Tree,
    Attributes,
}

#[derive(Debug, Clone)]
pub enum Mode {
    Normal,
    Search,
    Help,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum ContentShowMode {
    Preview,
    Matrix,
    Heatmap,
}

pub struct ImgState {
    pub protocol: Option<ThreadProtocol>,
    pub tx_load_imgfs: Sender<(BufReader<ByteReader>, ImageFormat)>,
    pub tx_load_img: Sender<(Dataset, ImageType)>,
    pub ds: Option<String>,
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
                if *name == ds_name_str {
                    return true;
                }
                false
            }
            _ => false,
        }
    }
}

pub struct AppState<'a> {
    pub root: Rc<RefCell<H5FNode>>,
    pub treeview: Vec<TreeItem<'a>>,
    pub tree_view_cursor: usize,
    pub help: bool,
    pub focus: Focus,
    pub mode: Mode,
    pub indexed: bool,
    pub searcher: Rc<RefCell<Searcher>>,
    pub show_tree_view: bool,
    pub content_mode: ContentShowMode,
    pub selected_x_dim: usize,
    // pub selected_y_dim: usize,
    pub selected_indexes: [usize; 15], // WARN: Will we ever need more than 15 dimensions?
    pub img_state: ImgState,
}

type Result<T> = std::result::Result<T, AppError>;
impl AppState<'_> {
    pub fn index(&mut self) -> Result<()> {
        let mut root = self.root.borrow_mut();
        root.index(true)?;
        self.indexed = true;
        Ok(())
    }
}
