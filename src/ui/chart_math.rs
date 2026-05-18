use ratatui::layout::Rect;

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
        zoom_axis_range,
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
}
