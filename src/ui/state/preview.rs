use std::{
    collections::{HashSet, VecDeque},
    io::BufReader,
    sync::mpsc::Sender,
};

use hdf5_metno::{ByteReader, Dataset};
use image::ImageFormat;
use ratatui::layout::Rect;
use ratatui_image::thread::{ResizeRequest, ThreadProtocol};

use crate::{
    data::{DatasetPlotingData, PreviewSelection},
    h5f::{DatasetMeta, ImageType},
    ui::mchart::ChartItem,
};

mod cache;
mod viewport;

pub const PREVIEW_CHART_VISIBLE_POINT_LIMIT: usize = 50;

pub struct ChartPreviewLoadRequest {
    pub key: ChartPreviewKey,
    pub source: ChartPreviewSource,
    pub page_state: PageState,
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
    pub rendered_viewport: Option<PreviewChartViewport>,
    pub rendered_roi: Option<PreviewChartRoi>,
    pub rendered_size: Option<(u16, u16)>,
    pub pending_key: Option<ChartPreviewKey>,
    pub tx_resize_chartpreview: Sender<ResizeRequest>,
    pub tx_load_chartpreview: Sender<ChartPreviewLoadRequest>,
    pub cached_previews: VecDeque<CachedChartPreview>,
    pub viewport: Option<PreviewChartViewport>,
    pub data_bounds: Option<PreviewChartViewport>,
    pub current_data: Option<DatasetPlotingData>,
    pub roi: Option<PreviewChartRoi>,
    pub last_chart_area: Option<Rect>,
    pub last_plot_area: Option<Rect>,
    pub drag_state: Option<PreviewChartDragState>,
}

#[derive(Debug, Clone, Copy)]
pub struct PreviewChartViewport {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl PartialEq for PreviewChartViewport {
    fn eq(&self, other: &Self) -> bool {
        self.x_min.to_bits() == other.x_min.to_bits()
            && self.x_max.to_bits() == other.x_max.to_bits()
            && self.y_min.to_bits() == other.y_min.to_bits()
            && self.y_max.to_bits() == other.y_max.to_bits()
    }
}

impl Eq for PreviewChartViewport {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewChartZoomMode {
    Uniform,
    XOnly,
    YOnly,
}

#[derive(Debug, Clone, Copy)]
pub struct PreviewChartDragState {
    pub anchor_column: u16,
    pub anchor_row: u16,
    pub viewport: PreviewChartViewport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewChartRoi {
    pub start: usize,
    pub end: usize,
    pub precise: bool,
    pub selection_count: u8,
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
    pub viewport: Option<PreviewChartViewport>,
    pub roi: Option<PreviewChartRoi>,
    pub width: u16,
    pub height: u16,
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
    pub data_bounds: PreviewChartViewport,
    pub data_preview: DatasetPlotingData,
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
            viewport: None,
            roi: None,
            width: 10,
            height: 10,
        }
    }

    fn bounds() -> PreviewChartViewport {
        PreviewChartViewport {
            x_min: 0.0,
            x_max: 1.0,
            y_min: 0.0,
            y_max: 1.0,
        }
    }

    fn clipboard_image(id: u8) -> ClipboardImageData {
        ClipboardImageData {
            width: 1,
            height: 1,
            bytes: vec![id, 0, 0, 255],
        }
    }

    fn data_preview() -> DatasetPlotingData {
        DatasetPlotingData {
            data: vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
            length: 3,
            min: 1.0,
            max: 3.0,
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
            rendered_viewport: None,
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: None,
            current_data: None,
            roi: None,
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };
        let first = preview_key("first");
        let second = preview_key("second");
        state.cache_preview(
            first.clone(),
            clipboard_image(1),
            bounds(),
            data_preview(),
            2,
        );
        state.cache_preview(
            second.clone(),
            clipboard_image(2),
            bounds(),
            data_preview(),
            2,
        );

        let (touched, cached_bounds, cached_data) = state.touch_cached_preview(&first).unwrap();

        assert_eq!(touched.bytes, clipboard_image(1).bytes);
        assert_eq!(cached_bounds, bounds());
        assert_eq!(cached_data.length, 3);
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
            rendered_viewport: None,
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: None,
            current_data: None,
            roi: None,
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };

        state.cache_preview(
            preview_key("first"),
            clipboard_image(1),
            bounds(),
            data_preview(),
            2,
        );
        state.cache_preview(
            preview_key("second"),
            clipboard_image(2),
            bounds(),
            data_preview(),
            2,
        );
        state.cache_preview(
            preview_key("third"),
            clipboard_image(3),
            bounds(),
            data_preview(),
            2,
        );

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
            rendered_viewport: Some(bounds()),
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: None,
            current_data: Some(data_preview()),
            roi: None,
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };

        let key = preview_key("fresh");
        state.begin_loading(key.clone());

        assert_eq!(state.ds_loaded, Some("fresh".to_string()));
        assert_eq!(state.ds_selection, Some(key.selection.clone()));
        assert_eq!(state.rendered_viewport, None);
        assert!(state.clipboard_image.is_none());
        assert!(state.error.is_none());
        assert_eq!(state.pending_key, Some(key));
    }

    #[test]
    fn chart_preview_current_request_key_tracks_rendered_viewport() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let viewport = PreviewChartViewport {
            x_min: 1.0,
            x_max: 5.0,
            y_min: -2.0,
            y_max: 3.0,
        };
        let key = ChartPreviewKey {
            viewport: Some(viewport),
            ..preview_key("viewported")
        };
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            rendered_viewport: None,
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: None,
            current_data: None,
            roi: None,
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };

        state.begin_loading(key.clone());

        assert_eq!(state.current_request_key(), Some(key));
    }

    #[test]
    fn chart_preview_sync_selection_identity_clears_existing_viewport() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let key = preview_key("same");
        let mut state = ChartPreviwState {
            ds_loaded: Some(key.ds_path.clone()),
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: Some(key.selection.clone()),
            rendered_viewport: None,
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: Some(bounds()),
            data_bounds: Some(bounds()),
            current_data: Some(data_preview()),
            roi: None,
            last_chart_area: Some(Rect::new(0, 0, 10, 10)),
            last_plot_area: Some(Rect::new(0, 0, 10, 10)),
            drag_state: None,
        };
        let changed_selection = PreviewSelection {
            x: 0,
            index: vec![1],
            slice: crate::data::SliceSelection::All,
        };

        state.sync_selection_identity("same", &changed_selection);

        assert!(state.viewport.is_none());
        assert!(state.data_bounds.is_none());
        assert!(state.last_chart_area.is_none());
    }

    #[test]
    fn chart_preview_zoom_with_anchor_creates_explicit_viewport() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            rendered_viewport: None,
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 10.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            current_data: None,
            roi: None,
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };

        assert!(state.zoom_with_anchor(10.0, 0.5, 0.5, true, PreviewChartZoomMode::Uniform));
        assert!(state.viewport.is_some());
        assert_ne!(state.viewport, state.data_bounds);
    }

    #[test]
    fn chart_preview_zoom_with_anchor_clears_roi() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            rendered_viewport: None,
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 10.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            current_data: Some(data_preview()),
            roi: Some(PreviewChartRoi {
                start: 0,
                end: 1,
                precise: true,
                selection_count: 1,
            }),
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };

        assert!(state.zoom_with_anchor(10.0, 0.5, 0.5, true, PreviewChartZoomMode::Uniform));
        assert!(state.roi.is_none());
    }

    #[test]
    fn chart_preview_drag_updates_viewport_before_mouse_up() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let initial_viewport = PreviewChartViewport {
            x_min: 2.0,
            x_max: 8.0,
            y_min: 1.0,
            y_max: 9.0,
        };
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            rendered_viewport: None,
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: Some(initial_viewport),
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 10.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            current_data: None,
            roi: None,
            last_chart_area: Some(Rect::new(0, 0, 10, 10)),
            last_plot_area: None,
            drag_state: None,
        };

        assert!(state.start_drag_at_position(5, 5));
        assert!(!state.drag_to_position(6, 5));
        assert_eq!(state.viewport, Some(initial_viewport));
        assert!(state.finish_drag_at_position(6, 5));
        assert_ne!(state.viewport, Some(initial_viewport));
    }

    #[test]
    fn chart_preview_roi_coarse_clicks_cycle_from_first_to_second_to_clear() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: Some(preview_key("roi").selection),
            rendered_viewport: None,
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 100.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 100.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            current_data: Some(DatasetPlotingData {
                data: (0..100).map(|i| (i as f64, i as f64)).collect(),
                length: 100,
                min: 0.0,
                max: 99.0,
            }),
            roi: None,
            last_chart_area: Some(Rect::new(5, 3, 10, 10)),
            last_plot_area: Some(Rect::new(5, 3, 10, 10)),
            drag_state: None,
        };

        assert!(state.cycle_roi_at_position(6, 4));
        let first = state.roi.expect("first roi");
        assert_eq!(first.selection_count, 1);
        assert!(first.end >= first.start);

        assert!(state.cycle_roi_at_position(13, 4));
        let second = state.roi.expect("second roi");
        assert_eq!(second.selection_count, 2);
        assert!(second.end > first.end);

        assert!(state.cycle_roi_at_position(10, 4));
        assert!(state.roi.is_none());
    }

    #[test]
    fn chart_preview_zoom_to_roi_allows_single_selection() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: Some(preview_key("roi").selection),
            rendered_viewport: None,
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 10.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            current_data: Some(data_preview()),
            roi: Some(PreviewChartRoi {
                start: 0,
                end: 2,
                precise: false,
                selection_count: 1,
            }),
            last_chart_area: Some(Rect::new(0, 0, 10, 10)),
            last_plot_area: Some(Rect::new(0, 0, 10, 10)),
            drag_state: None,
        };

        assert!(state.zoom_to_roi());
        assert!(state.viewport.is_some());
        assert!(state.roi.is_none());
    }

    #[test]
    fn chart_preview_starts_drag_when_points_are_not_selectable() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: Some(preview_key("drag").selection),
            rendered_viewport: None,
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 100.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 100.0,
                y_min: 0.0,
                y_max: 10.0,
            }),
            current_data: Some(DatasetPlotingData {
                data: (0..100).map(|i| (i as f64, i as f64)).collect(),
                length: 100,
                min: 0.0,
                max: 99.0,
            }),
            roi: None,
            last_chart_area: Some(Rect::new(5, 3, 10, 10)),
            last_plot_area: Some(Rect::new(5, 3, 10, 10)),
            drag_state: None,
        };

        assert!(state.start_drag_at_position(6, 4));
        assert!(state.drag_state.is_some());
    }

    #[test]
    fn chart_preview_blocks_drag_when_precise_points_are_visible() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: Some(preview_key("drag").selection),
            rendered_viewport: None,
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 3.0,
                y_min: 0.0,
                y_max: 3.0,
            }),
            data_bounds: Some(PreviewChartViewport {
                x_min: 0.0,
                x_max: 3.0,
                y_min: 0.0,
                y_max: 3.0,
            }),
            current_data: Some(DatasetPlotingData {
                data: (0..3).map(|i| (i as f64, i as f64)).collect(),
                length: 3,
                min: 0.0,
                max: 2.0,
            }),
            roi: Some(PreviewChartRoi {
                start: 0,
                end: 1,
                precise: true,
                selection_count: 1,
            }),
            last_chart_area: Some(Rect::new(5, 3, 10, 10)),
            last_plot_area: Some(Rect::new(5, 3, 10, 10)),
            drag_state: None,
        };

        assert!(!state.start_drag_at_position(6, 4));
        assert!(state.drag_state.is_none());
    }

    #[test]
    fn chart_preview_clear_roi_or_zoom_prefers_roi() {
        let (tx_resize_chartpreview, _) = channel();
        let (tx_load_chartpreview, _) = channel();
        let mut state = ChartPreviwState {
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            rendered_viewport: None,
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview,
            tx_load_chartpreview,
            cached_previews: Default::default(),
            viewport: Some(bounds()),
            data_bounds: Some(bounds()),
            current_data: Some(data_preview()),
            roi: Some(PreviewChartRoi {
                start: 0,
                end: 1,
                precise: false,
                selection_count: 2,
            }),
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        };

        assert!(state.clear_roi_or_zoom());
        assert!(state.roi.is_none());
        assert!(state.viewport.is_some());
        assert!(state.clear_roi_or_zoom());
        assert!(state.viewport.is_none());
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
