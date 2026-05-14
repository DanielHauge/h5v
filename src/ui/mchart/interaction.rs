use ratatui::layout::Rect;

use super::{ChartDragState, ChartViewport, ChartZoomMode, MultiChartState};

fn point_in_rect(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn viewport_eq(left: ChartViewport, right: ChartViewport) -> bool {
    (left.x_min - right.x_min).abs() < 1e-9
        && (left.x_max - right.x_max).abs() < 1e-9
        && (left.y_min - right.y_min).abs() < 1e-9
        && (left.y_max - right.y_max).abs() < 1e-9
}

fn normalized_axis_bounds(min: f64, max: f64) -> Option<(f64, f64)> {
    if !min.is_finite() || !max.is_finite() {
        return None;
    }
    if max < min {
        return None;
    }
    if (max - min).abs() < f64::EPSILON {
        let pad = if min == 0.0 {
            1.0
        } else {
            min.abs().max(1.0) * 0.05
        };
        return Some((min - pad, max + pad));
    }
    Some((min, max))
}

fn minimum_zoom_span(bounds_min: f64, bounds_max: f64) -> f64 {
    let span = (bounds_max - bounds_min).abs();
    span.mul_add(1e-6, f64::EPSILON).max(1e-9)
}

fn clamp_axis_range(mut start: f64, mut end: f64, bounds_min: f64, bounds_max: f64) -> (f64, f64) {
    if bounds_max <= bounds_min {
        return (bounds_min, bounds_max);
    }
    if start > end {
        std::mem::swap(&mut start, &mut end);
    }
    let bounds_span = bounds_max - bounds_min;
    let span = (end - start)
        .max(minimum_zoom_span(bounds_min, bounds_max))
        .min(bounds_span);
    if span >= bounds_span {
        return (bounds_min, bounds_max);
    }

    let mut clamped_start = start;
    let mut clamped_end = clamped_start + span;
    if clamped_start < bounds_min {
        clamped_end += bounds_min - clamped_start;
        clamped_start = bounds_min;
    }
    if clamped_end > bounds_max {
        let overflow = clamped_end - bounds_max;
        clamped_start -= overflow;
        clamped_end = bounds_max;
    }
    clamped_start = clamped_start.max(bounds_min);
    clamped_end = clamped_end.min(bounds_max);
    (clamped_start, clamped_end)
}

fn zoom_axis_range(
    current_min: f64,
    current_max: f64,
    bounds_min: f64,
    bounds_max: f64,
    anchor_ratio: f64,
    percent: f64,
    zoom_in: bool,
) -> (f64, f64) {
    let current_span = (current_max - current_min).abs();
    let bounds_span = (bounds_max - bounds_min).abs();
    if bounds_span <= f64::EPSILON {
        return (bounds_min, bounds_max);
    }

    let anchor_ratio = anchor_ratio.clamp(0.0, 1.0);
    let delta = current_span * percent / 100.0;
    let min_span = minimum_zoom_span(bounds_min, bounds_max);
    let next_span = if zoom_in {
        (current_span - 2.0 * delta).max(min_span)
    } else {
        (current_span + 2.0 * delta).min(bounds_span)
    };
    let anchor = current_min + current_span * anchor_ratio;
    let next_min = anchor - next_span * anchor_ratio;
    let next_max = next_min + next_span;
    clamp_axis_range(next_min, next_max, bounds_min, bounds_max)
}

impl MultiChartState {
    fn bounds_from_points<'a>(
        points: impl Iterator<Item = &'a (f64, f64)>,
    ) -> Option<ChartViewport> {
        let mut iter = points
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .peekable();
        let &(mut x_min, mut y_min) = iter.peek()?;
        let mut x_max = x_min;
        let mut y_max = y_min;
        for &(x, y) in iter {
            x_min = x_min.min(x);
            x_max = x_max.max(x);
            y_min = y_min.min(y);
            y_max = y_max.max(y);
        }
        let (x_min, x_max) = normalized_axis_bounds(x_min, x_max)?;
        let (y_min, y_max) = normalized_axis_bounds(y_min, y_max)?;
        Some(ChartViewport {
            x_min,
            x_max,
            y_min,
            y_max,
        })
    }

    pub(super) fn visible_data_bounds(&self) -> Option<ChartViewport> {
        Self::bounds_from_points(
            self.items
                .iter()
                .filter(|item| item.visible && item.has_loaded_series())
                .flat_map(|item| item.overview_series().points.iter()),
        )
    }

    pub(super) fn selected_data_bounds(&self) -> Option<ChartViewport> {
        let item = self.selected_item()?;
        if !item.has_loaded_series() {
            return None;
        }
        Self::bounds_from_points(item.overview_series().points.iter())
    }

    pub(super) fn effective_viewport(&self) -> Option<ChartViewport> {
        self.viewport.or_else(|| self.visible_data_bounds())
    }

    fn clamp_viewport(&self, viewport: ChartViewport, bounds: ChartViewport) -> ChartViewport {
        let (x_min, x_max) =
            clamp_axis_range(viewport.x_min, viewport.x_max, bounds.x_min, bounds.x_max);
        let (y_min, y_max) =
            clamp_axis_range(viewport.y_min, viewport.y_max, bounds.y_min, bounds.y_max);
        ChartViewport {
            x_min,
            x_max,
            y_min,
            y_max,
        }
    }

    fn set_explicit_viewport(&mut self, viewport: Option<ChartViewport>) -> bool {
        let Some(full_bounds) = self.visible_data_bounds() else {
            return false;
        };
        let next = viewport.map(|value| self.clamp_viewport(value, full_bounds));
        let next = match next {
            Some(value) if viewport_eq(value, full_bounds) => None,
            other => other,
        };
        if self.viewport == next {
            return false;
        }
        self.viewport = next;
        if self.viewport.is_none() {
            for item in &mut self.items {
                item.clear_detail_state(true);
            }
        }
        self.modified = true;
        true
    }

    fn set_viewport_from_bounds(&mut self, viewport: ChartViewport) -> bool {
        self.set_explicit_viewport(Some(viewport))
    }

    pub fn fit_all(&mut self) -> bool {
        self.clear_zoom()
    }

    pub fn fit_selected(&mut self) -> bool {
        let Some(bounds) = self.selected_data_bounds() else {
            return false;
        };
        self.set_viewport_from_bounds(bounds)
    }

    pub(super) fn zoom_with_anchor(
        &mut self,
        percent: f64,
        anchor_x_ratio: f64,
        anchor_y_ratio: f64,
        zoom_in: bool,
        mode: ChartZoomMode,
    ) -> bool {
        let Some(bounds) = self.visible_data_bounds() else {
            return false;
        };
        let Some(current) = self.effective_viewport() else {
            return false;
        };

        let (x_min, x_max) = match mode {
            ChartZoomMode::Uniform | ChartZoomMode::XOnly => zoom_axis_range(
                current.x_min,
                current.x_max,
                bounds.x_min,
                bounds.x_max,
                anchor_x_ratio,
                percent,
                zoom_in,
            ),
            ChartZoomMode::YOnly => (current.x_min, current.x_max),
        };
        let (y_min, y_max) = match mode {
            ChartZoomMode::Uniform | ChartZoomMode::YOnly => zoom_axis_range(
                current.y_min,
                current.y_max,
                bounds.y_min,
                bounds.y_max,
                anchor_y_ratio,
                percent,
                zoom_in,
            ),
            ChartZoomMode::XOnly => (current.y_min, current.y_max),
        };

        self.set_viewport_from_bounds(ChartViewport {
            x_min,
            x_max,
            y_min,
            y_max,
        })
    }

    pub fn zoom_in(&mut self, percent: f64) -> bool {
        self.zoom_with_anchor(percent, 0.5, 0.5, true, ChartZoomMode::Uniform)
    }

    pub fn zoom_out(&mut self, percent: f64) -> bool {
        self.zoom_with_anchor(percent, 0.5, 0.5, false, ChartZoomMode::Uniform)
    }

    pub fn zoom_in_x(&mut self, percent: f64) -> bool {
        self.zoom_with_anchor(percent, 0.5, 0.5, true, ChartZoomMode::XOnly)
    }

    pub fn zoom_out_x(&mut self, percent: f64) -> bool {
        self.zoom_with_anchor(percent, 0.5, 0.5, false, ChartZoomMode::XOnly)
    }

    pub fn zoom_in_y(&mut self, percent: f64) -> bool {
        self.zoom_with_anchor(percent, 0.5, 0.5, true, ChartZoomMode::YOnly)
    }

    pub fn zoom_out_y(&mut self, percent: f64) -> bool {
        self.zoom_with_anchor(percent, 0.5, 0.5, false, ChartZoomMode::YOnly)
    }

    pub fn clear_zoom(&mut self) -> bool {
        self.set_explicit_viewport(None)
    }

    pub fn zoom_in_at_position(
        &mut self,
        column: u16,
        row: u16,
        percent: f64,
        mode: ChartZoomMode,
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
        mode: ChartZoomMode,
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
            false,
            mode,
        )
    }

    pub fn start_drag_at_position(&mut self, column: u16, row: u16) -> bool {
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        if !point_in_rect(chart_area, column, row) || self.visible_item_count() == 0 {
            return false;
        }
        let Some(viewport) = self.effective_viewport() else {
            return false;
        };
        self.drag_state = Some(ChartDragState {
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
        let Some(drag_state) = self.drag_state.as_ref() else {
            return false;
        };
        let Some(bounds) = self.visible_data_bounds() else {
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

        self.set_viewport_from_bounds(ChartViewport {
            x_min: drag_state.viewport.x_min - x_shift,
            x_max: drag_state.viewport.x_max - x_shift,
            y_min: drag_state.viewport.y_min + y_shift,
            y_max: drag_state.viewport.y_max + y_shift,
        })
        .then_some(bounds)
        .is_some()
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

    fn pan_by(&mut self, dx_percent: f64, dy_percent: f64) -> bool {
        let Some(bounds) = self.visible_data_bounds() else {
            return false;
        };
        let Some(current) = self.effective_viewport() else {
            return false;
        };
        let x_shift = (current.x_max - current.x_min) * dx_percent / 100.0;
        let y_shift = (current.y_max - current.y_min) * dy_percent / 100.0;
        self.set_viewport_from_bounds(ChartViewport {
            x_min: current.x_min + x_shift,
            x_max: current.x_max + x_shift,
            y_min: current.y_min + y_shift,
            y_max: current.y_max + y_shift,
        })
        .then_some(bounds)
        .is_some()
    }

    pub fn pan_left(&mut self, percent: f64) -> bool {
        self.pan_by(-(percent.abs()), 0.0)
    }

    pub fn pan_right(&mut self, percent: f64) -> bool {
        self.pan_by(percent.abs(), 0.0)
    }

    pub(super) fn viewport_summary(&self) -> String {
        match self.viewport {
            Some(viewport) => format!(
                "x=[{:.4}, {:.4}] y=[{:.4}, {:.4}]",
                viewport.x_min, viewport.x_max, viewport.y_min, viewport.y_max
            ),
            None => "auto-fit visible".to_string(),
        }
    }
}
