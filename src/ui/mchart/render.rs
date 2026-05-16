use std::ops::Range;

use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea},
    style::{Color as _, IntoFont, RGBColor, ShapeStyle},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::Style,
    symbols::Marker,
    text::{Line, Span, Text},
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType, Paragraph, Wrap},
};
use ratatui_image::{picker::ProtocolType, StatefulImage};

use crate::{configure, error::log_error};

use super::{
    prompt::ExpressionPromptFocus, ChartItem, ChartSource, ExpressionPromptInputKind,
    ExpressionPromptMessageKind, ExpressionPromptMode, ExpressionPromptSuggestion,
    ExpressionPromptSuggestionKind, MultiChartEditorHitbox, MultiChartItemHitbox,
    MultiChartRenderRequest, MultiChartRenderResult, MultiChartState, MultiChartViewModeHitbox,
    Point, EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS,
};

fn mchart_body_style() -> Style {
    let mut style = Style::default().fg(configure::themed_color(|colors| colors.text.primary));
    if configure::prefers_strong_text() {
        style = style.bold();
    }
    style
}

fn mchart_body_span(content: impl Into<String>) -> Span<'static> {
    Span::styled(content.into(), mchart_body_style())
}

fn mchart_mode_tab_style(selected: bool) -> Style {
    if selected {
        Style::default()
            .fg(configure::themed_color(|colors| colors.accent.selection_fg))
            .bg(configure::themed_color(|colors| colors.accent.selection_bg))
            .bold()
    } else {
        Style::default()
            .fg(configure::themed_color(|colors| colors.help.description))
            .bg(configure::themed_color(|colors| colors.surface.help_key_bg))
            .bold()
    }
}

fn render_suggestion_label(
    suggestion: &ExpressionPromptSuggestion,
    base_style: Style,
    highlight_style: Style,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut last_end = 0;
    for span in &suggestion.highlight_spans {
        if span.start > last_end {
            spans.push(Span::styled(
                suggestion.label[last_end..span.start].to_string(),
                base_style,
            ));
        }
        spans.push(Span::styled(
            suggestion.label[span.start..span.end].to_string(),
            highlight_style,
        ));
        last_end = span.end;
    }
    if last_end < suggestion.label.len() {
        spans.push(Span::styled(
            suggestion.label[last_end..].to_string(),
            base_style,
        ));
    }
    if spans.is_empty() {
        spans.push(Span::styled(suggestion.label.clone(), base_style));
    }
    spans
}

fn truncate_to_width(message: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let char_count = message.chars().count();
    if char_count <= max_width {
        return message.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }
    let mut truncated = message.chars().take(max_width - 3).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn normalize_axis_bounds(min: f64, max: f64) -> Option<(f64, f64)> {
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

fn render_line_chart_request(
    request: &MultiChartRenderRequest,
    prepared: &super::PreparedLineChartData,
) -> MultiChartRenderResult {
    let mut plot_buffer = vec![0; (request.width * request.height * 3) as usize];
    let (plot_x_range, plot_y_range) = {
        let root = BitMapBackend::with_buffer(&mut plot_buffer, (request.width, request.height))
            .into_drawing_area();
        let (bg_r, bg_g, bg_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.plot_bg));
        let (grid_r, grid_g, grid_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.grid));
        let (axis_r, axis_g, axis_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.axis));
        let plot_bg = RGBColor(bg_r, bg_g, bg_b);
        let grid = RGBColor(grid_r, grid_g, grid_b);
        let axis = RGBColor(axis_r, axis_g, axis_b);
        if let Err(error) = root.fill(&plot_bg) {
            log_error(&error);
            return MultiChartRenderResult::Failure {
                generation: request.generation,
                message: error.to_string(),
            };
        }
        let y_label_area_size = format!("{:.4}", prepared.y_max).len() as u32 * 3 + 30;
        let chart = plotters::prelude::ChartBuilder::on(&root)
            .margin(10)
            .x_label_area_size(30)
            .y_label_area_size(y_label_area_size)
            .build_cartesian_2d(
                prepared.plot_x_min..prepared.plot_x_max,
                prepared.y_min..prepared.y_max,
            );

        let mut chart = match chart {
            Ok(chart) => chart,
            Err(error) => {
                log_error(&error);
                return MultiChartRenderResult::Failure {
                    generation: request.generation,
                    message: error.to_string(),
                };
            }
        };

        let ranges = chart.plotting_area().get_pixel_range();

        if let Err(error) = chart
            .configure_mesh()
            .x_desc("x values")
            .y_desc("value")
            .y_label_style(("sans-serif", 18).into_font().color(&axis))
            .x_label_style(("sans-serif", 18).into_font().color(&axis))
            .axis_style(ShapeStyle::from(&axis).stroke_width(2))
            .light_line_style(grid.mix(0.35))
            .bold_line_style(grid.mix(0.55))
            .draw()
        {
            log_error(&error);
        }

        for series in prepared.series.iter().cloned() {
            let (r, g, b) = configure::rgb_channels(configure::themed_color(|colors| {
                colors.chart.series[series.color_slot % colors.chart.series.len()]
            }));
            let color = RGBColor(r, g, b);
            let stroke_width = if series.is_selected { 4 } else { 3 };
            let line_series = plotters::prelude::LineSeries::new(
                series.points.iter().copied(),
                ShapeStyle::from(&color).stroke_width(stroke_width),
            );
            let series_label = series.label.clone();
            let drawn_series = match chart.draw_series(line_series) {
                Ok(series) => series,
                Err(error) => {
                    log_error(&error);
                    continue;
                }
            };
            drawn_series.label(series_label).legend(move |(x, y)| {
                plotters::element::PathElement::new(
                    vec![(x, y), (x + 20, y)],
                    color.stroke_width(3),
                )
            });
        }

        if let Err(error) = chart
            .configure_series_labels()
            .background_style(plot_bg.mix(0.85))
            .border_style(axis.mix(0.8))
            .label_font(("sans-serif", 18).into_font().color(&axis))
            .draw()
        {
            log_error(&error);
        }
        ranges
    };

    MultiChartRenderResult::Success {
        generation: request.generation,
        chart_area: request.chart_area,
        width: request.width,
        height: request.height,
        rgb_bytes: plot_buffer,
        plot_x_range,
        plot_y_range,
    }
}

fn render_histogram_request(
    request: &MultiChartRenderRequest,
    prepared: &super::PreparedHistogramData,
) -> MultiChartRenderResult {
    let mut plot_buffer = vec![0; (request.width * request.height * 3) as usize];
    let (plot_x_range, plot_y_range) = {
        let root = BitMapBackend::with_buffer(&mut plot_buffer, (request.width, request.height))
            .into_drawing_area();
        let (bg_r, bg_g, bg_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.plot_bg));
        let (grid_r, grid_g, grid_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.grid));
        let (axis_r, axis_g, axis_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.axis));
        let plot_bg = RGBColor(bg_r, bg_g, bg_b);
        let grid = RGBColor(grid_r, grid_g, grid_b);
        let axis = RGBColor(axis_r, axis_g, axis_b);
        if let Err(error) = root.fill(&plot_bg) {
            log_error(&error);
            return MultiChartRenderResult::Failure {
                generation: request.generation,
                message: error.to_string(),
            };
        }
        let y_label_area_size = format!("{:.0}", prepared.count_max).len() as u32 * 3 + 30;
        let chart = plotters::prelude::ChartBuilder::on(&root)
            .margin(10)
            .x_label_area_size(30)
            .y_label_area_size(y_label_area_size)
            .build_cartesian_2d(
                prepared.value_min..prepared.value_max,
                0.0..prepared.count_max,
            );
        let mut chart = match chart {
            Ok(chart) => chart,
            Err(error) => {
                log_error(&error);
                return MultiChartRenderResult::Failure {
                    generation: request.generation,
                    message: error.to_string(),
                };
            }
        };
        let ranges = chart.plotting_area().get_pixel_range();
        if let Err(error) = chart
            .configure_mesh()
            .x_desc(format!("value ({} bins)", prepared.bin_count))
            .y_desc("count")
            .y_label_style(("sans-serif", 18).into_font().color(&axis))
            .x_label_style(("sans-serif", 18).into_font().color(&axis))
            .axis_style(ShapeStyle::from(&axis).stroke_width(2))
            .light_line_style(grid.mix(0.35))
            .bold_line_style(grid.mix(0.55))
            .draw()
        {
            log_error(&error);
        }
        for series in prepared.series.iter().cloned() {
            let (r, g, b) = configure::rgb_channels(configure::themed_color(|colors| {
                colors.chart.series[series.color_slot % colors.chart.series.len()]
            }));
            let color = RGBColor(r, g, b);
            let stroke_width = if series.is_selected { 3 } else { 2 };
            let drawn_series = match chart.draw_series(series.bins.iter().map(|bin| {
                plotters::prelude::Rectangle::new(
                    [(bin.start, 0.0), (bin.end, bin.count)],
                    color
                        .mix(if series.is_selected { 0.45 } else { 0.28 })
                        .filled(),
                )
            })) {
                Ok(series_drawn) => series_drawn,
                Err(error) => {
                    log_error(&error);
                    continue;
                }
            };
            drawn_series
                .label(series.label.clone())
                .legend(move |(x, y)| {
                    plotters::prelude::Rectangle::new(
                        [(x, y - 5), (x + 20, y + 5)],
                        color.mix(0.45).stroke_width(stroke_width).filled(),
                    )
                });
        }
        if let Err(error) = chart
            .configure_series_labels()
            .background_style(plot_bg.mix(0.85))
            .border_style(axis.mix(0.8))
            .label_font(("sans-serif", 18).into_font().color(&axis))
            .draw()
        {
            log_error(&error);
        }
        ranges
    };
    MultiChartRenderResult::Success {
        generation: request.generation,
        chart_area: request.chart_area,
        width: request.width,
        height: request.height,
        rgb_bytes: plot_buffer,
        plot_x_range,
        plot_y_range,
    }
}

fn render_comparison_scatter_request(
    request: &MultiChartRenderRequest,
    prepared: &super::PreparedComparisonScatterData,
) -> MultiChartRenderResult {
    let mut plot_buffer = vec![0; (request.width * request.height * 3) as usize];
    let (plot_x_range, plot_y_range) = {
        let root = BitMapBackend::with_buffer(&mut plot_buffer, (request.width, request.height))
            .into_drawing_area();
        let (bg_r, bg_g, bg_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.plot_bg));
        let (grid_r, grid_g, grid_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.grid));
        let (axis_r, axis_g, axis_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.axis));
        let plot_bg = RGBColor(bg_r, bg_g, bg_b);
        let grid = RGBColor(grid_r, grid_g, grid_b);
        let axis = RGBColor(axis_r, axis_g, axis_b);
        if let Err(error) = root.fill(&plot_bg) {
            log_error(&error);
            return MultiChartRenderResult::Failure {
                generation: request.generation,
                message: error.to_string(),
            };
        }
        let chart = plotters::prelude::ChartBuilder::on(&root)
            .margin(10)
            .x_label_area_size(30)
            .y_label_area_size(45)
            .build_cartesian_2d(
                prepared.x_min..prepared.x_max,
                prepared.y_min..prepared.y_max,
            );
        let mut chart = match chart {
            Ok(chart) => chart,
            Err(error) => {
                log_error(&error);
                return MultiChartRenderResult::Failure {
                    generation: request.generation,
                    message: error.to_string(),
                };
            }
        };
        let ranges = chart.plotting_area().get_pixel_range();
        if let Err(error) = chart
            .configure_mesh()
            .x_desc(prepared.x_label.clone())
            .y_desc(prepared.y_label.clone())
            .y_label_style(("sans-serif", 18).into_font().color(&axis))
            .x_label_style(("sans-serif", 18).into_font().color(&axis))
            .axis_style(ShapeStyle::from(&axis).stroke_width(2))
            .light_line_style(grid.mix(0.35))
            .bold_line_style(grid.mix(0.55))
            .draw()
        {
            log_error(&error);
        }
        let diagonal_min = prepared.x_min.min(prepared.y_min);
        let diagonal_max = prepared.x_max.max(prepared.y_max);
        let (r, g, b) = configure::rgb_channels(configure::themed_color(|colors| {
            colors.chart.series[prepared.color_slot % colors.chart.series.len()]
        }));
        let color = RGBColor(r, g, b);
        let _ = chart.draw_series(std::iter::once(plotters::element::PathElement::new(
            vec![(diagonal_min, diagonal_min), (diagonal_max, diagonal_max)],
            axis.mix(0.4).stroke_width(2),
        )));
        let points = prepared.points.clone();
        let drawn_series = match chart.draw_series(plotters::prelude::PointSeries::of_element(
            points,
            4,
            color.filled(),
            &|coord, size, style| {
                plotters::element::EmptyElement::at(coord)
                    + plotters::element::Circle::new((0, 0), size, style)
            },
        )) {
            Ok(series) => series,
            Err(error) => {
                log_error(&error);
                return MultiChartRenderResult::Failure {
                    generation: request.generation,
                    message: error.to_string(),
                };
            }
        };
        drawn_series
            .label(prepared.label.clone())
            .legend(move |(x, y)| plotters::element::Circle::new((x + 10, y), 4, color.filled()));
        if let Err(error) = chart
            .configure_series_labels()
            .background_style(plot_bg.mix(0.85))
            .border_style(axis.mix(0.8))
            .label_font(("sans-serif", 18).into_font().color(&axis))
            .draw()
        {
            log_error(&error);
        }
        ranges
    };
    MultiChartRenderResult::Success {
        generation: request.generation,
        chart_area: request.chart_area,
        width: request.width,
        height: request.height,
        rgb_bytes: plot_buffer,
        plot_x_range,
        plot_y_range,
    }
}

pub(super) fn render_prepared_chart_request(
    request: MultiChartRenderRequest,
) -> MultiChartRenderResult {
    match &request.prepared {
        super::PreparedChartData::Line(prepared) => render_line_chart_request(&request, prepared),
        super::PreparedChartData::Histogram(prepared) => {
            render_histogram_request(&request, prepared)
        }
        super::PreparedChartData::ComparisonScatter(prepared) => {
            render_comparison_scatter_request(&request, prepared)
        }
    }
}

impl MultiChartState {
    pub(super) fn chart_panel_title(&self) -> String {
        " 📈 Chart ".to_string()
    }

    pub(super) fn should_defer_image_protocol_frame(&self, chart_area: Rect) -> bool {
        self.expression_prompt.is_some()
            && !self.modified
            && self.stateful_protocol.is_some()
            && self.last_chart_panel_area == Some(chart_area)
    }

    fn item_display_label(&self, item: &ChartItem) -> String {
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
            Some((x_min, x_max)) => super::model::sanitize_chart_points(
                points
                    .filter(|(x, _)| *x >= x_min && *x <= x_max)
                    .collect::<Vec<_>>(),
            ),
            None => super::model::sanitize_chart_points(points.collect::<Vec<_>>()),
        }
    }

    fn comparison_scatter_pair(&self) -> Option<(&ChartItem, &ChartItem)> {
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

    fn comparison_scatter_pair_summary(&self) -> Option<String> {
        let (left, right) = self.comparison_scatter_pair()?;
        Some(format!(
            "{} vs {}",
            self.item_display_label(left),
            self.item_display_label(right)
        ))
    }

    fn comparison_scatter_truncation_note(&self) -> Option<String> {
        self.prepared_comparison_scatter_data()
            .and_then(|prepared| prepared.truncation_note)
    }

    fn mode_window_summary(&self) -> String {
        match (self.view_mode(), self.viewport) {
            (mode, _) if matches!(mode, super::MultiChartViewMode::Line) => {
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

    fn chart_mode_tab_specs(&self) -> Vec<(super::MultiChartViewMode, String, u16)> {
        let labels = [
            (super::MultiChartViewMode::Line, " Line "),
            (super::MultiChartViewMode::Histogram, " Histogram "),
            (
                super::MultiChartViewMode::ComparisonScatter,
                " Comparison scatter ",
            ),
        ];
        labels
            .iter()
            .map(|(mode, label)| {
                (
                    *mode,
                    (*label).to_string(),
                    Line::from(*label).width() as u16,
                )
            })
            .collect()
    }

    fn chart_mode_tabs(&self) -> Line<'static> {
        let mut spans = Vec::new();
        for (index, (mode, label, _)) in self.chart_mode_tab_specs().iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled(
                    "  ",
                    Style::default().fg(configure::themed_color(|colors| colors.help.muted)),
                ));
            }
            spans.push(Span::styled(
                label.clone(),
                mchart_mode_tab_style(*mode == self.view_mode()),
            ));
        }
        Line::from(spans)
    }

    pub(super) fn chart_mode_subheader(&self) -> String {
        match self.view_mode() {
            super::MultiChartViewMode::Line => {
                "[x values] - parametric curves and sampled series".to_string()
            }
            super::MultiChartViewMode::Histogram => {
                "[visible sample values] - overlaid distributions".to_string()
            }
            super::MultiChartViewMode::ComparisonScatter => {
                let summary = self
                    .comparison_scatter_pair_summary()
                    .unwrap_or_else(|| "selected vs next visible series".to_string());
                if let Some(note) = self.comparison_scatter_truncation_note() {
                    format!("[sample aligned, truncated] - {summary} ({note})")
                } else {
                    format!("[sample aligned] - {summary}")
                }
            }
        }
    }

    pub(super) fn unavailable_chart_message(&self) -> String {
        match self.view_mode() {
            super::MultiChartViewMode::Line => format!(
                "No plottable series in the current viewport. {}.",
                self.x_axis_policy.description()
            ),
            super::MultiChartViewMode::Histogram => {
                "No histogram samples in the current visible window.".to_string()
            }
            super::MultiChartViewMode::ComparisonScatter => {
                if self.comparison_scatter_pair().is_some()
                    && self.prepared_comparison_scatter_data().is_none()
                {
                    "Comparison scatter requires matching visible sample positions in both series."
                        .to_string()
                } else {
                    "Comparison scatter needs two visible loaded series; it uses the selected series and the next visible series."
                        .to_string()
                }
            }
        }
    }

    fn prepared_line_chart_data(&self) -> Option<super::PreparedLineChartData> {
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

            series.push(super::PreparedLineChartSeries {
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
            normalize_axis_bounds(plot_x_min, plot_x_max)?
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
            normalize_axis_bounds(y_min, y_max)?
        };

        Some(super::PreparedLineChartData {
            plot_x_min,
            plot_x_max,
            y_min,
            y_max,
            series,
        })
    }

    fn prepared_histogram_data(&self) -> Option<super::PreparedHistogramData> {
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

        let (value_min, value_max) = normalize_axis_bounds(value_min, value_max)?;
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
                    super::PreparedHistogramBin {
                        start,
                        end,
                        count: count as f64,
                    }
                })
                .collect::<Vec<_>>();
            series.push(super::PreparedHistogramSeries {
                label: self.item_display_label(item),
                color_slot: item.color_slot,
                bins,
                is_selected: selected_item_id == Some(item.id),
            });
        }
        Some(super::PreparedHistogramData {
            value_min,
            value_max,
            count_max: count_max.max(1.0),
            bin_count,
            series,
        })
    }

    fn prepared_comparison_scatter_data(&self) -> Option<super::PreparedComparisonScatterData> {
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

        Some(super::PreparedComparisonScatterData {
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

    pub(super) fn prepared_chart_data(&self) -> Option<super::PreparedChartData> {
        match self.view_mode() {
            super::MultiChartViewMode::Line => self
                .prepared_line_chart_data()
                .map(super::PreparedChartData::Line),
            super::MultiChartViewMode::Histogram => self
                .prepared_histogram_data()
                .map(super::PreparedChartData::Histogram),
            super::MultiChartViewMode::ComparisonScatter => self
                .prepared_comparison_scatter_data()
                .map(super::PreparedChartData::ComparisonScatter),
        }
    }

    pub(crate) fn render(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let inner_area = area;
        f.render_widget(
            Paragraph::new(Line::raw(""))
                .style(Style::default().bg(configure::themed_color(|colors| colors.surface.bg))),
            inner_area,
        );
        let wide_layout = inner_area.width >= 110;

        if wide_layout {
            let panes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(42), Constraint::Min(20)])
                .split(inner_area);
            let sidebar_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(10),
                    Constraint::Length(8),
                    Constraint::Length(8),
                ])
                .split(panes[0]);
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(7), Constraint::Min(10)])
                .split(panes[1]);

            self.render_item_list(f, sidebar_chunks[0]);
            self.render_selected_details(f, sidebar_chunks[1]);
            self.render_selected_statistics(f, sidebar_chunks[2]);

            self.render_expression_prompt(f, main_chunks[0]);

            if self.items.is_empty() {
                self.render_empty(f, main_chunks[1]);
            } else {
                self.render_chart_panel(f, main_chunks[1]);
            }
            return;
        }

        let panes = {
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(7), Constraint::Min(10)])
                .split(inner_area);
            let prompt_area = split[0];
            let workspace_area = split[1];
            self.render_expression_prompt(f, prompt_area);
            if self.items.is_empty() {
                self.render_empty(f, workspace_area);
                return;
            }

            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(12),
                    Constraint::Length(11),
                    Constraint::Length(8),
                    Constraint::Length(8),
                ])
                .split(workspace_area)
        };
        self.render_chart_panel(f, panes[0]);
        self.render_item_list(f, panes[1]);
        self.render_selected_details(f, panes[2]);
        self.render_selected_statistics(f, panes[3]);
    }

    fn render_empty(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        self.last_chart_area = None;
        self.drag_state = None;
        self.view_mode_hitboxes.clear();
        let no_data_message = concat!(
            "No chart items yet.\n\n",
            "Press 'm' on any previewable dataset view to add it here.\n",
            "The same dataset can appear multiple times with different x dimensions or fixed indices.\n",
            "Press Enter or 'e' to create an expression from the current selection."
        );
        let paragraph = Paragraph::new(no_data_message)
            .alignment(Alignment::Center)
            .style(Style::default().fg(configure::themed_color(|colors| colors.mchart.empty_state)))
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, area);
    }

    fn render_item_list(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let block = Block::default()
            .title(" 🧾 Items ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(configure::themed_color(|colors| {
                colors.surface.panel_border
            })))
            .title_style(
                Style::default()
                    .fg(configure::themed_color(|colors| colors.surface.panel_title))
                    .bold(),
            )
            .title_alignment(Alignment::Center);
        let inner = block.inner(area);
        f.render_widget(block, area);
        self.item_hitboxes.clear();

        let available_rows = inner.height as usize;
        let visible_items = available_rows / 2;
        if visible_items == 0 {
            return;
        }

        let half = visible_items / 2;
        let start = self.idx.saturating_sub(half);
        let end = usize::min(start + visible_items, self.items.len());
        let start = end.saturating_sub(visible_items);

        let lines = self.items[start..end]
            .iter()
            .enumerate()
            .flat_map(|(offset, item)| {
                let absolute_idx = start + offset;
                let is_selected = absolute_idx == self.idx;
                let has_error = matches!(item.load_state, super::MultiChartLoadState::Error(_));
                let marker_color = if has_error {
                    configure::themed_color(|colors| colors.text.error)
                } else {
                    configure::themed_color(|colors| {
                        colors.chart.series[item.color_slot % colors.chart.series.len()]
                    })
                };
                let marker = if has_error {
                    "⚠"
                } else if item.visible {
                    configure::configured_symbol(|symbols| symbols.chart.visibility_visible)
                } else {
                    configure::configured_symbol(|symbols| symbols.chart.visibility_hidden)
                };
                let row_style = match (has_error, is_selected, item.visible) {
                    (true, true, _) => Style::default()
                        .fg(configure::themed_color(|colors| colors.text.error))
                        .bg(configure::themed_color(|colors| colors.surface.focus_bg))
                        .dim(),
                    (true, false, _) => Style::default()
                        .fg(configure::themed_color(|colors| colors.text.error))
                        .dim(),
                    (false, true, true) => Style::default()
                        .fg(configure::themed_color(|colors| colors.text.primary))
                        .bg(configure::themed_color(|colors| colors.surface.focus_bg))
                        .bold(),
                    (false, true, false) => Style::default()
                        .fg(configure::themed_color(|colors| colors.text.primary))
                        .bg(configure::themed_color(|colors| colors.surface.focus_bg))
                        .bold()
                        .dim(),
                    (false, false, true) => Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.item_visible)),
                    (false, false, false) => Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.item_hidden))
                        .dim(),
                };
                let id_text = format!("${}", item.id.0);
                let prefix_width = 5 + id_text.chars().count();
                let label_width = inner.width as usize - prefix_width.min(inner.width as usize);
                let label = truncate_to_width(&item.list_label(), label_width);
                let selected_bg =
                    is_selected.then(|| configure::themed_color(|colors| colors.surface.focus_bg));
                let label_style = if item.name.is_some() {
                    let mut style = Style::default()
                        .fg(configure::themed_color(|colors| colors.tree.dataset_file))
                        .underlined();
                    if let Some(bg) = selected_bg {
                        style = style.bg(bg);
                    }
                    if !item.visible || has_error {
                        style = style.dim();
                    }
                    style
                } else {
                    row_style
                };
                let mut id_style = Style::default()
                    .fg(configure::themed_color(|colors| colors.toast.warning))
                    .bold();
                if let Some(bg) = selected_bg {
                    id_style = id_style.bg(bg);
                }
                if !item.visible || has_error {
                    id_style = id_style.dim();
                }
                let first_line = Line::from(vec![
                    Span::styled(" ", row_style),
                    Span::styled(marker, row_style.fg(marker_color).bold()),
                    Span::styled(" ", row_style),
                    Span::styled(format!("{id_text} "), id_style),
                    Span::styled(label, label_style),
                ]);
                let state_label = if has_error {
                    "expression error".to_string()
                } else {
                    item.data_state_label()
                };
                let second_line = Line::from(vec![
                    Span::styled("   ", row_style),
                    Span::styled(
                        format!("[{}]", state_label),
                        if has_error {
                            row_style
                                .fg(configure::themed_color(|colors| colors.text.error))
                                .dim()
                        } else {
                            row_style
                                .fg(configure::themed_color(|colors| colors.mchart.detail_label))
                                .dim()
                        },
                    ),
                ]);
                [first_line, second_line]
            })
            .collect::<Vec<_>>();
        self.item_hitboxes = (start..end)
            .enumerate()
            .map(|(offset, absolute_idx)| MultiChartItemHitbox {
                area: Rect::new(
                    inner.x,
                    inner.y.saturating_add((offset * 2) as u16),
                    inner.width,
                    2,
                ),
                index: absolute_idx,
            })
            .collect();
        f.render_widget(
            Paragraph::new(Text::from(lines)).style(mchart_body_style()),
            inner,
        );
    }

    fn render_selected_details(&self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let block = Block::default()
            .title("Active item")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(configure::themed_color(|colors| {
                colors.surface.panel_border
            })))
            .title_style(
                Style::default()
                    .fg(configure::themed_color(|colors| colors.surface.panel_title))
                    .bold(),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);

        let Some(item) = self.selected_item() else {
            return;
        };

        let viewport = self.mode_window_summary();
        let mode_label = self.view_mode().label();
        let lines = match &item.source {
            ChartSource::DatasetSelection(source) => vec![
                Line::from(vec![
                    Span::styled(
                        "mode ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(mode_label),
                ]),
                Line::from(vec![
                    Span::styled(
                        "path ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(source.display_path.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "type ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(source.kind_label()),
                    mchart_body_span("  "),
                    Span::styled(
                        "shape ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(source.shape_summary()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "view ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(source.selection_summary()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "align ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(self.x_axis_policy.label()),
                    mchart_body_span("  "),
                    Span::styled(
                        "view ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(viewport),
                ]),
                Line::from(vec![
                    Span::styled(
                        "data ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(item.data_state_label()),
                ]),
            ],
            ChartSource::DerivedExpression { expression, .. } => vec![
                Line::from(vec![
                    Span::styled(
                        "mode ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(mode_label),
                ]),
                Line::from(vec![
                    Span::styled(
                        "expr ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(expression.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "type ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(item.source.source_kind_label()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "align ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(self.x_axis_policy.label()),
                    mchart_body_span("  "),
                    Span::styled(
                        "view ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(viewport),
                ]),
                Line::from(vec![
                    Span::styled(
                        "data ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(item.data_state_label()),
                ]),
            ],
        };

        f.render_widget(
            Paragraph::new(Text::from(lines))
                .style(mchart_body_style())
                .wrap(Wrap { trim: true }),
            inner,
        );
    }

    fn render_selected_statistics(&self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let block = Block::default()
            .title("Statistics")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(configure::themed_color(|colors| {
                colors.surface.panel_border
            })))
            .title_style(
                Style::default()
                    .fg(configure::themed_color(|colors| colors.surface.panel_title))
                    .bold(),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);

        let Some(item) = self.selected_item() else {
            return;
        };
        if !item.has_loaded_series() {
            f.render_widget(
                Paragraph::new(item.data_state_label())
                    .style(mchart_body_style())
                    .wrap(Wrap { trim: true }),
                inner,
            );
            return;
        }
        let stats = item.statistics();
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    "samples ",
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                ),
                mchart_body_span(stats.samples.to_string()),
                mchart_body_span("  "),
                Span::styled(
                    "x ",
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                ),
                mchart_body_span(format!("[{:.4}, {:.4}]", stats.x_min, stats.x_max)),
            ]),
            Line::from(vec![
                Span::styled(
                    "y ",
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                ),
                mchart_body_span(format!("[{:.4}, {:.4}]", stats.y_min, stats.y_max)),
            ]),
            Line::from(vec![
                Span::styled(
                    "mean ",
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                ),
                mchart_body_span(format!("{:.4}", stats.mean)),
                mchart_body_span("  "),
                Span::styled(
                    "median ",
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                ),
                mchart_body_span(format!("{:.4}", stats.median)),
            ]),
            Line::from(vec![
                Span::styled(
                    "stddev ",
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                ),
                mchart_body_span(format!("{:.4}", stats.stddev)),
            ]),
        ];
        f.render_widget(
            Paragraph::new(Text::from(lines))
                .style(mchart_body_style())
                .wrap(Wrap { trim: true }),
            inner,
        );
    }

    fn render_chart_panel(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        self.view_mode_hitboxes.clear();
        let block = Block::default()
            .title(self.chart_panel_title())
            .borders(Borders::TOP)
            .border_style(
                Style::default().fg(configure::themed_color(|colors| colors.surface.break_line)),
            )
            .title_style(
                Style::default()
                    .fg(configure::themed_color(|colors| colors.surface.panel_title))
                    .bold(),
            )
            .title_alignment(Alignment::Center);
        let chart_panel_area = block.inner(area);
        f.render_widget(block, area);

        let (tabs_area, subheader_area, chart_area) = if chart_panel_area.height >= 3 {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .split(chart_panel_area);
            (Some(chunks[0]), Some(chunks[1]), chunks[2])
        } else {
            (None, None, chart_panel_area)
        };
        if let Some(tabs_area) = tabs_area {
            let tabs = self.chart_mode_tabs();
            let tab_specs = self.chart_mode_tab_specs();
            let tabs_width = tabs.width() as u16;
            let start_x = tabs_area
                .x
                .saturating_add(tabs_area.width.saturating_sub(tabs_width) / 2);
            let separator_width = Line::from("  ").width() as u16;
            let mut current_x = start_x;
            for (index, (mode, _, width)) in tab_specs.iter().enumerate() {
                self.view_mode_hitboxes.push(MultiChartViewModeHitbox {
                    area: Rect::new(current_x, tabs_area.y, *width, 1),
                    mode: *mode,
                });
                current_x = current_x.saturating_add(*width);
                if index + 1 != tab_specs.len() {
                    current_x = current_x.saturating_add(separator_width);
                }
            }
            f.render_widget(Paragraph::new(tabs).alignment(Alignment::Center), tabs_area);
        }
        if let Some(subheader_area) = subheader_area {
            f.render_widget(
                Paragraph::new(self.chart_mode_subheader())
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(configure::themed_color(|colors| colors.help.muted)))
                    .wrap(Wrap { trim: true }),
                subheader_area,
            );
        }

        if self.visible_item_count() == 0 {
            self.last_chart_area = None;
            self.last_chart_panel_area = None;
            let paragraph = Paragraph::new(format!(
                "All chart items are hidden.\nPress Space or 'v' to toggle the selected item back on.\nCurrent mode: {}.",
                self.view_mode().label()
            ))
            .style(Style::default().fg(configure::themed_color(|colors| colors.mchart.empty_state)))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
            f.render_widget(paragraph, chart_area);
            return;
        }
        if self
            .items
            .iter()
            .filter(|item| item.visible)
            .all(|item| !item.has_loaded_series())
        {
            self.last_chart_area = None;
            self.last_chart_panel_area = None;
            let paragraph = Paragraph::new("Loading sampled chart data...")
                .style(
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.empty_state)),
                )
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            f.render_widget(paragraph, chart_area);
            return;
        }
        if chart_area.width == 0 || chart_area.height == 0 {
            self.last_chart_area = None;
            self.last_chart_panel_area = None;
            return;
        }
        if self.picker.protocol_type() == ProtocolType::Halfblocks
            && matches!(self.view_mode(), super::MultiChartViewMode::Line)
        {
            self.last_chart_area = Some(chart_area);
            self.last_chart_panel_area = Some(chart_area);
            if !self.render_braille_chart_panel(f, chart_area) {
                let paragraph = Paragraph::new(self.unavailable_chart_message())
                    .style(Style::default().fg(configure::themed_color(|colors| colors.text.error)))
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: true });
                f.render_widget(paragraph, chart_area);
            }
            return;
        }

        let (cell_w, cell_h) = self.picker.font_size();
        let new_height = chart_area.height as u32 * cell_h as u32;
        let new_width = chart_area.width as u32 * cell_w as u32;
        self.last_chart_panel_area = Some(chart_area);
        if new_height != self.height || new_width != self.width {
            self.height = new_height;
            self.width = new_width;
            self.modified = true;
            self.stateful_protocol = None;
            self.pending_render_generation = None;
        }

        if self.modified {
            self.queue_chart_render(chart_area);
        }
        if self.should_defer_image_protocol_frame(chart_area) {
            return;
        }

        match self.stateful_protocol {
            None => {
                let (message, color) = if let Some(error) = &self.render_error {
                    (
                        error.clone(),
                        configure::themed_color(|colors| colors.text.error),
                    )
                } else if self.pending_render_generation.is_some() {
                    (
                        "Rendering chart...".to_string(),
                        configure::themed_color(|colors| colors.mchart.empty_state),
                    )
                } else {
                    (
                        self.unavailable_chart_message(),
                        configure::themed_color(|colors| colors.mchart.empty_state),
                    )
                };
                let paragraph = Paragraph::new(message)
                    .style(Style::default().fg(color))
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: true });
                f.render_widget(paragraph, chart_area);
            }
            Some(ref mut protocol) => {
                f.render_stateful_widget(StatefulImage::default(), chart_area, protocol);
            }
        }
    }

    fn render_braille_chart_panel(&self, f: &mut ratatui::Frame<'_>, chart_area: Rect) -> bool {
        let Some(super::PreparedChartData::Line(prepared)) = self.prepared_chart_data() else {
            return false;
        };

        let x_label_count = if chart_area.width == 0 {
            0
        } else {
            chart_area.width / 12
        };
        let x_labels = (0..=x_label_count)
            .map(|i| {
                let x = prepared.plot_x_min
                    + (prepared.plot_x_max - prepared.plot_x_min) * (i as f64)
                        / (x_label_count.max(1) as f64);
                Span::styled(
                    format!("{x:.1}"),
                    configure::themed_color(|colors| colors.chart.label),
                )
            })
            .collect::<Vec<_>>();

        let y_label_count = match chart_area.height {
            0 => 0,
            _ => chart_area.height / 4,
        };
        let y_labels = (0..=y_label_count)
            .map(|i| {
                let y = prepared.y_min
                    + (prepared.y_max - prepared.y_min) * (i as f64)
                        / (y_label_count.max(1) as f64);
                Span::styled(
                    format!("{y:.1}"),
                    configure::themed_color(|colors| colors.chart.label),
                )
            })
            .collect::<Vec<_>>();

        let datasets = prepared
            .series
            .iter()
            .map(|series| {
                let mut style = Style::default().fg(configure::themed_color(|colors| {
                    colors.chart.series[series.color_slot % colors.chart.series.len()]
                }));
                if series.is_selected {
                    style = style.bold();
                }
                Dataset::default()
                    .marker(Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(style)
                    .data(series.points.as_slice())
            })
            .collect::<Vec<_>>();

        let chart = Chart::new(datasets)
            .style(Style::default().bg(configure::themed_color(|colors| colors.chart.plot_bg)))
            .x_axis(
                Axis::default()
                    .title(self.x_axis_policy.label())
                    .style(Style::default().fg(configure::themed_color(|colors| colors.chart.axis)))
                    .labels(x_labels)
                    .bounds((prepared.plot_x_min, prepared.plot_x_max).into()),
            )
            .y_axis(
                Axis::default()
                    .title("value")
                    .style(Style::default().fg(configure::themed_color(|colors| colors.chart.axis)))
                    .labels(y_labels)
                    .bounds((prepared.y_min, prepared.y_max).into()),
            );
        f.render_widget(chart, chart_area);
        true
    }

    fn render_expression_prompt(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let prompt = self.expression_prompt.as_ref();
        let panel_bg = configure::themed_color(|colors| colors.surface.bg);
        let title = match prompt {
            Some(prompt)
                if prompt
                    .messages
                    .iter()
                    .any(|message| message.kind == ExpressionPromptMessageKind::Error) =>
            {
                " ∑ Expression editor [invalid]"
            }
            Some(prompt) if matches!(prompt.mode, ExpressionPromptMode::EditExisting(_)) => {
                " ∑ Expression editor [edit]"
            }
            Some(_) => " ∑ Expression editor ",
            None => " ∑ Expression editor ",
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().bg(panel_bg))
            .border_style(Style::default().fg(configure::themed_color(|colors| {
                colors.surface.panel_border
            })))
            .title_style(
                Style::default()
                    .fg(configure::themed_color(|colors| colors.surface.panel_title))
                    .bold(),
            )
            .title_alignment(Alignment::Center);
        let inner = block.inner(area);
        f.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(2)])
            .split(inner);

        let input_text = match prompt {
            Some(prompt) => {
                let name_style = if prompt.name_buffer.is_empty() {
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.text.type_desc))
                        .dim()
                } else {
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.tree.dataset_file))
                        .underlined()
                };
                let mut spans = vec![
                    Span::styled(
                        format!("${} ", prompt.item_id.0),
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.toast.warning))
                            .bold(),
                    ),
                    Span::styled(
                        if prompt.name_buffer.is_empty() {
                            "(?)".to_string()
                        } else {
                            format!("${}", prompt.name_buffer)
                        },
                        if prompt.focus == ExpressionPromptFocus::Name {
                            name_style.bold()
                        } else {
                            name_style
                        },
                    ),
                    Span::styled(
                        " = ",
                        Style::default()
                            .fg(configure::themed_color(|colors| {
                                colors.mchart.prompt_prefix
                            }))
                            .bold(),
                    ),
                ];
                if prompt.buffer.is_empty() {
                    spans.push(Span::styled(
                        "$1 + load(/path)[..,0]",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.text.type_desc))
                            .italic(),
                    ));
                } else {
                    spans.extend(prompt.input_segments.iter().map(|segment| {
                        let style = match segment.kind {
                            ExpressionPromptInputKind::Plain => mchart_body_style(),
                            ExpressionPromptInputKind::ValidReference => Style::default()
                                .fg(configure::themed_color(|colors| {
                                    colors.mchart.prompt_prefix
                                }))
                                .bold(),
                            ExpressionPromptInputKind::InvalidReference => Style::default()
                                .fg(configure::themed_color(|colors| colors.text.error))
                                .bold(),
                        };
                        Span::styled(segment.text.clone(), style)
                    }));
                }
                Line::from(spans)
            }
            None => {
                if let Some(item) = self.selected_item() {
                    let name_style = if item.name.is_some() {
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.tree.dataset_file))
                            .underlined()
                    } else {
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.text.type_desc))
                            .dim()
                    };
                    Line::from(vec![
                        Span::styled(
                            format!("${} ", item.id.0),
                            Style::default()
                                .fg(configure::themed_color(|colors| colors.toast.warning))
                                .bold(),
                        ),
                        Span::styled(
                            item.name
                                .as_ref()
                                .map(|name| format!("${name}"))
                                .unwrap_or_else(|| "(?)".to_string()),
                            name_style,
                        ),
                        Span::styled(
                            " = ",
                            Style::default()
                                .fg(configure::themed_color(|colors| {
                                    colors.mchart.prompt_prefix
                                }))
                                .bold()
                                .dim(),
                        ),
                        Span::styled(
                            item.editable_expression()
                                .unwrap_or_else(|| "expression editor inactive".to_string()),
                            Style::default()
                                .fg(configure::themed_color(|colors| colors.text.type_desc))
                                .italic()
                                .dim(),
                        ),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled(
                            "$? (?) = ",
                            Style::default()
                                .fg(configure::themed_color(|colors| {
                                    colors.mchart.prompt_prefix
                                }))
                                .bold(),
                        ),
                        Span::styled(
                            "expression editor inactive",
                            Style::default()
                                .fg(configure::themed_color(|colors| colors.text.type_desc))
                                .italic(),
                        ),
                    ])
                }
            }
        };
        let input_block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().bg(panel_bg))
            .border_style(
                Style::default().fg(configure::themed_color(|colors| colors.surface.break_line)),
            );
        let input_area = if chunks[0].width > 2 {
            chunks[0].inner(Margin {
                horizontal: 1,
                vertical: 0,
            })
        } else {
            chunks[0]
        };
        let input_inner = input_block.inner(input_area);
        f.render_widget(input_block.clone(), input_area);
        f.render_widget(
            Paragraph::new(input_text).style(mchart_body_style().bg(panel_bg)),
            input_inner,
        );
        self.editor_hitbox = if input_inner.height == 0 || input_inner.width == 0 {
            None
        } else if let Some(prompt) = prompt {
            let id_width = format!("${} ", prompt.item_id.0).chars().count() as u16;
            let name_text = if prompt.name_buffer.is_empty() {
                "(?)".to_string()
            } else {
                format!("${}", prompt.name_buffer)
            };
            let expression_text = if prompt.buffer.is_empty() {
                "$1 + load(/path)[..,0]".to_string()
            } else {
                prompt.buffer.clone()
            };
            let name_width = name_text.chars().count().max(1) as u16;
            let expr_x = input_inner
                .x
                .saturating_add(id_width)
                .saturating_add(name_width)
                .saturating_add(3);
            Some(MultiChartEditorHitbox {
                area: input_inner,
                name_area: Rect::new(
                    input_inner.x.saturating_add(id_width),
                    input_inner.y,
                    name_width,
                    1,
                ),
                expression_area: Rect::new(
                    expr_x,
                    input_inner.y,
                    expression_text.chars().count().max(1) as u16,
                    1,
                ),
            })
        } else if let Some(item) = self.selected_item() {
            let id_width = format!("${} ", item.id.0).chars().count() as u16;
            let name_text = item
                .name
                .as_ref()
                .map(|name| format!("${name}"))
                .unwrap_or_else(|| "(?)".to_string());
            let expression_text = item
                .editable_expression()
                .unwrap_or_else(|| "expression editor inactive".to_string());
            let name_width = name_text.chars().count().max(1) as u16;
            let expr_x = input_inner
                .x
                .saturating_add(id_width)
                .saturating_add(name_width)
                .saturating_add(3);
            Some(MultiChartEditorHitbox {
                area: input_inner,
                name_area: Rect::new(
                    input_inner.x.saturating_add(id_width),
                    input_inner.y,
                    name_width,
                    1,
                ),
                expression_area: Rect::new(
                    expr_x,
                    input_inner.y,
                    expression_text.chars().count().max(1) as u16,
                    1,
                ),
            })
        } else {
            None
        };

        let mut lines = Vec::new();
        if let Some(prompt) = prompt {
            if !prompt.suggestions.is_empty() {
                let suggestion_line = prompt
                    .suggestions
                    .iter()
                    .take(EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS)
                    .enumerate()
                    .flat_map(|(idx, suggestion)| {
                        let (symbol_color, label_color) = match suggestion.kind {
                            ExpressionPromptSuggestionKind::Function => (
                                configure::themed_color(|colors| colors.mchart.detail_label),
                                configure::themed_color(|colors| colors.text.type_desc),
                            ),
                            ExpressionPromptSuggestionKind::Group => (
                                configure::themed_color(|colors| colors.tree.group),
                                configure::themed_color(|colors| colors.tree.group_name),
                            ),
                            ExpressionPromptSuggestionKind::Dataset => (
                                configure::themed_color(|colors| colors.tree.dataset_file),
                                configure::themed_color(|colors| colors.tree.dataset),
                            ),
                            ExpressionPromptSuggestionKind::CompoundLeaf => (
                                configure::themed_color(|colors| colors.tree.compound),
                                configure::themed_color(|colors| colors.tree.compound_name),
                            ),
                            ExpressionPromptSuggestionKind::Attribute => (
                                configure::themed_color(|colors| colors.mchart.detail_label),
                                configure::themed_color(|colors| colors.text.type_desc),
                            ),
                            ExpressionPromptSuggestionKind::ItemRef => (
                                configure::themed_color(|colors| colors.mchart.detail_label),
                                configure::themed_color(|colors| colors.text.primary),
                            ),
                        };
                        let selected_bg = if prompt.selected_suggestion == Some(idx) {
                            configure::themed_color(|colors| colors.surface.focus_bg)
                        } else {
                            panel_bg
                        };
                        let symbol_style = Style::default().fg(symbol_color).bg(selected_bg);
                        let label_style = Style::default().fg(label_color).bg(selected_bg);
                        let highlight_style = Style::default()
                            .fg(configure::themed_color(|colors| {
                                colors.accent.search_highlight
                            }))
                            .bg(selected_bg)
                            .bold();
                        let detail_style = Style::default()
                            .fg(configure::themed_color(|colors| colors.text.type_desc))
                            .bg(selected_bg);
                        let mut spans = vec![Span::styled(
                            format!(" {} ", suggestion.symbol),
                            symbol_style,
                        )];
                        spans.extend(render_suggestion_label(
                            suggestion,
                            label_style,
                            highlight_style,
                        ));
                        spans.push(Span::styled(suggestion.detail.clone(), detail_style));
                        spans.push(Span::raw(" "));
                        spans
                    })
                    .collect::<Vec<_>>();
                lines.push(Line::from(suggestion_line));
            } else {
                lines.push(Line::from(Span::styled(
                    "(none)",
                    Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)),
                )));
            }

            if let Some(message) = prompt.messages.first() {
                let color = match message.kind {
                    ExpressionPromptMessageKind::Error => {
                        configure::themed_color(|colors| colors.text.error)
                    }
                    ExpressionPromptMessageKind::Valid => {
                        configure::themed_color(|colors| colors.mchart.prompt_prefix)
                    }
                    ExpressionPromptMessageKind::Hint => {
                        configure::themed_color(|colors| colors.text.type_desc)
                    }
                };
                lines.push(Line::from(Span::styled(
                    message.text.clone(),
                    Style::default().fg(color),
                )));
            }
        }
        f.render_widget(
            Paragraph::new(Text::from(lines))
                .style(mchart_body_style().bg(panel_bg))
                .wrap(Wrap { trim: true }),
            chunks[1],
        );

        if let Some(prompt) = prompt {
            let name_display = if prompt.name_buffer.is_empty() {
                "(?)".to_string()
            } else {
                format!("${}", prompt.name_buffer)
            };
            let id_prefix_width = format!("${} ", prompt.item_id.0).chars().count();
            let prefix_width = id_prefix_width + name_display.chars().count() + 3;
            let cursor = ratatui::layout::Position::new(
                if prompt.focus == ExpressionPromptFocus::Name {
                    let name_offset = usize::from(!prompt.name_buffer.is_empty());
                    let name_cursor = MultiChartState::prompt_cursor_char_offset(
                        &prompt.name_buffer,
                        prompt.name_cursor,
                    );
                    input_inner
                        .x
                        .saturating_add((id_prefix_width + name_offset + name_cursor) as u16)
                } else {
                    let expression_cursor =
                        MultiChartState::prompt_cursor_char_offset(&prompt.buffer, prompt.cursor);
                    input_inner
                        .x
                        .saturating_add((prefix_width + expression_cursor) as u16)
                },
                input_inner.y,
            );
            f.set_cursor_position(cursor);
        }
    }
}

pub(crate) fn chart_plot_area_in_rect(
    outer_area: Rect,
    width_px: u32,
    height_px: u32,
    plot_x_range: Range<i32>,
    plot_y_range: Range<i32>,
) -> Option<Rect> {
    if outer_area.width == 0 || outer_area.height == 0 || width_px == 0 || height_px == 0 {
        return None;
    }
    let x_start = plot_x_range.start.max(0) as u32;
    let x_end = plot_x_range.end.max(plot_x_range.start).max(0) as u32;
    let y_start = plot_y_range.start.max(0) as u32;
    let y_end = plot_y_range.end.max(plot_y_range.start).max(0) as u32;
    if x_end <= x_start || y_end <= y_start {
        return None;
    }

    let left = x_start
        .saturating_mul(outer_area.width as u32)
        .checked_div(width_px)
        .unwrap_or(0);
    let right = ((x_end.saturating_mul(outer_area.width as u32)) + width_px.saturating_sub(1))
        .checked_div(width_px)
        .unwrap_or(outer_area.width as u32)
        .min(outer_area.width as u32);
    let top = y_start
        .saturating_mul(outer_area.height as u32)
        .checked_div(height_px)
        .unwrap_or(0);
    let bottom = ((y_end.saturating_mul(outer_area.height as u32)) + height_px.saturating_sub(1))
        .checked_div(height_px)
        .unwrap_or(outer_area.height as u32)
        .min(outer_area.height as u32);

    let width = right.saturating_sub(left).max(1) as u16;
    let height = bottom.saturating_sub(top).max(1) as u16;
    Some(Rect::new(
        outer_area.x.saturating_add(left as u16),
        outer_area.y.saturating_add(top as u16),
        width.min(outer_area.width.saturating_sub(left as u16)),
        height.min(outer_area.height.saturating_sub(top as u16)),
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{mchart_body_span, mchart_body_style, mchart_mode_tab_style};

    #[test]
    fn mchart_body_style_uses_primary_text_color() {
        assert_eq!(
            mchart_body_style().fg,
            Some(crate::configure::themed_color(|colors| colors.text.primary))
        );
    }

    #[test]
    fn mchart_body_spans_use_primary_text_color() {
        assert_eq!(mchart_body_span("value").style.fg, mchart_body_style().fg);
    }

    #[test]
    fn selected_mode_tabs_use_selection_accent_colors() {
        let style = mchart_mode_tab_style(true);
        assert_eq!(
            style.fg,
            Some(crate::configure::themed_color(|colors| {
                colors.accent.selection_fg
            }))
        );
        assert_eq!(
            style.bg,
            Some(crate::configure::themed_color(|colors| {
                colors.accent.selection_bg
            }))
        );
    }
}
