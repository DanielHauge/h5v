use crate::ui::{
    chart_math::{normalized_axis_bounds, normalized_log_axis_bounds},
    chart_stats::{box_plot_summary, histogram_bin_count, histogram_bins, histogram_bounds},
};

use super::super::{
    model::{sanitize_chart_points, ChartAxisScale},
    ChartItem, MultiChartState, MultiChartViewMode, Point, PreparedBoxPlotData,
    PreparedBoxPlotSeries, PreparedChartData, PreparedComparisonScatterData, PreparedHistogramBin,
    PreparedHistogramData, PreparedHistogramSeries, PreparedLineChartData, PreparedLineChartSeries,
};

impl MultiChartState {
    fn effective_axis_scale(
        &self,
        requested: ChartAxisScale,
        supported: bool,
        valid: bool,
    ) -> ChartAxisScale {
        if supported && (requested == ChartAxisScale::SymLog || valid) {
            requested
        } else {
            ChartAxisScale::Linear
        }
    }
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

    pub(crate) fn windowed_visible_points(&self, item: &ChartItem) -> Vec<Point> {
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

    pub(crate) fn comparison_scatter_pair(&self) -> Option<(&ChartItem, &ChartItem)> {
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
            match self.x_axis_scale {
                ChartAxisScale::Logarithmic => {
                    normalized_log_axis_bounds(plot_x_min, plot_x_max)
                        .or_else(|| normalized_axis_bounds(plot_x_min, plot_x_max))?
                }
                ChartAxisScale::Linear | ChartAxisScale::SymLog => {
                    normalized_axis_bounds(plot_x_min, plot_x_max)?
                }
            }
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
            match self.y_axis_scale {
                ChartAxisScale::Logarithmic => normalized_log_axis_bounds(y_min, y_max)
                    .or_else(|| normalized_axis_bounds(y_min, y_max))?,
                ChartAxisScale::Linear | ChartAxisScale::SymLog => {
                    normalized_axis_bounds(y_min, y_max)?
                }
            }
        };

        Some(PreparedLineChartData {
            x_axis_scale: self.effective_axis_scale(
                self.x_axis_scale,
                self.view_mode.supports_x_log_scale(),
                series
                    .iter()
                    .flat_map(|series| &series.points)
                    .all(|(x, _)| x.is_finite() && *x > 0.0),
            ),
            y_axis_scale: self.effective_axis_scale(
                self.y_axis_scale,
                self.view_mode.supports_y_log_scale(),
                series
                    .iter()
                    .flat_map(|series| &series.points)
                    .all(|(_, y)| y.is_finite() && *y > 0.0),
            ),
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

        for item in visible_items {
            let values = self
                .windowed_visible_points(item)
                .into_iter()
                .map(|(_, y)| y)
                .collect::<Vec<_>>();
            if values.is_empty() {
                continue;
            }
            series_values.push((item, values));
        }
        if series_values.is_empty() {
            return None;
        }

        let all_values = series_values
            .iter()
            .flat_map(|(_, values)| values.iter().copied())
            .collect::<Vec<_>>();
        let (raw_value_min, _) = histogram_bounds(&all_values, ChartAxisScale::Linear)?;
        let histogram_scale = self.effective_axis_scale(
            self.x_axis_scale,
            self.view_mode.supports_x_log_scale(),
            raw_value_min > 0.0,
        );
        let (value_min, value_max) = histogram_bounds(&all_values, histogram_scale)?;
        let max_samples = series_values.iter().map(|(_, values)| values.len()).max()?;
        let bin_count = if max_samples <= 4 {
            max_samples
        } else {
            histogram_bin_count(&all_values, histogram_scale)?
        };
        let mut count_max = 0.0_f64;
        let mut series = Vec::new();

        for (item, values) in series_values {
            let bins = histogram_bins(&values, value_min, value_max, bin_count, histogram_scale)?
                .into_iter()
                .map(|bin| PreparedHistogramBin {
                    start: bin.start,
                    end: bin.end,
                    count: bin.count,
                })
                .collect::<Vec<_>>();
            count_max = count_max.max(bins.iter().map(|bin| bin.count).fold(0.0, f64::max));
            series.push(PreparedHistogramSeries {
                label: self.item_display_label(item),
                color_slot: item.color_slot,
                bins,
                is_selected: selected_item_id == Some(item.id),
            });
        }
        Some(PreparedHistogramData {
            x_axis_scale: self.effective_axis_scale(
                self.x_axis_scale,
                self.view_mode.supports_x_log_scale(),
                series.iter().flat_map(|series| &series.bins).all(|bin| {
                    bin.start.is_finite() && bin.end.is_finite() && bin.start > 0.0 && bin.end > 0.0
                }),
            ),
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
        let mut series = Vec::new();
        let mut plot_bounds = None;
        let mut mark_bounds = None;

        for (x_index, item) in visible_items.into_iter().enumerate() {
            let values: Vec<f64> = self
                .windowed_visible_points(item)
                .into_iter()
                .map(|(_, y)| y)
                .filter(|value| value.is_finite())
                .collect();
            let value_min = values.iter().copied().reduce(f64::min)?;
            let value_max = values.iter().copied().reduce(f64::max)?;
            let (overall_min, overall_max) = mark_bounds.unwrap_or((value_min, value_max));
            mark_bounds = Some((overall_min.min(value_min), overall_max.max(value_max)));
            let summary = box_plot_summary(&values)?;
            let (overall_min, overall_max) =
                plot_bounds.unwrap_or((summary.value_min, summary.value_max));
            plot_bounds = Some((
                overall_min.min(summary.value_min),
                overall_max.max(summary.value_max),
            ));
            series.push(PreparedBoxPlotSeries {
                label: self.item_display_label(item),
                color_slot: item.color_slot,
                x_index,
                q1: summary.q1,
                median: summary.median,
                q3: summary.q3,
                whisker_low: summary.whisker_low,
                whisker_high: summary.whisker_high,
                outliers: summary.outliers,
                is_selected: selected_item_id == Some(item.id),
            });
        }
        if series.is_empty() {
            return None;
        }
        let marks_are_positive = series.iter().all(|series| {
            [
                series.whisker_low,
                series.q1,
                series.median,
                series.q3,
                series.whisker_high,
            ]
            .into_iter()
            .chain(series.outliers.iter().copied())
            .all(|value| value.is_finite() && value > 0.0)
        });
        let y_axis_scale = self.effective_axis_scale(
            self.y_axis_scale,
            self.view_mode.supports_y_log_scale(),
            marks_are_positive,
        );
        let (value_min, value_max) = match y_axis_scale {
            ChartAxisScale::Logarithmic => {
                normalized_log_axis_bounds(mark_bounds?.0, mark_bounds?.1)?
            }
            ChartAxisScale::Linear | ChartAxisScale::SymLog => plot_bounds?,
        };
        Some(PreparedBoxPlotData {
            y_axis_scale,
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
            x_axis_scale: self.effective_axis_scale(
                self.x_axis_scale,
                self.view_mode.supports_x_log_scale(),
                points.iter().all(|(x, _)| x.is_finite() && *x > 0.0),
            ),
            y_axis_scale: self.effective_axis_scale(
                self.y_axis_scale,
                self.view_mode.supports_y_log_scale(),
                points.iter().all(|(_, y)| y.is_finite() && *y > 0.0),
            ),
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
