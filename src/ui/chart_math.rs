use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RasterChartLayout {
    pub margin: u32,
    pub x_label_area_size: u32,
    pub y_label_area_size: u32,
    pub x_label_font_size: u32,
    pub y_label_font_size: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RasterChartLayoutHints {
    pub preferred_margin: u32,
    pub preferred_x_label_area_size: u32,
    pub preferred_y_label_area_size: u32,
    pub preferred_x_label_font_size: u32,
    pub preferred_y_label_font_size: u32,
    pub min_plot_width: u32,
    pub min_plot_height: u32,
}

fn clamp_reserved_area(total: u32, preferred: u32, min_plot: u32) -> u32 {
    let max_reserved = total.saturating_sub(min_plot);
    preferred.min(max_reserved)
}

pub(crate) fn raster_chart_layout(
    width_px: u32,
    height_px: u32,
    hints: RasterChartLayoutHints,
) -> RasterChartLayout {
    let margin = hints
        .preferred_margin
        .min(width_px / 12)
        .min(height_px / 12);
    let horizontal_margin = margin.saturating_mul(2);
    let vertical_margin = margin.saturating_mul(2);
    let y_label_area_size = clamp_reserved_area(
        width_px.saturating_sub(horizontal_margin),
        hints.preferred_y_label_area_size,
        hints.min_plot_width,
    );
    let x_label_area_size = clamp_reserved_area(
        height_px.saturating_sub(vertical_margin),
        hints.preferred_x_label_area_size,
        hints.min_plot_height,
    );

    let available_label_width = width_px.saturating_sub(horizontal_margin + hints.min_plot_width);
    let x_label_font_cap = x_label_area_size.saturating_sub(6);
    let y_label_font_cap = (available_label_width / 3).min(y_label_area_size.saturating_sub(6));
    let x_label_font_size = hints
        .preferred_x_label_font_size
        .min(x_label_font_cap)
        .max(x_label_font_cap.min(10));
    let y_label_font_size = hints
        .preferred_y_label_font_size
        .min(y_label_font_cap)
        .max(y_label_font_cap.min(10));

    RasterChartLayout {
        margin,
        x_label_area_size,
        y_label_area_size,
        x_label_font_size,
        y_label_font_size,
    }
}

pub(crate) fn point_in_rect(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

pub(crate) fn normalized_axis_bounds(min: f64, max: f64) -> Option<(f64, f64)> {
    if !min.is_finite() || !max.is_finite() || max < min {
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

pub(crate) fn padded_axis_bounds(min: f64, max: f64) -> Option<(f64, f64)> {
    let (min, max) = normalized_axis_bounds(min, max)?;
    let pad = (max - min).abs().max(1.0) * 0.05;
    Some((min - pad, max + pad))
}

fn minimum_zoom_span(bounds_min: f64, bounds_max: f64) -> f64 {
    let span = (bounds_max - bounds_min).abs();
    span.mul_add(1e-6, f64::EPSILON).max(1e-9)
}

pub(crate) fn clamp_axis_range(
    mut start: f64,
    mut end: f64,
    bounds_min: f64,
    bounds_max: f64,
) -> (f64, f64) {
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

pub(crate) fn zoom_axis_range(
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

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::{
        clamp_axis_range, normalized_axis_bounds, padded_axis_bounds, point_in_rect,
        raster_chart_layout, zoom_axis_range, RasterChartLayoutHints,
    };

    #[test]
    fn normalizes_degenerate_axis_bounds() {
        assert_eq!(normalized_axis_bounds(0.0, 0.0), Some((-1.0, 1.0)));
        assert_eq!(normalized_axis_bounds(10.0, 10.0), Some((9.5, 10.5)));
    }

    #[test]
    fn rejects_invalid_axis_bounds() {
        assert_eq!(normalized_axis_bounds(f64::NAN, 1.0), None);
        assert_eq!(normalized_axis_bounds(2.0, 1.0), None);
    }

    #[test]
    fn pads_normalized_bounds() {
        assert_eq!(padded_axis_bounds(0.0, 10.0), Some((-0.5, 10.5)));
    }

    #[test]
    fn detects_rect_hits() {
        let rect = Rect::new(10, 20, 4, 3);
        assert!(point_in_rect(rect, 10, 20));
        assert!(point_in_rect(rect, 13, 22));
        assert!(!point_in_rect(rect, 14, 22));
        assert!(!point_in_rect(rect, 13, 23));
    }

    #[test]
    fn clamps_axis_range_inside_bounds() {
        assert_eq!(clamp_axis_range(-5.0, 5.0, 0.0, 10.0), (0.0, 10.0));
        assert_eq!(clamp_axis_range(8.0, 12.0, 0.0, 10.0), (6.0, 10.0));
    }

    #[test]
    fn zooms_axis_range_around_anchor() {
        let zoomed = zoom_axis_range(0.0, 10.0, 0.0, 10.0, 0.5, 10.0, true);
        assert!(zoomed.0 > 0.0);
        assert!(zoomed.1 < 10.0);
        assert!(zoomed.0 < 5.0 && zoomed.1 > 5.0);
    }

    #[test]
    fn raster_chart_layout_preserves_plot_space_in_small_viewports() {
        let layout = raster_chart_layout(
            80,
            64,
            RasterChartLayoutHints {
                preferred_margin: 10,
                preferred_x_label_area_size: 30,
                preferred_y_label_area_size: 48,
                preferred_x_label_font_size: 18,
                preferred_y_label_font_size: 18,
                min_plot_width: 48,
                min_plot_height: 40,
            },
        );
        assert!(layout.margin <= 5);
        assert!(layout.x_label_area_size <= 24);
        assert!(layout.y_label_area_size <= 32);
        assert!(layout.x_label_area_size + layout.margin * 2 + 40 <= 64);
        assert!(layout.y_label_area_size + layout.margin * 2 + 48 <= 80);
    }
}
