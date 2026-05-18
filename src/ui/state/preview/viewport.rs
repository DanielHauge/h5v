use ratatui::layout::Rect;

use crate::{
    data::{DatasetPlotingData, PreviewSelection},
    ui::chart_math::{clamp_axis_range, normalized_axis_bounds, point_in_rect, zoom_axis_range},
};

use super::{
    ChartPreviwState, PreviewChartDragState, PreviewChartRoi, PreviewChartViewport,
    PreviewChartZoomMode, PREVIEW_CHART_VISIBLE_POINT_LIMIT,
};

impl ChartPreviwState {
    pub fn reset_viewport(&mut self) {
        self.viewport = None;
        self.data_bounds = None;
        self.current_data = None;
        self.roi = None;
        self.last_chart_area = None;
        self.last_plot_area = None;
        self.drag_state = None;
    }

    pub fn sync_selection_identity(&mut self, ds_path: &str, selection: &PreviewSelection) {
        if self.ds_loaded.as_deref() != Some(ds_path)
            || self.ds_selection.as_ref() != Some(selection)
        {
            self.reset_viewport();
        }
    }

    pub fn sync_data_bounds(&mut self, bounds: Option<PreviewChartViewport>) {
        self.data_bounds = bounds;
        let Some(full_bounds) = self.data_bounds else {
            self.viewport = None;
            self.current_data = None;
            self.roi = None;
            self.drag_state = None;
            return;
        };
        self.viewport = match self.viewport {
            Some(viewport) => {
                let next = self.clamp_viewport(viewport, full_bounds);
                (!viewport_eq(next, full_bounds)).then_some(next)
            }
            None => None,
        };
    }

    pub fn set_current_data(&mut self, data: Option<DatasetPlotingData>) {
        self.current_data = data;
        if let Some(roi) = self.roi {
            let len = self
                .current_data
                .as_ref()
                .map(|data| data.data.len())
                .unwrap_or(0);
            if roi.start >= len || roi.end >= len {
                self.roi = None;
            }
        }
    }

    pub fn clear_roi(&mut self) -> bool {
        let had_roi = self.roi.is_some();
        self.roi = None;
        had_roi
    }

    pub fn clear_roi_or_zoom(&mut self) -> bool {
        if self.clear_roi() {
            true
        } else {
            self.clear_zoom()
        }
    }

    pub fn effective_viewport(&self) -> Option<PreviewChartViewport> {
        self.viewport.or(self.data_bounds)
    }

    pub fn has_explicit_viewport(&self) -> bool {
        self.viewport.is_some()
    }

    pub fn set_chart_area(&mut self, area: Option<Rect>) {
        self.last_chart_area = area;
        if area.is_none() {
            self.drag_state = None;
        }
    }

    pub fn set_plot_area(&mut self, area: Option<Rect>) {
        self.last_plot_area = area;
    }

    fn clamp_viewport(
        &self,
        viewport: PreviewChartViewport,
        bounds: PreviewChartViewport,
    ) -> PreviewChartViewport {
        let (x_min, x_max) =
            clamp_axis_range(viewport.x_min, viewport.x_max, bounds.x_min, bounds.x_max);
        let (y_min, y_max) =
            clamp_axis_range(viewport.y_min, viewport.y_max, bounds.y_min, bounds.y_max);
        PreviewChartViewport {
            x_min,
            x_max,
            y_min,
            y_max,
        }
    }

    fn set_explicit_viewport(&mut self, viewport: Option<PreviewChartViewport>) -> bool {
        let Some(full_bounds) = self.data_bounds else {
            return false;
        };
        let next = viewport
            .map(|value| self.clamp_viewport(value, full_bounds))
            .filter(|value| !viewport_eq(*value, full_bounds));
        if self.viewport == next {
            return false;
        }
        self.viewport = next;
        true
    }

    pub fn clear_zoom(&mut self) -> bool {
        self.set_explicit_viewport(None)
    }

    pub fn pan_by(&mut self, dx_percent: f64, dy_percent: f64) -> bool {
        let Some(current) = self.effective_viewport() else {
            return false;
        };
        let x_shift = (current.x_max - current.x_min) * dx_percent / 100.0;
        let y_shift = (current.y_max - current.y_min) * dy_percent / 100.0;
        self.set_explicit_viewport(Some(PreviewChartViewport {
            x_min: current.x_min + x_shift,
            x_max: current.x_max + x_shift,
            y_min: current.y_min + y_shift,
            y_max: current.y_max + y_shift,
        }))
    }

    pub fn zoom_with_anchor(
        &mut self,
        percent: f64,
        anchor_x_ratio: f64,
        anchor_y_ratio: f64,
        zoom_in: bool,
        mode: PreviewChartZoomMode,
    ) -> bool {
        let Some(bounds) = self.data_bounds else {
            return false;
        };
        let Some(current) = self.effective_viewport() else {
            return false;
        };

        let (x_min, x_max) = match mode {
            PreviewChartZoomMode::Uniform | PreviewChartZoomMode::XOnly => zoom_axis_range(
                current.x_min,
                current.x_max,
                bounds.x_min,
                bounds.x_max,
                anchor_x_ratio,
                percent,
                zoom_in,
            ),
            PreviewChartZoomMode::YOnly => (current.x_min, current.x_max),
        };
        let (y_min, y_max) = match mode {
            PreviewChartZoomMode::Uniform | PreviewChartZoomMode::YOnly => zoom_axis_range(
                current.y_min,
                current.y_max,
                bounds.y_min,
                bounds.y_max,
                anchor_y_ratio,
                percent,
                zoom_in,
            ),
            PreviewChartZoomMode::XOnly => (current.y_min, current.y_max),
        };
        let changed = self.set_explicit_viewport(Some(PreviewChartViewport {
            x_min,
            x_max,
            y_min,
            y_max,
        }));
        if changed {
            self.roi = None;
        }
        changed
    }

    pub fn zoom_in_at_position(
        &mut self,
        column: u16,
        row: u16,
        percent: f64,
        mode: PreviewChartZoomMode,
    ) -> bool {
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        if !point_in_rect(chart_area, column, row) {
            return false;
        }
        let relative_x = column.saturating_sub(chart_area.x) as f64;
        let relative_y = row.saturating_sub(chart_area.y) as f64;
        let x_denom = chart_area.width.saturating_sub(1).max(1) as f64;
        let y_denom = chart_area.height.saturating_sub(1).max(1) as f64;
        self.zoom_with_anchor(
            percent,
            relative_x / x_denom,
            1.0 - (relative_y / y_denom),
            true,
            mode,
        )
    }

    pub fn zoom_out_at_position(
        &mut self,
        column: u16,
        row: u16,
        percent: f64,
        mode: PreviewChartZoomMode,
    ) -> bool {
        let Some(chart_area) = self.last_plot_area else {
            return false;
        };
        if !point_in_rect(chart_area, column, row) {
            return false;
        }
        let relative_x = column.saturating_sub(chart_area.x) as f64;
        let relative_y = row.saturating_sub(chart_area.y) as f64;
        let x_denom = chart_area.width.saturating_sub(1).max(1) as f64;
        let y_denom = chart_area.height.saturating_sub(1).max(1) as f64;
        self.zoom_with_anchor(
            percent,
            relative_x / x_denom,
            1.0 - (relative_y / y_denom),
            false,
            mode,
        )
    }

    pub fn start_drag_at_position(&mut self, column: u16, row: u16) -> bool {
        if !self.chart_contains_position(column, row) || self.precise_point_mode() {
            return false;
        }
        let Some(viewport) = self.effective_viewport() else {
            return false;
        };
        self.drag_state = Some(PreviewChartDragState {
            anchor_column: column,
            anchor_row: row,
            viewport,
        });
        true
    }

    fn apply_drag_position(&mut self, column: u16, row: u16) -> bool {
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        let Some(drag_state) = self.drag_state else {
            return false;
        };
        if chart_area.width <= 1 || chart_area.height <= 1 {
            return false;
        }
        let delta_columns = column as f64 - drag_state.anchor_column as f64;
        let delta_rows = row as f64 - drag_state.anchor_row as f64;
        let x_span = drag_state.viewport.x_max - drag_state.viewport.x_min;
        let y_span = drag_state.viewport.y_max - drag_state.viewport.y_min;
        let x_shift = (delta_columns / chart_area.width.saturating_sub(1) as f64) * x_span;
        let y_shift = (delta_rows / chart_area.height.saturating_sub(1) as f64) * y_span;
        self.set_explicit_viewport(Some(PreviewChartViewport {
            x_min: drag_state.viewport.x_min - x_shift,
            x_max: drag_state.viewport.x_max - x_shift,
            y_min: drag_state.viewport.y_min + y_shift,
            y_max: drag_state.viewport.y_max + y_shift,
        }))
    }

    pub fn drag_to_position(&mut self, column: u16, row: u16) -> bool {
        let _ = (column, row);
        false
    }

    pub fn finish_drag_at_position(&mut self, column: u16, row: u16) -> bool {
        let changed = self.apply_drag_position(column, row);
        self.drag_state = None;
        changed
    }

    pub fn end_drag(&mut self) {
        self.drag_state = None;
    }

    pub fn chart_contains_position(&self, column: u16, row: u16) -> bool {
        self.last_chart_area
            .is_some_and(|chart_area| point_in_rect(chart_area, column, row))
    }

    fn selection_x_min(&self) -> f64 {
        match self.ds_selection.as_ref().map(|selection| &selection.slice) {
            Some(crate::data::SliceSelection::FromTo(start, _)) => *start as f64,
            _ => 0.0,
        }
    }

    fn visible_index_window(&self) -> Option<(usize, usize)> {
        let data = self.current_data.as_ref()?;
        let viewport = self.effective_viewport()?;
        let x_min = self.selection_x_min();
        let start = (viewport.x_min - x_min).floor().max(0.0) as usize;
        let end = (viewport.x_max - x_min)
            .ceil()
            .max(viewport.x_min - x_min)
            .min(data.data.len().saturating_sub(1) as f64) as usize;
        Some((start.min(end), end.max(start.min(end))))
    }

    fn precise_point_mode(&self) -> bool {
        let Some((start, end)) = self.visible_index_window() else {
            return false;
        };
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        let visible = end.saturating_sub(start).saturating_add(1);
        visible <= PREVIEW_CHART_VISIBLE_POINT_LIMIT
            && chart_area.width as usize >= visible.saturating_mul(2)
    }

    fn roi_at_position(&self, column: u16, row: u16) -> Option<PreviewChartRoi> {
        let chart_area = self.last_plot_area?;
        if !point_in_rect(chart_area, column, row) || chart_area.width == 0 {
            return None;
        }
        let (visible_start, visible_end) = self.visible_index_window()?;
        let visible_len = visible_end.saturating_sub(visible_start).saturating_add(1);
        if visible_len == 0 {
            return None;
        }
        let relative_col = column.saturating_sub(chart_area.x) as usize;
        if self.precise_point_mode() {
            let idx = visible_start
                + ((relative_col as f64 / chart_area.width.saturating_sub(1).max(1) as f64)
                    * visible_len.saturating_sub(1) as f64)
                    .round() as usize;
            let idx = idx.clamp(visible_start, visible_end);
            return Some(PreviewChartRoi {
                start: idx,
                end: idx,
                precise: true,
                selection_count: 1,
            });
        }
        let width = chart_area.width.max(1) as usize;
        let start = visible_start + (relative_col * visible_len) / width;
        let end = visible_start
            + (((relative_col + 1) * visible_len).div_ceil(width))
                .saturating_sub(1)
                .min(visible_len.saturating_sub(1));
        Some(PreviewChartRoi {
            start: start.min(visible_end),
            end: end.min(visible_end).max(start.min(visible_end)),
            precise: false,
            selection_count: 1,
        })
    }

    pub fn cycle_roi_at_position(&mut self, column: u16, row: u16) -> bool {
        let Some(hit) = self.roi_at_position(column, row) else {
            return false;
        };
        self.roi = match self.roi {
            None => Some(hit),
            Some(existing) if existing.selection_count < 2 => Some(PreviewChartRoi {
                start: existing.start.min(hit.start),
                end: existing.end.max(hit.end),
                precise: existing.precise && hit.precise,
                selection_count: 2,
            }),
            Some(_) => None,
        };
        true
    }

    pub fn zoom_to_roi(&mut self) -> bool {
        let Some(roi) = self.roi else {
            return false;
        };
        let Some(data) = self.current_data.as_ref() else {
            return false;
        };
        let x_min = self.selection_x_min();
        let start = roi.start.min(data.data.len().saturating_sub(1));
        let end = roi.end.min(data.data.len().saturating_sub(1)).max(start);
        let slice = &data.data[start..=end];
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        for &(_, y) in slice {
            if y.is_finite() {
                y_min = y_min.min(y);
                y_max = y_max.max(y);
            }
        }
        let x_start = x_min + start as f64;
        let x_end = x_min + end as f64 + if roi.precise { 0.0 } else { 1.0 };
        let Some((x_min, x_max)) = normalized_axis_bounds(x_start, x_end) else {
            return false;
        };
        let Some((y_min, y_max)) = normalized_axis_bounds(y_min, y_max) else {
            return false;
        };
        let changed = self.set_explicit_viewport(Some(PreviewChartViewport {
            x_min,
            x_max,
            y_min,
            y_max,
        }));
        if changed {
            self.roi = None;
        }
        changed
    }
}

fn viewport_eq(left: PreviewChartViewport, right: PreviewChartViewport) -> bool {
    (left.x_min - right.x_min).abs() < 1e-9
        && (left.x_max - right.x_max).abs() < 1e-9
        && (left.y_min - right.y_min).abs() < 1e-9
        && (left.y_max - right.y_max).abs() < 1e-9
}
