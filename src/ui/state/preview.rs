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
    h5f::{DatasetMeta, ImageType},
    ui::mchart::ChartItem,
};

pub struct ChartPreviewLoadRequest {
    pub ds_path: String,
    pub source: ChartPreviewSource,
    pub page_state: PageState,
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
        meta: Box<DatasetMeta>,
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
    pub tx_resize_chartpreview: Sender<ResizeRequest>,
    pub tx_load_chartpreview: Sender<ChartPreviewLoadRequest>,
    pub cached_previews: VecDeque<CachedChartPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewExpressionKey {
    pub group_path: String,
    pub expression: String,
    pub expression_revision: u64,
}

#[derive(Debug, Clone)]
pub struct PreviewExpressionRequest {
    pub key: PreviewExpressionKey,
    pub items: Vec<ChartItem>,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PreviewExpressionResult {
    Success {
        key: PreviewExpressionKey,
        data_preview: DatasetPlotingData,
    },
    Failure {
        key: PreviewExpressionKey,
        message: String,
    },
}

pub struct PreviewExpressionState {
    pub current_key: Option<PreviewExpressionKey>,
    pub pending_key: Option<PreviewExpressionKey>,
    pub data_preview: Option<DatasetPlotingData>,
    pub error: Option<String>,
    pub tx_load: Sender<PreviewExpressionRequest>,
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

pub const CHART_PREVIEW_CACHE_CAPACITY: usize = 6;

#[derive(Debug, Clone)]
pub struct CachedChartPreview {
    pub key: ChartPreviewKey,
    pub clipboard_image: ClipboardImageData,
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

    pub fn begin_loading(&mut self, key: ImageLoadKey, idx_loaded: i32) {
        self.protocol = None;
        self.clipboard_image = None;
        self.error = None;
        self.ds = Some(key.ds_path.clone());
        self.current_key = Some(key.clone());
        self.idx_loaded = idx_loaded;
        self.pending_keys.insert(key);
    }
}

impl ChartPreviwState {
    pub fn current_request_key(&self) -> Option<ChartPreviewKey> {
        Some(ChartPreviewKey {
            ds_path: self.ds_loaded.clone()?,
            selection: self.ds_selection.clone()?,
        })
    }

    pub fn touch_cached_preview(&mut self, key: &ChartPreviewKey) -> Option<ClipboardImageData> {
        let index = self
            .cached_previews
            .iter()
            .position(|entry| &entry.key == key)?;
        let entry = self.cached_previews.remove(index)?;
        let clipboard_image = entry.clipboard_image.clone();
        self.cached_previews.push_back(entry);
        Some(clipboard_image)
    }

    pub fn cache_preview(
        &mut self,
        key: ChartPreviewKey,
        clipboard_image: ClipboardImageData,
        capacity: usize,
    ) {
        self.cached_previews.retain(|entry| entry.key != key);
        self.cached_previews.push_back(CachedChartPreview {
            key,
            clipboard_image,
        });
        while self.cached_previews.len() > capacity {
            self.cached_previews.pop_front();
        }
    }

    pub fn begin_loading(&mut self, key: ChartPreviewKey) {
        self.ds_loaded = Some(key.ds_path.clone());
        self.ds_selection = Some(key.selection.clone());
        self.protocol = None;
        self.clipboard_image = None;
        self.error = None;
        self.pending_key = Some(key);
    }
}

#[derive(Clone)]
pub enum PageType {
    Image,
    Chart,
    Unpaged,
}

#[derive(Clone)]
pub struct PageState {
    pub idx: i32,
    pub paged: PageType,
    pub page_count: i32,
}

impl PageState {
    pub fn max_index(&self) -> Option<i32> {
        (self.page_count > 0).then_some(self.page_count.saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn preview_key(name: &str) -> ChartPreviewKey {
        ChartPreviewKey {
            ds_path: name.to_string(),
            selection: PreviewSelection {
                index: vec![0],
                x: 0,
                slice: crate::data::SliceSelection::All,
            },
        }
    }

    fn clipboard_image(id: u8) -> ClipboardImageData {
        ClipboardImageData {
            width: 1,
            height: 1,
            bytes: vec![id, 0, 0, 255],
        }
    }

    #[test]
    fn chart_preview_cache_touches_existing_entries() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
        };
        let first = preview_key("first");
        let second = preview_key("second");
        state.cache_preview(first.clone(), clipboard_image(1), 2);
        state.cache_preview(second.clone(), clipboard_image(2), 2);

        let touched = state.touch_cached_preview(&first).unwrap();

        assert_eq!(touched.bytes, clipboard_image(1).bytes);
        assert_eq!(state.cached_previews.back().unwrap().key, first);
        assert_eq!(state.cached_previews.front().unwrap().key, second);
    }

    #[test]
    fn chart_preview_cache_respects_capacity() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
        };

        state.cache_preview(preview_key("first"), clipboard_image(1), 2);
        state.cache_preview(preview_key("second"), clipboard_image(2), 2);
        state.cache_preview(preview_key("third"), clipboard_image(3), 2);

        assert!(state
            .cached_previews
            .iter()
            .all(|entry| entry.key != preview_key("first")));
        assert!(state
            .cached_previews
            .iter()
            .any(|entry| entry.key == preview_key("second")));
        assert!(state
            .cached_previews
            .iter()
            .any(|entry| entry.key == preview_key("third")));
    }

    #[test]
    fn chart_preview_begin_loading_clears_active_preview_state() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: Some("stale".to_string()),
            protocol: None,
            clipboard_image: Some(clipboard_image(9)),
            error: Some("old".to_string()),
            ds_selection: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
        };

        let key = preview_key("fresh");
        state.begin_loading(key.clone());

        assert_eq!(state.ds_loaded, Some("fresh".to_string()));
        assert_eq!(state.ds_selection, Some(key.selection.clone()));
        assert!(state.clipboard_image.is_none());
        assert!(state.error.is_none());
        assert_eq!(state.pending_key, Some(key));
    }

    #[test]
    fn image_begin_loading_clears_active_image_state() {
        let (tx_resize_img, _) = channel();
        let (tx_load_imgfs, _) = channel();
        let (tx_load_imgfsvlen, _) = channel();
        let (tx_load_img, _) = channel();
        let mut state = ImgState {
            protocol: None,
            tx_resize_img,
            tx_load_imgfs,
            tx_load_imgfsvlen,
            tx_load_img,
            ds: Some("stale".to_string()),
            current_key: None,
            clipboard_image: Some(clipboard_image(7)),
            window: None,
            error: Some("old".to_string()),
            idx_to_load: 3,
            idx_loaded: -1,
            cached_images: Default::default(),
            pending_keys: Default::default(),
        };
        let key = ImageLoadKey {
            ds_path: "fresh".to_string(),
            idx: 2,
            window_axis: None,
            window_start: 0,
            window_len: 0,
        };

        state.begin_loading(key.clone(), 2);

        assert_eq!(state.ds, Some("fresh".to_string()));
        assert_eq!(state.current_key, Some(key.clone()));
        assert!(state.clipboard_image.is_none());
        assert!(state.error.is_none());
        assert_eq!(state.idx_loaded, 2);
        assert!(state.pending_keys.contains(&key));
    }
}
