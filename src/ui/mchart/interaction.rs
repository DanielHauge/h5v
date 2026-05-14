use ratatui::layout::Rect;

use super::{ChartDragState, MultiChartState};

impl MultiChartState {
    pub(super) fn global_x_bounds(&self) -> Option<(usize, usize)> {
        let mut visible = self.items.iter().filter(|item| item.visible);
        let first = visible.next()?;
        let mut min_x = first.series.sample_min;
        let mut max_x = first.series.sample_max;
        for item in visible {
            min_x = min_x.min(item.series.sample_min);
            max_x = max_x.max(item.series.sample_max);
        }
        Some((min_x, max_x))
    }

    fn current_sample_bounds(&self) -> Option<(usize, usize, usize, usize)> {
        let (global_min, global_max) = self.global_x_bounds()?;
        let actual_min = global_min.max(self.aoi_from.unwrap_or(global_min));
        let actual_max = global_max.min(self.aoi_to.unwrap_or(global_max));
        (actual_min < actual_max).then_some((global_min, global_max, actual_min, actual_max))
    }

    fn set_viewport_bounds(
        &mut self,
        global_min: usize,
        global_max: usize,
        from: usize,
        to: usize,
    ) {
        let next_from = (from > global_min).then_some(from);
        let next_to = (to < global_max).then_some(to);
        if self.aoi_from == next_from && self.aoi_to == next_to {
            return;
        }
        self.aoi_from = next_from;
        self.aoi_to = next_to;
        self.modified = true;
    }

    fn shift_viewport_by_samples(
        &mut self,
        global_min: usize,
        global_max: usize,
        from: usize,
        to: usize,
        delta: isize,
    ) {
        let range = to.saturating_sub(from);
        if range == 0 {
            return;
        }

        let mut next_from = from as isize - delta;
        let mut next_to = to as isize - delta;
        let global_min = global_min as isize;
        let global_max = global_max as isize;
        if next_from < global_min {
            next_to += global_min - next_from;
            next_from = global_min;
        }
        if next_to > global_max {
            let overflow = next_to - global_max;
            next_from -= overflow;
            next_to = global_max;
        }
        next_from = next_from.max(global_min);
        next_to = next_to.min(global_max);
        if next_to <= next_from {
            return;
        }

        self.set_viewport_bounds(
            global_min as usize,
            global_max as usize,
            next_from as usize,
            next_to as usize,
        );
    }

    pub(super) fn zoom_with_anchor_ratio(
        &mut self,
        percent: f64,
        anchor_ratio: f64,
        zoom_in: bool,
    ) {
        let Some((global_min, global_max, actual_min, actual_max)) = self.current_sample_bounds()
        else {
            return;
        };
        if !zoom_in && self.aoi_from.is_none() && self.aoi_to.is_none() {
            return;
        }

        let range = actual_max.saturating_sub(actual_min);
        if range <= 1 && zoom_in {
            return;
        }
        if range == 0 {
            return;
        }

        let anchor_ratio = anchor_ratio.clamp(0.0, 1.0);
        let delta = range as f64 * percent / 100.0;
        let new_range = if zoom_in {
            (range as f64 - 2.0 * delta).max(1.0)
        } else {
            (range as f64 + 2.0 * delta).min((global_max - global_min) as f64)
        };
        let anchor = actual_min as f64 + range as f64 * anchor_ratio;
        let mut new_from = anchor - new_range * anchor_ratio;
        let mut new_to = new_from + new_range;
        let global_min_f = global_min as f64;
        let global_max_f = global_max as f64;

        if new_from < global_min_f {
            new_to += global_min_f - new_from;
            new_from = global_min_f;
        }
        if new_to > global_max_f {
            let overflow = new_to - global_max_f;
            new_from -= overflow;
            new_to = global_max_f;
        }
        new_from = new_from.max(global_min_f);
        new_to = new_to.min(global_max_f);

        if global_max <= global_min {
            return;
        }

        let mut from = (new_from.round() as usize).clamp(global_min, global_max.saturating_sub(1));
        let mut to = (new_to.round() as usize).clamp(from.saturating_add(1), global_max);
        if to <= from {
            if global_max.saturating_sub(global_min) <= 1 {
                return;
            }
            from = from.min(global_max - 1);
            to = global_max;
        }

        self.set_viewport_bounds(global_min, global_max, from, to);
    }

    pub fn zoom_in(&mut self, percent: f64) {
        self.zoom_with_anchor_ratio(percent, 0.5, true);
    }

    pub fn clear_zoom(&mut self) {
        self.aoi_from = None;
        self.aoi_to = None;
        self.modified = true;
    }

    pub fn zoom_out(&mut self, percent: f64) {
        self.zoom_with_anchor_ratio(percent, 0.5, false);
    }

    pub fn zoom_in_at_position(&mut self, column: u16, row: u16, percent: f64) -> bool {
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        if !point_in_rect(chart_area, column, row) {
            return false;
        }
        let relative_x = column.saturating_sub(chart_area.x) as f64;
        let denom = chart_area.width.saturating_sub(1).max(1) as f64;
        let before = (self.aoi_from, self.aoi_to);
        self.zoom_with_anchor_ratio(percent, relative_x / denom, true);
        (self.aoi_from, self.aoi_to) != before
    }

    pub fn zoom_out_at_position(&mut self, column: u16, row: u16, percent: f64) -> bool {
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        if !point_in_rect(chart_area, column, row) {
            return false;
        }
        let relative_x = column.saturating_sub(chart_area.x) as f64;
        let denom = chart_area.width.saturating_sub(1).max(1) as f64;
        let before = (self.aoi_from, self.aoi_to);
        self.zoom_with_anchor_ratio(percent, relative_x / denom, false);
        (self.aoi_from, self.aoi_to) != before
    }

    pub fn start_drag_at_position(&mut self, column: u16, row: u16) -> bool {
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        if !point_in_rect(chart_area, column, row) || self.visible_item_count() == 0 {
            return false;
        }
        let Some((_, _, viewport_from, viewport_to)) = self.current_sample_bounds() else {
            return false;
        };
        self.drag_state = Some(ChartDragState {
            anchor_column: column,
            viewport_from,
            viewport_to,
        });
        true
    }

    fn apply_drag_position(&mut self, column: u16) -> bool {
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        let Some(drag_state) = self.drag_state.as_ref() else {
            return false;
        };
        let Some((global_min, global_max, _, _)) = self.current_sample_bounds() else {
            return false;
        };
        if chart_area.width <= 1 {
            return false;
        }
        let viewport_range = drag_state
            .viewport_to
            .saturating_sub(drag_state.viewport_from);
        if viewport_range == 0 {
            return false;
        }

        let delta_columns = column as i32 - drag_state.anchor_column as i32;
        let sample_delta = ((delta_columns as f64 / chart_area.width.saturating_sub(1) as f64)
            * viewport_range as f64)
            .round() as isize;
        let before = (self.aoi_from, self.aoi_to);
        self.shift_viewport_by_samples(
            global_min,
            global_max,
            drag_state.viewport_from,
            drag_state.viewport_to,
            sample_delta,
        );
        (self.aoi_from, self.aoi_to) != before
    }

    pub fn drag_to_position(&mut self, column: u16) -> bool {
        let Some(_drag_state) = self.drag_state.as_mut() else {
            return false;
        };
        let _ = column;
        false
    }

    pub fn finish_drag_at_position(&mut self, column: u16) -> bool {
        let changed = self.apply_drag_position(column);
        self.drag_state = None;
        changed
    }

    pub fn end_drag(&mut self) {
        self.drag_state = None;
    }

    pub fn pan_left(&mut self, percent: f64) {
        let Some((min_x, max_x)) = self.global_x_bounds() else {
            return;
        };
        if self.aoi_from.is_none() && self.aoi_to.is_none() {
            return;
        }
        let actual_min = self.aoi_from.unwrap_or(min_x).max(min_x);
        let actual_max = self.aoi_to.unwrap_or(max_x).min(max_x);
        let range = actual_max.saturating_sub(actual_min);
        if range == 0 {
            return;
        }
        let delta = (range as f64 * percent / 100.0).round() as usize;
        let new_min = actual_min.saturating_sub(delta);
        let new_max = actual_max.saturating_sub(delta);
        self.aoi_from = (new_min > min_x).then_some(new_min);
        self.aoi_to = (new_max < max_x).then_some(new_max);
        self.modified = true;
    }

    pub fn pan_right(&mut self, percent: f64) {
        let Some((min_x, max_x)) = self.global_x_bounds() else {
            return;
        };
        if self.aoi_from.is_none() && self.aoi_to.is_none() {
            return;
        }
        let actual_min = self.aoi_from.unwrap_or(min_x).max(min_x);
        let actual_max = self.aoi_to.unwrap_or(max_x).min(max_x);
        let range = actual_max.saturating_sub(actual_min);
        if range == 0 {
            return;
        }
        let delta = (range as f64 * percent / 100.0).round() as usize;
        let new_min = actual_min.saturating_add(delta);
        let new_max = actual_max.saturating_add(delta);
        self.aoi_from = (new_min > min_x).then_some(new_min);
        self.aoi_to = (new_max < max_x).then_some(new_max);
        self.modified = true;
    }
}

fn point_in_rect(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}
