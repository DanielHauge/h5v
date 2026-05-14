use std::{
    collections::{HashSet, VecDeque},
    io::BufReader,
    sync::mpsc::Sender,
};

use hdf5_metno::{ByteReader, Dataset};
use image::ImageFormat;
use ratatui_image::thread::{ResizeRequest, ThreadProtocol};

use crate::{
    data::{DatasetPlotingData, PreviewSelection},
    h5f::{DatasetMeta, HasPath, ImageType, Node},
};

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
    ProjectedDataset {
        ds: Dataset,
        meta: DatasetMeta,
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
    pub pending_key: Option<ChartPreviewKey>,
    pub tx_load_chartpreview: Sender<ChartPreviewLoadRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageLoadKey {
    pub ds_path: String,
    pub idx: i32,
    pub window_axis: Option<ImageWindowAxis>,
    pub window_start: usize,
    pub window_len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub tx_resize_img: Sender<ResizeRequest>,
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
    pub cached_images: VecDeque<CachedImage>,
    pub pending_keys: HashSet<ImageLoadKey>,
}

#[derive(Debug, Clone)]
pub struct CachedImage {
    pub key: ImageLoadKey,
    pub clipboard_image: ClipboardImageData,
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

    pub fn has_cached_image(&self, key: &ImageLoadKey) -> bool {
        self.cached_images.iter().any(|entry| &entry.key == key)
    }

    pub fn touch_cached_image(&mut self, key: &ImageLoadKey) -> Option<ClipboardImageData> {
        let index = self
            .cached_images
            .iter()
            .position(|entry| &entry.key == key)?;
        let entry = self.cached_images.remove(index)?;
        let clipboard_image = entry.clipboard_image.clone();
        self.cached_images.push_back(entry);
        Some(clipboard_image)
    }

    pub fn cache_image(
        &mut self,
        key: ImageLoadKey,
        clipboard_image: ClipboardImageData,
        capacity: usize,
    ) {
        self.cached_images.retain(|entry| entry.key != key);
        self.cached_images.push_back(CachedImage {
            key,
            clipboard_image,
        });
        while self.cached_images.len() > capacity {
            self.cached_images.pop_front();
        }
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
    pub fn max_index(&self) -> Option<i32> {
        (self.segment_count > 0).then_some(self.segment_count.saturating_sub(1))
    }
}
