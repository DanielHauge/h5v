use super::{
    CachedChartPreview, CachedImage, ChartPreviewKey, ChartPreviwState, ClipboardImageData,
    DatasetPlotingData, ImageLoadKey, ImgState, PreviewChartViewport,
};

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
        let (width, height) = self.rendered_size?;
        Some(ChartPreviewKey {
            ds_path: self.ds_loaded.clone()?,
            selection: self.ds_selection.clone()?,
            viewport: self.rendered_viewport,
            roi: self.rendered_roi,
            width,
            height,
        })
    }

    pub fn touch_cached_preview(
        &mut self,
        key: &ChartPreviewKey,
    ) -> Option<(ClipboardImageData, PreviewChartViewport, DatasetPlotingData)> {
        let index = self
            .cached_previews
            .iter()
            .position(|entry| &entry.key == key)?;
        let entry = self.cached_previews.remove(index)?;
        let clipboard_image = entry.clipboard_image.clone();
        let data_bounds = entry.data_bounds;
        let data_preview = entry.data_preview.clone();
        self.cached_previews.push_back(entry);
        Some((clipboard_image, data_bounds, data_preview))
    }

    pub fn cache_preview(
        &mut self,
        key: ChartPreviewKey,
        clipboard_image: ClipboardImageData,
        data_bounds: PreviewChartViewport,
        data_preview: DatasetPlotingData,
        capacity: usize,
    ) {
        self.cached_previews.retain(|entry| entry.key != key);
        self.cached_previews.push_back(CachedChartPreview {
            key,
            clipboard_image,
            data_bounds,
            data_preview,
        });
        while self.cached_previews.len() > capacity {
            self.cached_previews.pop_front();
        }
    }

    pub fn begin_loading(&mut self, key: ChartPreviewKey) {
        self.ds_loaded = Some(key.ds_path.clone());
        self.ds_selection = Some(key.selection.clone());
        self.rendered_viewport = key.viewport;
        self.rendered_roi = key.roi;
        self.rendered_size = Some((key.width, key.height));
        self.protocol = None;
        self.clipboard_image = None;
        self.error = None;
        self.current_data = None;
        self.pending_key = Some(key);
    }
}
