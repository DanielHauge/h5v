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

#[cfg(test)]
mod tests {
    use super::{normalized_axis_bounds, padded_axis_bounds};

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
}
