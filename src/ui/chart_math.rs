use ratatui::layout::Rect;

use crate::configure::{self, AxisNumberFormat};

pub(crate) fn format_axis_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-∞"
        } else {
            "∞"
        }
        .to_string();
    }
    let value = if value == 0.0 { 0.0 } else { value };
    if value == 0.0 {
        return "0".to_string();
    }
    let settings = configure::current_chart_settings();
    let scientific = match settings.axis_numbers {
        AxisNumberFormat::Exact => false,
        AxisNumberFormat::Scientific => true,
        AxisNumberFormat::Auto => {
            let exponent = value.abs().log10().floor() as i32;
            value != 0.0
                && (exponent <= settings.scientific_lower_exponent
                    || exponent >= settings.scientific_upper_exponent)
        }
    };
    let output = if scientific {
        format!("{value:.4e}")
    } else {
        format!("{value:.12}")
    };
    trim_number(&output)
}

pub(crate) fn axis_label_area_size(values: &[f64], padding: u32) -> u32 {
    values
        .iter()
        .map(|value| format_axis_number(*value).len() as u32 * 3 + padding)
        .max()
        .unwrap_or(padding)
}

/// Space for horizontal tick labels, an axis title, and a small gap between them.
pub(crate) fn axis_title_label_area_size(font_size: u32) -> u32 {
    font_size.saturating_mul(2).saturating_add(8)
}

fn trim_number(value: &str) -> String {
    let (mantissa, exponent) = value.split_once('e').unwrap_or((value, ""));
    let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
    if exponent.is_empty() {
        mantissa.to_string()
    } else {
        format!("{mantissa}e{}", exponent.parse::<i32>().unwrap_or(0))
    }
}

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

pub(crate) fn normalized_log_axis_bounds(min: f64, max: f64) -> Option<(f64, f64)> {
    if !min.is_finite() || !max.is_finite() || min <= 0.0 || max < min {
        return None;
    }
    if min == max {
        return Some((min / 10.0, max * 10.0));
    }
    Some((min, max))
}

/// A continuous signed logarithm with a linear neighbourhood around zero.
pub(crate) fn symlog(value: f64) -> f64 {
    value.signum() * value.abs().ln_1p()
}

pub(crate) fn symlog_inverse(value: f64) -> f64 {
    value.signum() * value.abs().exp_m1()
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
        axis_label_area_size, axis_title_label_area_size, clamp_axis_range, format_axis_number,
        normalized_axis_bounds, normalized_log_axis_bounds, padded_axis_bounds, point_in_rect,
        raster_chart_layout, symlog, symlog_inverse, zoom_axis_range, RasterChartLayoutHints,
    };

    #[test]
    fn normalizes_degenerate_axis_bounds() {
        assert_eq!(normalized_axis_bounds(0.0, 0.0), Some((-1.0, 1.0)));
        assert_eq!(normalized_axis_bounds(10.0, 10.0), Some((9.5, 10.5)));
    }

    #[test]
    fn normalizes_positive_log_degenerate_bounds_multiplicatively() {
        assert_eq!(normalized_log_axis_bounds(10.0, 10.0), Some((1.0, 100.0)));
        assert_eq!(normalized_log_axis_bounds(0.0, 1.0), None);
    }

    #[test]
    fn symlog_round_trips_signed_values() {
        for value in [-1_000.0, -1.0, 0.0, 1.0, 1_000.0] {
            assert!((symlog_inverse(symlog(value)) - value).abs() < 1e-10);
        }
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

    #[test]
    fn axis_title_area_separates_title_from_tick_labels() {
        assert_eq!(axis_title_label_area_size(18), 44);
    }

    #[test]
    fn formats_axis_numbers_by_mode_and_boundary() {
        let snapshot = crate::configure::snapshot_config();
        let mut settings = crate::configure::ChartSettings::default();
        settings.axis_numbers = crate::configure::AxisNumberFormat::Auto;
        crate::configure::set_chart_settings(&settings);
        assert_eq!(format_axis_number(1e-3), "1e-3");
        assert_eq!(format_axis_number(1e-2), "0.01");
        assert_eq!(format_axis_number(1e5), "1e5");
        settings.axis_numbers = crate::configure::AxisNumberFormat::Exact;
        crate::configure::set_chart_settings(&settings);
        assert_eq!(format_axis_number(1e5), "100000");
        settings.axis_numbers = crate::configure::AxisNumberFormat::Scientific;
        crate::configure::set_chart_settings(&settings);
        assert_eq!(format_axis_number(-0.0), "0");
        assert_eq!(format_axis_number(f64::INFINITY), "∞");
        assert!(axis_label_area_size(&[1e5], 30) > 30);
        crate::configure::restore_config(snapshot);
    }
}
