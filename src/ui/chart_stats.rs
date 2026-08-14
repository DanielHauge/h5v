use crate::ui::{
    chart_math::{
        normalized_axis_bounds, normalized_log_axis_bounds, padded_axis_bounds, symlog,
        symlog_inverse,
    },
    mchart::ChartAxisScale,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HistogramBinSummary {
    pub start: f64,
    pub end: f64,
    pub count: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HistogramSummary {
    pub value_min: f64,
    pub value_max: f64,
    pub count_max: f64,
    pub bin_count: usize,
    pub bins: Vec<HistogramBinSummary>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BoxPlotSummary {
    pub value_min: f64,
    pub value_max: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub whisker_low: f64,
    pub whisker_high: f64,
    pub outliers: Vec<f64>,
}

pub(crate) fn quantile_sorted(values: &[f64], quantile: f64) -> f64 {
    if values.len() == 1 {
        return values[0];
    }
    let position = quantile.clamp(0.0, 1.0) * (values.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        values[lower]
    } else {
        let weight = position - lower as f64;
        values[lower] * (1.0 - weight) + values[upper] * weight
    }
}

pub(crate) fn histogram_summary(values: &[f64]) -> Option<HistogramSummary> {
    histogram_summary_with_scale(values, ChartAxisScale::Linear)
}

pub(crate) fn histogram_summary_with_scale(
    values: &[f64],
    scale: ChartAxisScale,
) -> Option<HistogramSummary> {
    let values = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if values.is_empty() {
        return None;
    }

    let (value_min, value_max) = histogram_bounds(&values, scale)?;
    let bin_count = histogram_bin_count(&values, scale)?;
    let bins = histogram_bins(&values, value_min, value_max, bin_count, scale)?;
    let count_max = bins.iter().map(|bin| bin.count).fold(0.0, f64::max);
    Some(HistogramSummary {
        value_min,
        value_max,
        count_max: count_max.max(1.0),
        bin_count,
        bins,
    })
}

/// Returns data-derived bounds for histogram edges. Degenerate data is expanded once so bins have
/// a usable width; non-degenerate endpoints are the actual finite data extrema.
pub(crate) fn histogram_bounds(values: &[f64], scale: ChartAxisScale) -> Option<(f64, f64)> {
    let value_min = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .reduce(f64::min)?;
    let value_max = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .reduce(f64::max)?;
    match scale {
        ChartAxisScale::Logarithmic => normalized_log_axis_bounds(value_min, value_max),
        ChartAxisScale::Linear | ChartAxisScale::SymLog => {
            normalized_axis_bounds(value_min, value_max)
        }
    }
}

pub(crate) fn histogram_bin_count(values: &[f64], scale: ChartAxisScale) -> Option<usize> {
    let mut transformed = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .map(|value| transform_histogram_value(value, scale))
        .collect::<Option<Vec<_>>>()?;
    let count = transformed.len();
    if count == 0 {
        return None;
    }
    if count <= 4 {
        return Some(count);
    }
    transformed.sort_by(f64::total_cmp);
    let span = transformed.last()? - transformed[0];
    let iqr = quantile_sorted(&transformed, 0.75) - quantile_sorted(&transformed, 0.25);
    let width = 2.0 * iqr / (count as f64).cbrt();
    let fallback = (count as f64).sqrt().round() as usize;
    let bins = if span.is_finite() && width.is_finite() && width > 0.0 {
        (span / width).ceil() as usize
    } else {
        fallback
    };
    Some(bins.clamp(6, 64))
}

fn transform_histogram_value(value: f64, scale: ChartAxisScale) -> Option<f64> {
    let transformed = match scale {
        ChartAxisScale::Linear => value,
        ChartAxisScale::SymLog => symlog(value),
        ChartAxisScale::Logarithmic if value > 0.0 => value.ln(),
        ChartAxisScale::Logarithmic => return None,
    };
    transformed.is_finite().then_some(transformed)
}

pub(crate) fn histogram_bins(
    values: &[f64],
    value_min: f64,
    value_max: f64,
    bin_count: usize,
    scale: ChartAxisScale,
) -> Option<Vec<HistogramBinSummary>> {
    if bin_count == 0 || !value_min.is_finite() || !value_max.is_finite() || value_max <= value_min
    {
        return None;
    }
    let (transformed_min, transformed_max) = (
        transform_histogram_value(value_min, scale)?,
        transform_histogram_value(value_max, scale)?,
    );
    let width = (transformed_max - transformed_min) / bin_count as f64;
    if !width.is_finite() || width <= 0.0 {
        return None;
    }
    let mut counts = vec![0usize; bin_count];
    for &value in values.iter().filter(|value| value.is_finite()) {
        let Some(transformed) = transform_histogram_value(value, scale) else {
            continue;
        };
        let index = ((transformed - transformed_min) / width)
            .floor()
            .clamp(0.0, bin_count.saturating_sub(1) as f64) as usize;
        counts[index] = counts[index].saturating_add(1);
    }
    let raw = |value: f64| match scale {
        ChartAxisScale::SymLog => symlog_inverse(value),
        ChartAxisScale::Logarithmic => value.exp(),
        ChartAxisScale::Linear => value,
    };
    Some(
        counts
            .into_iter()
            .enumerate()
            .map(|(index, count)| HistogramBinSummary {
                start: raw(transformed_min + width * index as f64),
                end: if index + 1 == bin_count {
                    value_max
                } else {
                    raw(transformed_min + width * (index + 1) as f64)
                },
                count: count as f64,
            })
            .collect(),
    )
}

pub(crate) fn box_plot_summary(values: &[f64]) -> Option<BoxPlotSummary> {
    let mut values = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if values.is_empty() {
        return None;
    }
    values.sort_by(|left, right| left.total_cmp(right));
    let q1 = quantile_sorted(&values, 0.25);
    let median = quantile_sorted(&values, 0.5);
    let q3 = quantile_sorted(&values, 0.75);
    let iqr = q3 - q1;
    let fence_low = q1 - 1.5 * iqr;
    let fence_high = q3 + 1.5 * iqr;
    let whisker_low = values
        .iter()
        .copied()
        .find(|value| *value >= fence_low)
        .unwrap_or(values[0]);
    let whisker_high = values
        .iter()
        .copied()
        .rev()
        .find(|value| *value <= fence_high)
        .unwrap_or(*values.last()?);
    let outliers = values
        .iter()
        .copied()
        .filter(|value| *value < whisker_low || *value > whisker_high)
        .collect::<Vec<_>>();
    let (value_min, value_max) = padded_axis_bounds(*values.first()?, *values.last()?)?;
    Some(BoxPlotSummary {
        value_min,
        value_max,
        q1,
        median,
        q3,
        whisker_low,
        whisker_high,
        outliers,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        box_plot_summary, histogram_bin_count, histogram_summary, histogram_summary_with_scale,
    };
    use crate::ui::mchart::ChartAxisScale;

    #[test]
    fn histogram_summary_creates_bins_for_finite_values() {
        let summary = histogram_summary(&[1.0, 2.0, 3.0, 4.0]).expect("summary");
        assert_eq!(summary.bin_count, 4);
        assert_eq!(summary.bins.len(), 4);
        assert!(summary.count_max >= 1.0);
    }

    #[test]
    fn histogram_edges_match_non_degenerate_data_extents() {
        let summary = histogram_summary(&[2.0, 3.0, 8.0, f64::NAN]).expect("summary");
        assert_eq!(summary.value_min, 2.0);
        assert_eq!(summary.value_max, 8.0);
        assert_eq!(summary.bins.first().expect("first bin").start, 2.0);
        assert_eq!(summary.bins.last().expect("last bin").end, 8.0);
    }

    #[test]
    fn histogram_uses_freedman_diaconis_bin_count_in_transformed_space() {
        let values = (0..100)
            .map(|value| 10.0_f64.powi(value))
            .collect::<Vec<_>>();
        assert_eq!(
            histogram_bin_count(&values, ChartAxisScale::Logarithmic),
            Some(6)
        );
        assert_eq!(
            histogram_bin_count(&values, ChartAxisScale::Linear),
            Some(64)
        );
    }

    #[test]
    fn logarithmic_histogram_has_log_spaced_raw_edges() {
        let summary =
            histogram_summary_with_scale(&[1.0, 10.0, 100.0, 1_000.0], ChartAxisScale::Logarithmic)
                .expect("summary");
        assert!((summary.bins[1].start / summary.bins[0].start - 10.0_f64.powf(0.75)).abs() < 1e-9);
    }

    #[test]
    fn histogram_bin_counts_sum_to_finite_input_count() {
        let summary = histogram_summary(&[1.0, 2.0, f64::NAN, 4.0]).expect("summary");
        assert_eq!(summary.bins.iter().map(|bin| bin.count).sum::<f64>(), 3.0);
    }

    #[test]
    fn symlog_histogram_keeps_zero_and_signed_values() {
        let summary = histogram_summary_with_scale(&[-100.0, 0.0, 100.0], ChartAxisScale::SymLog)
            .expect("summary");
        assert!(summary
            .bins
            .iter()
            .any(|bin| bin.start <= 0.0 && bin.end >= 0.0));
    }

    #[test]
    fn logarithmic_histogram_rejects_zero_or_mixed_values() {
        assert!(histogram_summary_with_scale(&[0.0, 1.0], ChartAxisScale::Logarithmic).is_none());
        assert!(histogram_summary_with_scale(&[-1.0, 1.0], ChartAxisScale::Logarithmic).is_none());
    }

    #[test]
    fn box_plot_summary_extracts_quartiles() {
        let summary = box_plot_summary(&[1.0, 2.0, 3.0, 4.0, 10.0]).expect("summary");
        assert!(summary.q1 <= summary.median);
        assert!(summary.median <= summary.q3);
        assert!(summary.value_min < 1.0);
        assert!(summary.value_max > 10.0);
    }
}
