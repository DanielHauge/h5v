use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea, IntoLogRange, Text},
    style::{
        text_anchor::{HPos, Pos, VPos},
        Color as _, IntoFont, RGBColor, ShapeStyle,
    },
};

use crate::{
    configure,
    error::log_error,
    ui::chart_math::{
        axis_label_area_size, axis_title_label_area_size, format_axis_number, raster_chart_layout,
        symlog, symlog_inverse, RasterChartLayout, RasterChartLayoutHints,
    },
};

use super::super::{
    ChartAxisScale, MultiChartRenderRequest, MultiChartRenderResult, PreparedBoxPlotData,
    PreparedChartData, PreparedComparisonScatterData, PreparedHistogramData, PreparedLineChartData,
};

fn line_chart_layout(width: u32, height: u32, y_label_area_size: u32) -> RasterChartLayout {
    raster_chart_layout(
        width,
        height,
        RasterChartLayoutHints {
            preferred_margin: 10,
            preferred_x_label_area_size: axis_title_label_area_size(18),
            preferred_y_label_area_size: y_label_area_size,
            preferred_x_label_font_size: 18,
            preferred_y_label_font_size: 18,
            min_plot_width: 48,
            min_plot_height: 40,
        },
    )
}

fn valid_log_range(min: f64, max: f64) -> bool {
    min.is_finite() && max.is_finite() && min > 0.0 && max > min
}

fn scale_value(scale: ChartAxisScale, value: f64) -> f64 {
    match scale {
        ChartAxisScale::Linear => value,
        ChartAxisScale::Logarithmic => value.ln(),
        ChartAxisScale::SymLog => symlog(value),
    }
}

fn axis_label(scale: ChartAxisScale, value: f64) -> String {
    format_axis_number(match scale {
        ChartAxisScale::Linear => value,
        ChartAxisScale::Logarithmic => value.exp(),
        ChartAxisScale::SymLog => symlog_inverse(value),
    })
}

fn histogram_tick_indices(boundary_count: usize, max_labels: usize) -> Vec<usize> {
    if boundary_count <= max_labels {
        return (0..boundary_count).collect();
    }
    let step = (boundary_count - 1).div_ceil(max_labels.saturating_sub(1));
    (0..boundary_count)
        .step_by(step)
        .chain(std::iter::once(boundary_count - 1))
        .collect()
}

fn render_line_chart_request(
    request: &MultiChartRenderRequest,
    prepared: &PreparedLineChartData,
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
        let layout = line_chart_layout(
            request.width,
            request.height,
            axis_label_area_size(&[prepared.y_min, prepared.y_max], 30),
        );
        macro_rules! draw_line_chart {
            ($x:expr, $y:expr, $points:expr, $x_scale:expr, $y_scale:expr) => {{
                let mut chart = match plotters::prelude::ChartBuilder::on(&root)
                    .margin(layout.margin)
                    .x_label_area_size(layout.x_label_area_size)
                    .y_label_area_size(layout.y_label_area_size)
                    .build_cartesian_2d($x, $y)
                {
                    Ok(chart) => chart,
                    Err(error) => {
                        return MultiChartRenderResult::Failure {
                            generation: request.generation,
                            message: error.to_string(),
                        }
                    }
                };
                let ranges = chart.plotting_area().get_pixel_range();
                if let Err(error) = chart
                    .configure_mesh()
                    .x_desc("x values")
                    .y_desc("value")
                    .x_label_formatter(&|value| axis_label($x_scale, *value))
                    .y_label_formatter(&|value| axis_label($y_scale, *value))
                    .y_label_style(
                        ("sans-serif", layout.y_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
                    .x_label_style(
                        ("sans-serif", layout.x_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
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
                        $points(&series),
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
                    .label_font(
                        ("sans-serif", layout.y_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
                    .draw()
                {
                    log_error(&error);
                }
                ranges
            }};
        }
        if prepared.x_axis_scale == ChartAxisScale::SymLog
            || prepared.y_axis_scale == ChartAxisScale::SymLog
        {
            draw_line_chart!(
                scale_value(prepared.x_axis_scale, prepared.plot_x_min)
                    ..scale_value(prepared.x_axis_scale, prepared.plot_x_max),
                scale_value(prepared.y_axis_scale, prepared.y_min)
                    ..scale_value(prepared.y_axis_scale, prepared.y_max),
                |series: &super::super::PreparedLineChartSeries| series
                    .points
                    .iter()
                    .map(|(x, y)| (
                        scale_value(prepared.x_axis_scale, *x),
                        scale_value(prepared.y_axis_scale, *y)
                    ))
                    .collect::<Vec<_>>(),
                prepared.x_axis_scale,
                prepared.y_axis_scale
            )
        } else {
            match (
                prepared.x_axis_scale == ChartAxisScale::Logarithmic
                    && valid_log_range(prepared.plot_x_min, prepared.plot_x_max),
                prepared.y_axis_scale == ChartAxisScale::Logarithmic
                    && valid_log_range(prepared.y_min, prepared.y_max),
            ) {
                (false, false) => draw_line_chart!(
                    prepared.plot_x_min..prepared.plot_x_max,
                    prepared.y_min..prepared.y_max,
                    |series: &super::super::PreparedLineChartSeries| series.points.clone(),
                    ChartAxisScale::Linear,
                    ChartAxisScale::Linear
                ),
                (true, false) => draw_line_chart!(
                    (prepared.plot_x_min..prepared.plot_x_max).log_scale(),
                    prepared.y_min..prepared.y_max,
                    |series: &super::super::PreparedLineChartSeries| series.points.clone(),
                    ChartAxisScale::Linear,
                    ChartAxisScale::Linear
                ),
                (false, true) => draw_line_chart!(
                    prepared.plot_x_min..prepared.plot_x_max,
                    (prepared.y_min..prepared.y_max).log_scale(),
                    |series: &super::super::PreparedLineChartSeries| series.points.clone(),
                    ChartAxisScale::Linear,
                    ChartAxisScale::Linear
                ),
                (true, true) => draw_line_chart!(
                    (prepared.plot_x_min..prepared.plot_x_max).log_scale(),
                    (prepared.y_min..prepared.y_max).log_scale(),
                    |series: &super::super::PreparedLineChartSeries| series.points.clone(),
                    ChartAxisScale::Linear,
                    ChartAxisScale::Linear
                ),
            }
        }
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
    prepared: &PreparedHistogramData,
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
        let layout = line_chart_layout(
            request.width,
            request.height,
            axis_label_area_size(&[0.0, prepared.count_max], 30),
        );
        let x_scale = prepared.x_axis_scale;
        let x_label_scale = if x_scale == ChartAxisScale::Logarithmic {
            ChartAxisScale::Linear
        } else {
            x_scale
        };
        macro_rules! draw_histogram {
            ($x:expr) => {
                plotters::prelude::ChartBuilder::on(&root)
                    .margin(layout.margin)
                    .x_label_area_size(layout.x_label_area_size)
                    .y_label_area_size(layout.y_label_area_size)
                    .build_cartesian_2d($x, 0.0..prepared.count_max)
            };
        }
        macro_rules! finish_histogram {
            ($chart:expr) => {{
                let mut chart = match $chart {
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
                let boundaries = prepared
                    .series
                    .first()
                    .into_iter()
                    .flat_map(|series| {
                        series
                            .bins
                            .iter()
                            .map(|bin| bin.start)
                            .chain(series.bins.last().map(|bin| bin.end))
                    })
                    .collect::<Vec<_>>();
                let tick_positions = histogram_tick_indices(
                    boundaries.len(),
                    (request.width / 110).clamp(2, 7) as usize,
                )
                .into_iter()
                .map(|index| {
                    let value = boundaries[index];
                    let coordinate = if x_scale == ChartAxisScale::SymLog {
                        symlog(value)
                    } else {
                        value
                    };
                    (
                        chart.backend_coord(&(coordinate, 0.0)),
                        axis_label(x_label_scale, coordinate),
                    )
                })
                .collect::<Vec<_>>();
                if let Err(error) = chart
                    .configure_mesh()
                    .x_desc(format!(
                        "visible distribution ({} bins)",
                        prepared.bin_count
                    ))
                    .y_desc("count")
                    .disable_x_mesh()
                    .x_labels(0)
                    .y_label_formatter(&|value| format_axis_number(*value))
                    .y_label_style(
                        ("sans-serif", layout.y_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
                    .x_label_style(
                        ("sans-serif", layout.x_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
                    .axis_style(ShapeStyle::from(&axis).stroke_width(2))
                    .light_line_style(grid.mix(0.35))
                    .bold_line_style(grid.mix(0.55))
                    .draw()
                {
                    log_error(&error);
                }
                for series in prepared
                    .series
                    .iter()
                    .filter(|series| !series.is_selected)
                    .chain(prepared.series.iter().filter(|series| series.is_selected))
                {
                    let (r, g, b) = configure::rgb_channels(configure::themed_color(|colors| {
                        colors.chart.series[series.color_slot % colors.chart.series.len()]
                    }));
                    let color = RGBColor(r, g, b);
                    let fill_opacity = if series.is_selected { 0.45 } else { 0.16 };
                    let outline =
                        ShapeStyle::from(&color.mix(if series.is_selected { 0.95 } else { 0.5 }))
                            .stroke_width(if series.is_selected { 3 } else { 1 });
                    let drawn_series = match chart.draw_series(series.bins.iter().map(|bin| {
                        plotters::prelude::Rectangle::new(
                            [
                                (
                                    if x_scale == ChartAxisScale::SymLog {
                                        symlog(bin.start)
                                    } else {
                                        bin.start
                                    },
                                    0.0,
                                ),
                                (
                                    if x_scale == ChartAxisScale::SymLog {
                                        symlog(bin.end)
                                    } else {
                                        bin.end
                                    },
                                    bin.count,
                                ),
                            ],
                            color
                                .mix(if bin.is_selected { 0.78 } else { fill_opacity })
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
                                color.mix(fill_opacity).filled(),
                            )
                        });
                    if let Err(error) = chart.draw_series(series.bins.iter().map(|bin| {
                        plotters::prelude::Rectangle::new(
                            [
                                (
                                    if x_scale == ChartAxisScale::SymLog {
                                        symlog(bin.start)
                                    } else {
                                        bin.start
                                    },
                                    0.0,
                                ),
                                (
                                    if x_scale == ChartAxisScale::SymLog {
                                        symlog(bin.end)
                                    } else {
                                        bin.end
                                    },
                                    bin.count,
                                ),
                            ],
                            if bin.is_selected {
                                ShapeStyle::from(&color).stroke_width(4)
                            } else {
                                outline
                            },
                        )
                    })) {
                        log_error(&error);
                    }
                }
                if let Err(error) = chart
                    .configure_series_labels()
                    .background_style(plot_bg.mix(0.85))
                    .border_style(axis.mix(0.8))
                    .label_font(
                        ("sans-serif", layout.y_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
                    .draw()
                {
                    log_error(&error);
                }
                drop(chart);
                for ((x, y), label) in tick_positions {
                    if let Err(error) = root.draw(&plotters::element::PathElement::new(
                        vec![(x, y), (x, y + 5)],
                        ShapeStyle::from(&axis).stroke_width(2),
                    )) {
                        log_error(&error);
                    }
                    if let Err(error) = root.draw(&Text::new(
                        label,
                        (x, y + 5),
                        ("sans-serif", layout.x_label_font_size)
                            .into_font()
                            .color(&axis)
                            .pos(Pos::new(HPos::Center, VPos::Top)),
                    )) {
                        log_error(&error);
                    }
                }
                ranges
            }};
        }
        if prepared.x_axis_scale == ChartAxisScale::SymLog {
            finish_histogram!(draw_histogram!(
                symlog(prepared.value_min)..symlog(prepared.value_max)
            ))
        } else if prepared.x_axis_scale == ChartAxisScale::Logarithmic
            && valid_log_range(prepared.value_min, prepared.value_max)
        {
            finish_histogram!(draw_histogram!(
                (prepared.value_min..prepared.value_max).log_scale()
            ))
        } else {
            finish_histogram!(draw_histogram!(prepared.value_min..prepared.value_max))
        }
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

fn render_box_plot_request(
    request: &MultiChartRenderRequest,
    prepared: &PreparedBoxPlotData,
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
        let x_max = prepared.series.len().max(1) as f64 + 0.5;
        let y_scale = prepared.y_axis_scale;
        let y_label_scale = if y_scale == ChartAxisScale::Logarithmic {
            ChartAxisScale::Linear
        } else {
            y_scale
        };
        let layout = raster_chart_layout(
            request.width,
            request.height,
            RasterChartLayoutHints {
                preferred_margin: 12,
                preferred_x_label_area_size: axis_title_label_area_size(16).max(60),
                preferred_y_label_area_size: axis_label_area_size(
                    &[prepared.value_min, prepared.value_max],
                    30,
                ),
                preferred_x_label_font_size: 16,
                preferred_y_label_font_size: 18,
                min_plot_width: 48,
                min_plot_height: 40,
            },
        );
        macro_rules! draw_box_plot {
            ($y:expr) => {
                plotters::prelude::ChartBuilder::on(&root)
                    .margin(layout.margin)
                    .x_label_area_size(layout.x_label_area_size)
                    .y_label_area_size(layout.y_label_area_size)
                    .build_cartesian_2d(0.5..x_max, $y)
            };
        }
        macro_rules! finish_box_plot {
            ($chart:expr) => {{
                let mut chart = match $chart {
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
                let labels = prepared
                    .series
                    .iter()
                    .map(|series| series.label.clone())
                    .collect::<Vec<_>>();
                if let Err(error) = chart
                    .configure_mesh()
                    .x_desc("visible series")
                    .y_desc("value")
                    .x_labels(labels.len().max(1))
                    .disable_x_mesh()
                    .x_label_formatter(&move |value| {
                        let index = value.round() as isize - 1;
                        if index < 0 || index as usize >= labels.len() {
                            String::new()
                        } else {
                            labels[index as usize].clone()
                        }
                    })
                    .y_label_formatter(&|value| axis_label(y_label_scale, *value))
                    .y_label_style(
                        ("sans-serif", layout.y_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
                    .x_label_style(
                        ("sans-serif", layout.x_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
                    .axis_style(ShapeStyle::from(&axis).stroke_width(2))
                    .light_line_style(grid.mix(0.35))
                    .bold_line_style(grid.mix(0.55))
                    .draw()
                {
                    log_error(&error);
                }
                let y_span = (prepared.value_max - prepared.value_min).abs().max(1.0);
                for series in &prepared.series {
                    let transform = |value| {
                        if y_scale == ChartAxisScale::SymLog {
                            symlog(value)
                        } else {
                            value
                        }
                    };
                    let (r, g, b) = configure::rgb_channels(configure::themed_color(|colors| {
                        colors.chart.series[series.color_slot % colors.chart.series.len()]
                    }));
                    let color = RGBColor(r, g, b);
                    let x = series.x_index as f64 + 1.0;
                    let half_width = 0.28_f64;
                    let whisker_width = if series.is_selected { 3 } else { 2 };
                    let box_outline_width = if series.is_selected { 4 } else { 2 };
                    let median_width = if series.is_selected { 4 } else { 3 };
                    let outlier_radius = if series.is_selected { 5 } else { 4 };
                    let box_fill = color
                        .mix(if series.is_selected { 0.38 } else { 0.24 })
                        .filled();
                    let whisker_style =
                        ShapeStyle::from(&color.mix(0.82)).stroke_width(whisker_width);
                    let box_outline =
                        ShapeStyle::from(&color.mix(0.98)).stroke_width(box_outline_width);
                    let median_style = ShapeStyle::from(&axis.mix(0.98)).stroke_width(median_width);
                    let spine_style = ShapeStyle::from(&axis.mix(0.28)).stroke_width(1);
                    let cap_style = ShapeStyle::from(&color.mix(0.72)).stroke_width(whisker_width);
                    let box_height = (transform(series.q3) - transform(series.q1)).abs();
                    let corner_x = half_width * 0.24;
                    let corner_y = (box_height * 0.22).min(y_span * 0.018);
                    let use_chamfered_box = corner_y > f64::EPSILON && box_height > corner_y * 2.0;

                    let _ =
                        chart.draw_series(std::iter::once(plotters::element::PathElement::new(
                            vec![
                                (x, transform(series.whisker_low)),
                                (x, transform(series.q1)),
                            ],
                            whisker_style,
                        )));
                    let _ =
                        chart.draw_series(std::iter::once(plotters::element::PathElement::new(
                            vec![
                                (x, transform(series.q3)),
                                (x, transform(series.whisker_high)),
                            ],
                            whisker_style,
                        )));
                    let _ =
                        chart.draw_series(std::iter::once(plotters::element::PathElement::new(
                            vec![
                                (x - half_width / 2.0, transform(series.whisker_low)),
                                (x + half_width / 2.0, transform(series.whisker_low)),
                            ],
                            cap_style,
                        )));
                    let _ =
                        chart.draw_series(std::iter::once(plotters::element::PathElement::new(
                            vec![
                                (x - half_width / 2.0, transform(series.whisker_high)),
                                (x + half_width / 2.0, transform(series.whisker_high)),
                            ],
                            cap_style,
                        )));
                    if use_chamfered_box {
                        let box_points = vec![
                            (x - half_width + corner_x, transform(series.q1)),
                            (x + half_width - corner_x, transform(series.q1)),
                            (x + half_width, transform(series.q1) + corner_y),
                            (x + half_width, transform(series.q3) - corner_y),
                            (x + half_width - corner_x, transform(series.q3)),
                            (x - half_width + corner_x, transform(series.q3)),
                            (x - half_width, transform(series.q3) - corner_y),
                            (x - half_width, transform(series.q1) + corner_y),
                        ];
                        let _ = chart.draw_series(std::iter::once(
                            plotters::element::Polygon::new(box_points.clone(), box_fill),
                        ));
                        let mut box_outline_points = box_points;
                        box_outline_points.push(box_outline_points[0]);
                        let _ = chart.draw_series(std::iter::once(
                            plotters::element::PathElement::new(box_outline_points, box_outline),
                        ));
                    } else {
                        let _ =
                            chart.draw_series(std::iter::once(plotters::prelude::Rectangle::new(
                                [
                                    (x - half_width, transform(series.q1)),
                                    (x + half_width, transform(series.q3)),
                                ],
                                box_fill,
                            )));
                        let _ =
                            chart.draw_series(std::iter::once(plotters::prelude::Rectangle::new(
                                [
                                    (x - half_width, transform(series.q1)),
                                    (x + half_width, transform(series.q3)),
                                ],
                                box_outline,
                            )));
                    }
                    let _ =
                        chart.draw_series(std::iter::once(plotters::element::PathElement::new(
                            vec![(x, transform(series.q1)), (x, transform(series.q3))],
                            spine_style,
                        )));
                    let _ =
                        chart.draw_series(std::iter::once(plotters::element::PathElement::new(
                            vec![
                                (x - half_width, transform(series.median)),
                                (x + half_width, transform(series.median)),
                            ],
                            median_style,
                        )));
                    let _ = chart.draw_series(series.outliers.iter().map(|outlier| {
                        plotters::element::Circle::new(
                            (x, transform(*outlier)),
                            outlier_radius,
                            plot_bg.filled(),
                        )
                    }));
                    let _ = chart.draw_series(series.outliers.iter().map(|outlier| {
                        plotters::element::Circle::new(
                            (x, transform(*outlier)),
                            outlier_radius,
                            ShapeStyle::from(&color).stroke_width(2),
                        )
                    }));
                }
                ranges
            }};
        }
        if prepared.y_axis_scale == ChartAxisScale::SymLog {
            finish_box_plot!(draw_box_plot!(
                symlog(prepared.value_min)..symlog(prepared.value_max)
            ))
        } else if prepared.y_axis_scale == ChartAxisScale::Logarithmic
            && valid_log_range(prepared.value_min, prepared.value_max)
        {
            finish_box_plot!(draw_box_plot!(
                (prepared.value_min..prepared.value_max).log_scale()
            ))
        } else {
            finish_box_plot!(draw_box_plot!(prepared.value_min..prepared.value_max))
        }
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
    prepared: &PreparedComparisonScatterData,
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
        let x_scale = prepared.x_axis_scale;
        let y_scale = prepared.y_axis_scale;
        let (x_label_scale, y_label_scale) =
            if x_scale == ChartAxisScale::SymLog || y_scale == ChartAxisScale::SymLog {
                (x_scale, y_scale)
            } else {
                (ChartAxisScale::Linear, ChartAxisScale::Linear)
            };
        if let Err(error) = root.fill(&plot_bg) {
            log_error(&error);
            return MultiChartRenderResult::Failure {
                generation: request.generation,
                message: error.to_string(),
            };
        }
        let layout = raster_chart_layout(
            request.width,
            request.height,
            RasterChartLayoutHints {
                preferred_margin: 10,
                preferred_x_label_area_size: axis_title_label_area_size(18),
                preferred_y_label_area_size: axis_label_area_size(
                    &[prepared.y_min, prepared.y_max],
                    30,
                ),
                preferred_x_label_font_size: 18,
                preferred_y_label_font_size: 18,
                min_plot_width: 48,
                min_plot_height: 40,
            },
        );
        macro_rules! draw_scatter {
            ($x:expr, $y:expr) => {
                plotters::prelude::ChartBuilder::on(&root)
                    .margin(layout.margin)
                    .x_label_area_size(layout.x_label_area_size)
                    .y_label_area_size(layout.y_label_area_size)
                    .build_cartesian_2d($x, $y)
            };
        }
        macro_rules! finish_scatter {
            ($chart:expr) => {{
                let mut chart = match $chart {
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
                    .x_label_formatter(&|value| axis_label(x_label_scale, *value))
                    .y_label_formatter(&|value| axis_label(y_label_scale, *value))
                    .y_label_style(
                        ("sans-serif", layout.y_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
                    .x_label_style(
                        ("sans-serif", layout.x_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
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
                    vec![
                        (
                            scale_value(x_scale, diagonal_min),
                            scale_value(y_scale, diagonal_min),
                        ),
                        (
                            scale_value(x_scale, diagonal_max),
                            scale_value(y_scale, diagonal_max),
                        ),
                    ],
                    axis.mix(0.4).stroke_width(2),
                )));
                let points = prepared
                    .points
                    .iter()
                    .map(|(x, y)| (scale_value(x_scale, *x), scale_value(y_scale, *y)))
                    .collect::<Vec<_>>();
                let drawn_series =
                    match chart.draw_series(plotters::prelude::PointSeries::of_element(
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
                    .legend(move |(x, y)| {
                        plotters::element::Circle::new((x + 10, y), 4, color.filled())
                    });
                if let Err(error) = chart
                    .configure_series_labels()
                    .background_style(plot_bg.mix(0.85))
                    .border_style(axis.mix(0.8))
                    .label_font(
                        ("sans-serif", layout.y_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
                    .draw()
                {
                    log_error(&error);
                }
                ranges
            }};
        }
        if prepared.x_axis_scale == ChartAxisScale::SymLog
            || prepared.y_axis_scale == ChartAxisScale::SymLog
        {
            finish_scatter!(draw_scatter!(
                scale_value(prepared.x_axis_scale, prepared.x_min)
                    ..scale_value(prepared.x_axis_scale, prepared.x_max),
                scale_value(prepared.y_axis_scale, prepared.y_min)
                    ..scale_value(prepared.y_axis_scale, prepared.y_max)
            ))
        } else {
            match (
                prepared.x_axis_scale == ChartAxisScale::Logarithmic
                    && valid_log_range(prepared.x_min, prepared.x_max),
                prepared.y_axis_scale == ChartAxisScale::Logarithmic
                    && valid_log_range(prepared.y_min, prepared.y_max),
            ) {
                (false, false) => finish_scatter!(draw_scatter!(
                    prepared.x_min..prepared.x_max,
                    prepared.y_min..prepared.y_max
                )),
                (true, false) => finish_scatter!(draw_scatter!(
                    (prepared.x_min..prepared.x_max).log_scale(),
                    prepared.y_min..prepared.y_max
                )),
                (false, true) => finish_scatter!(draw_scatter!(
                    prepared.x_min..prepared.x_max,
                    (prepared.y_min..prepared.y_max).log_scale()
                )),
                (true, true) => finish_scatter!(draw_scatter!(
                    (prepared.x_min..prepared.x_max).log_scale(),
                    (prepared.y_min..prepared.y_max).log_scale()
                )),
            }
        }
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

pub(crate) fn render_prepared_chart_request(
    request: MultiChartRenderRequest,
) -> MultiChartRenderResult {
    match &request.prepared {
        PreparedChartData::Line(prepared) => render_line_chart_request(&request, prepared),
        PreparedChartData::Histogram(prepared) => render_histogram_request(&request, prepared),
        PreparedChartData::BoxPlot(prepared) => render_box_plot_request(&request, prepared),
        PreparedChartData::ComparisonScatter(prepared) => {
            render_comparison_scatter_request(&request, prepared)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{axis_label, scale_value};
    use crate::ui::mchart::ChartAxisScale;

    #[test]
    fn manual_log_transform_uses_natural_log_and_raw_labels() {
        let raw = 100.0;
        let transformed = scale_value(ChartAxisScale::Logarithmic, raw);
        assert!((transformed - raw.ln()).abs() < f64::EPSILON);
        assert_eq!(axis_label(ChartAxisScale::Logarithmic, transformed), "100");
    }
}
