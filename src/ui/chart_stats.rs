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

    let mut value_min = f64::INFINITY;
    let mut value_max = f64::NEG_INFINITY;
    for value in &values {
        value_min = value_min.min(*value);
        value_max = value_max.max(*value);
    }
    let (value_min, value_max) = match scale {
        ChartAxisScale::Logarithmic => normalized_log_axis_bounds(value_min, value_max)?,
        ChartAxisScale::Linear | ChartAxisScale::SymLog => {
            normalized_axis_bounds(value_min, value_max)?
        }
    };
    let bin_count = match values.len() {
        0 => return None,
        1..=4 => values.len(),
        n => ((n as f64).sqrt().round() as usize).clamp(6, 64),
    };
    let (transformed_min, transformed_max) = if scale == ChartAxisScale::SymLog {
        (symlog(value_min), symlog(value_max))
    } else if scale == ChartAxisScale::Logarithmic {
        (value_min.ln(), value_max.ln())
    } else {
        (value_min, value_max)
    };
    let bin_width = (transformed_max - transformed_min) / bin_count as f64;
    let mut counts = vec![0usize; bin_count];
    for value in values {
        let transformed = if scale == ChartAxisScale::SymLog {
            symlog(value)
        } else if scale == ChartAxisScale::Logarithmic {
            value.ln()
        } else {
            value
        };
        let normalized = ((transformed - transformed_min) / bin_width).floor();
        let index = normalized
            .max(0.0)
            .min((bin_count.saturating_sub(1)) as f64) as usize;
        counts[index] = counts[index].saturating_add(1);
    }
    let count_max = counts.iter().copied().max().unwrap_or_default() as f64;
    let bins = counts
        .into_iter()
        .enumerate()
        .map(|(index, count)| {
            let raw = |value: f64| match scale {
                ChartAxisScale::SymLog => symlog_inverse(value),
                ChartAxisScale::Logarithmic => value.exp(),
                ChartAxisScale::Linear => value,
            };
            let start = raw(transformed_min + bin_width * index as f64);
            let end = if index + 1 == bin_count {
                value_max
            } else {
                raw(transformed_min + bin_width * (index + 1) as f64)
            };
            HistogramBinSummary {
                start,
                end,
                count: count as f64,
            }
        })
        .collect();
    Some(HistogramSummary {
        value_min,
        value_max,
        count_max: count_max.max(1.0),
        bin_count,
        bins,
    })
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
    let (transformed_min, transformed_max) = match scale {
        ChartAxisScale::SymLog => (symlog(value_min), symlog(value_max)),
        ChartAxisScale::Logarithmic if value_min > 0.0 => (value_min.ln(), value_max.ln()),
        ChartAxisScale::Logarithmic => return None,
        ChartAxisScale::Linear => (value_min, value_max),
    };
    let width = (transformed_max - transformed_min) / bin_count as f64;
    let mut counts = vec![0usize; bin_count];
    for &value in values.iter().filter(|value| value.is_finite()) {
        let transformed = match scale {
            ChartAxisScale::SymLog => symlog(value),
            ChartAxisScale::Logarithmic => {
                if value <= 0.0 {
                    continue;
                }
                value.ln()
            }
            ChartAxisScale::Linear => value,
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
    use super::{box_plot_summary, histogram_summary, histogram_summary_with_scale};
    use crate::ui::mchart::ChartAxisScale;

    #[test]
    fn histogram_summary_creates_bins_for_finite_values() {
        let summary = histogram_summary(&[1.0, 2.0, 3.0, 4.0]).expect("summary");
        assert_eq!(summary.bin_count, 4);
        assert_eq!(summary.bins.len(), 4);
        assert!(summary.count_max >= 1.0);
    }

    #[test]
    fn logarithmic_histogram_has_log_spaced_raw_edges() {
        let summary =
            histogram_summary_with_scale(&[1.0, 10.0, 100.0, 1_000.0], ChartAxisScale::Logarithmic)
                .expect("summary");
        assert!((summary.bins[1].start / summary.bins[0].start - 10.0_f64.powf(0.75)).abs() < 1e-9);
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
