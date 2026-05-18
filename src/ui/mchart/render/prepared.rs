use crate::ui::chart_math::{normalized_axis_bounds, padded_axis_bounds};

use super::super::{
    model::sanitize_chart_points, ChartItem, MultiChartState, MultiChartViewMode, Point,
    PreparedBoxPlotData, PreparedBoxPlotSeries, PreparedChartData, PreparedComparisonScatterData,
    PreparedHistogramBin, PreparedHistogramData, PreparedHistogramSeries, PreparedLineChartData,
    PreparedLineChartSeries,
};

fn quantile_sorted(values: &[f64], quantile: f64) -> f64 {
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

impl MultiChartState {
    pub(super) fn item_display_label(&self, item: &ChartItem) -> String {
        item.name
            .as_ref()
            .cloned()
            .unwrap_or_else(|| item.label.clone())
    }

    fn sample_window(&self) -> Option<(f64, f64)> {
        self.effective_viewport()
            .map(|viewport| (viewport.x_min, viewport.x_max))
    }

    fn windowed_visible_points(&self, item: &ChartItem) -> Vec<Point> {
        let points = item.active_series().points.iter().copied();
        match self.sample_window() {
            Some((x_min, x_max)) => sanitize_chart_points(
                points
                    .filter(|(x, _)| *x >= x_min && *x <= x_max)
                    .collect::<Vec<_>>(),
            ),
            None => sanitize_chart_points(points.collect::<Vec<_>>()),
        }
    }

    pub(super) fn comparison_scatter_pair(&self) -> Option<(&ChartItem, &ChartItem)> {
        let visible_items = self
            .items
            .iter()
            .filter(|item| item.visible && item.has_loaded_series())
            .collect::<Vec<_>>();
        if visible_items.len() < 2 {
            return None;
        }
        if let Some(selected) = self
            .selected_item()
            .filter(|item| item.visible && item.has_loaded_series())
        {
            if let Some(selected_index) =
                visible_items.iter().position(|item| item.id == selected.id)
            {
                if let Some(other) = visible_items
                    .iter()
                    .skip(selected_index + 1)
                    .find(|item| item.id != selected.id)
                {
                    return Some((selected, *other));
                }
                if let Some(other) = visible_items.iter().find(|item| item.id != selected.id) {
                    return Some((selected, *other));
                }
            }
        }
        Some((visible_items[0], visible_items[1]))
    }

    pub(super) fn comparison_scatter_pair_summary(&self) -> Option<String> {
        let (left, right) = self.comparison_scatter_pair()?;
        Some(format!(
            "{} vs {}",
            self.item_display_label(left),
            self.item_display_label(right)
        ))
    }

    pub(super) fn comparison_scatter_truncation_note(&self) -> Option<String> {
        self.prepared_comparison_scatter_data()
            .and_then(|prepared| prepared.truncation_note)
    }

    pub(super) fn mode_window_summary(&self) -> String {
        match (self.view_mode(), self.viewport) {
            (mode, _) if matches!(mode, MultiChartViewMode::Line) => {
                format!(
                    "{} {}",
                    mode.sample_window_description(),
                    self.viewport_summary()
                )
            }
            (mode, Some(viewport)) => format!(
                "{} x=[{:.4}, {:.4}]",
                mode.sample_window_description(),
                viewport.x_min,
                viewport.x_max
            ),
            (mode, None) => format!("{} auto-fit visible", mode.sample_window_description()),
        }
    }

    fn prepared_line_chart_data(&self) -> Option<PreparedLineChartData> {
        let visible_items = self
            .items
            .iter()
            .filter(|item| item.visible && item.has_loaded_series())
            .collect::<Vec<_>>();
        if visible_items.is_empty() {
            return None;
        }
        let selected_item_id = self.selected_item().map(|item| item.id);
        let mut plot_x_min = f64::MAX;
        let mut plot_x_max = f64::MIN;
        let mut series = Vec::new();

        for item in visible_items {
            let points = self.windowed_visible_points(item);
            if points.is_empty() {
                continue;
            }

            for &(x, _) in &points {
                plot_x_min = plot_x_min.min(x);
                plot_x_max = plot_x_max.max(x);
            }

            series.push(PreparedLineChartSeries {
                label: self.item_display_label(item),
                color_slot: item.color_slot,
                points,
                is_selected: selected_item_id == Some(item.id),
            });
        }

        if series.is_empty() {
            return None;
        }
        let (plot_x_min, plot_x_max) = if let Some(viewport) = self.viewport {
            (viewport.x_min, viewport.x_max)
        } else {
            normalized_axis_bounds(plot_x_min, plot_x_max)?
        };
        let (y_min, y_max) = if let Some(viewport) = self.viewport {
            (viewport.y_min, viewport.y_max)
        } else {
            let mut y_min = f64::MAX;
            let mut y_max = f64::MIN;
            for prepared in &series {
                for &(_, y) in &prepared.points {
                    y_min = y_min.min(y);
                    y_max = y_max.max(y);
                }
            }
            normalized_axis_bounds(y_min, y_max)?
        };

        Some(PreparedLineChartData {
            plot_x_min,
            plot_x_max,
            y_min,
            y_max,
            series,
        })
    }

    fn prepared_histogram_data(&self) -> Option<PreparedHistogramData> {
        let visible_items = self
            .items
            .iter()
            .filter(|item| item.visible && item.has_loaded_series())
            .collect::<Vec<_>>();
        if visible_items.is_empty() {
            return None;
        }

        let selected_item_id = self.selected_item().map(|item| item.id);
        let mut series_values = Vec::new();
        let mut value_min = f64::MAX;
        let mut value_max = f64::MIN;
        let mut max_samples = 0usize;

        for item in visible_items {
            let values = self
                .windowed_visible_points(item)
                .into_iter()
                .map(|(_, y)| y)
                .collect::<Vec<_>>();
            if values.is_empty() {
                continue;
            }
            max_samples = max_samples.max(values.len());
            for value in &values {
                value_min = value_min.min(*value);
                value_max = value_max.max(*value);
            }
            series_values.push((item, values));
        }
        if series_values.is_empty() {
            return None;
        }

        let (value_min, value_max) = normalized_axis_bounds(value_min, value_max)?;
        let bin_count = match max_samples {
            0 => return None,
            1..=4 => max_samples,
            n => ((n as f64).sqrt().round() as usize).clamp(6, 64),
        };
        let bin_width = (value_max - value_min) / bin_count as f64;
        let mut count_max = 0.0_f64;
        let mut series = Vec::new();

        for (item, values) in series_values {
            let mut counts = vec![0usize; bin_count];
            for value in values {
                let normalized: f64 = (value - value_min) / bin_width;
                let normalized = normalized.floor();
                let index = normalized
                    .max(0.0)
                    .min((bin_count.saturating_sub(1)) as f64) as usize;
                counts[index] = counts[index].saturating_add(1);
            }
            count_max = count_max.max(counts.iter().copied().max().unwrap_or_default() as f64);
            let bins = counts
                .into_iter()
                .enumerate()
                .map(|(index, count)| {
                    let start = value_min + bin_width * index as f64;
                    let end = if index + 1 == bin_count {
                        value_max
                    } else {
                        start + bin_width
                    };
                    PreparedHistogramBin {
                        start,
                        end,
                        count: count as f64,
                    }
                })
                .collect::<Vec<_>>();
            series.push(PreparedHistogramSeries {
                label: self.item_display_label(item),
                color_slot: item.color_slot,
                bins,
                is_selected: selected_item_id == Some(item.id),
            });
        }
        Some(PreparedHistogramData {
            value_min,
            value_max,
            count_max: count_max.max(1.0),
            bin_count,
            series,
        })
    }

    fn prepared_box_plot_data(&self) -> Option<PreparedBoxPlotData> {
        let visible_items = self
            .items
            .iter()
            .filter(|item| item.visible && item.has_loaded_series())
            .collect::<Vec<_>>();
        if visible_items.is_empty() {
            return None;
        }
        let selected_item_id = self.selected_item().map(|item| item.id);
        let mut value_min = f64::MAX;
        let mut value_max = f64::MIN;
        let mut series = Vec::new();

        for (x_index, item) in visible_items.into_iter().enumerate() {
            let mut values: Vec<f64> = self
                .windowed_visible_points(item)
                .into_iter()
                .map(|(_, y)| y)
                .filter(|value| value.is_finite())
                .collect();
            if values.is_empty() {
                continue;
            }
            values.sort_by(|left: &f64, right: &f64| left.total_cmp(right));
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
            value_min = value_min.min(*values.first()?);
            value_max = value_max.max(*values.last()?);
            series.push(PreparedBoxPlotSeries {
                label: self.item_display_label(item),
                color_slot: item.color_slot,
                x_index,
                q1,
                median,
                q3,
                whisker_low,
                whisker_high,
                outliers,
                is_selected: selected_item_id == Some(item.id),
            });
        }
        if series.is_empty() {
            return None;
        }
        let (value_min, value_max) = padded_axis_bounds(value_min, value_max)?;
        Some(PreparedBoxPlotData {
            value_min,
            value_max,
            series,
        })
    }

    pub(super) fn prepared_comparison_scatter_data(&self) -> Option<PreparedComparisonScatterData> {
        let (left, right) = self.comparison_scatter_pair()?;
        let left_points = self
            .windowed_visible_points(left)
            .into_iter()
            .collect::<Vec<_>>();
        let right_points = self
            .windowed_visible_points(right)
            .into_iter()
            .collect::<Vec<_>>();
        let left_len = left_points.len();
        let right_len = right_points.len();
        let shared_len = left_len.min(right_len);
        if shared_len == 0
            || left_points
                .iter()
                .zip(&right_points)
                .take(shared_len)
                .any(|((left_x, _), (right_x, _))| left_x != right_x)
        {
            return None;
        }
        let truncation_note = match left_len.cmp(&right_len) {
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => {
                let dropped = left_len - shared_len;
                let truncated_at = left_points.get(shared_len).map(|(x, _)| *x)?;
                Some(format!(
                    "using first {shared_len} aligned samples; {} truncated by {dropped} trailing sample{} from x={truncated_at:.4}",
                    self.item_display_label(left),
                    if dropped == 1 { "" } else { "s" }
                ))
            }
            std::cmp::Ordering::Less => {
                let dropped = right_len - shared_len;
                let truncated_at = right_points.get(shared_len).map(|(x, _)| *x)?;
                Some(format!(
                    "using first {shared_len} aligned samples; {} truncated by {dropped} trailing sample{} from x={truncated_at:.4}",
                    self.item_display_label(right),
                    if dropped == 1 { "" } else { "s" }
                ))
            }
        };
        let points = left_points
            .iter()
            .zip(&right_points)
            .take(shared_len)
            .map(|((_, x), (_, y))| (*x, *y))
            .collect::<Vec<_>>();
        let bounds = Self::bounds_from_points(points.iter())?;

        Some(PreparedComparisonScatterData {
            label: format!(
                "{} vs {}",
                self.item_display_label(left),
                self.item_display_label(right)
            ),
            x_label: self.item_display_label(left),
            y_label: self.item_display_label(right),
            color_slot: left.color_slot,
            points,
            x_min: bounds.x_min,
            x_max: bounds.x_max,
            y_min: bounds.y_min,
            y_max: bounds.y_max,
            truncation_note,
        })
    }

    pub(in crate::ui::mchart) fn prepared_chart_data(&self) -> Option<PreparedChartData> {
        match self.view_mode() {
            MultiChartViewMode::Line => {
                self.prepared_line_chart_data().map(PreparedChartData::Line)
            }
            MultiChartViewMode::Histogram => self
                .prepared_histogram_data()
                .map(PreparedChartData::Histogram),
            MultiChartViewMode::BoxPlot => self
                .prepared_box_plot_data()
                .map(PreparedChartData::BoxPlot),
            MultiChartViewMode::ComparisonScatter => self
                .prepared_comparison_scatter_data()
                .map(PreparedChartData::ComparisonScatter),
        }
    }
}
