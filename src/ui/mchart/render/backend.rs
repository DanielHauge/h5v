use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea},
    style::{Color as _, IntoFont, RGBColor, ShapeStyle},
};

use crate::{configure, error::log_error};

use super::super::{
    MultiChartRenderRequest, MultiChartRenderResult, PreparedBoxPlotData, PreparedChartData,
    PreparedComparisonScatterData, PreparedHistogramData, PreparedLineChartData,
};

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
        let chart = plotters::prelude::ChartBuilder::on(&root)
            .margin(12)
            .x_label_area_size(60)
            .y_label_area_size(45)
            .build_cartesian_2d(0.5..x_max, prepared.value_min..prepared.value_max);
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
            .y_label_style(("sans-serif", 18).into_font().color(&axis))
            .x_label_style(("sans-serif", 16).into_font().color(&axis))
            .axis_style(ShapeStyle::from(&axis).stroke_width(2))
            .light_line_style(grid.mix(0.35))
            .bold_line_style(grid.mix(0.55))
            .draw()
        {
            log_error(&error);
        }
        let y_span = (prepared.value_max - prepared.value_min).abs().max(1.0);
        for series in &prepared.series {
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
            let whisker_style = ShapeStyle::from(&color.mix(0.82)).stroke_width(whisker_width);
            let box_outline = ShapeStyle::from(&color.mix(0.98)).stroke_width(box_outline_width);
            let median_style = ShapeStyle::from(&axis.mix(0.98)).stroke_width(median_width);
            let spine_style = ShapeStyle::from(&axis.mix(0.28)).stroke_width(1);
            let cap_style = ShapeStyle::from(&color.mix(0.72)).stroke_width(whisker_width);
            let box_height = (series.q3 - series.q1).abs();
            let corner_x = half_width * 0.24;
            let corner_y = (box_height * 0.22).min(y_span * 0.018);
            let use_chamfered_box = corner_y > f64::EPSILON && box_height > corner_y * 2.0;

            let _ = chart.draw_series(std::iter::once(plotters::element::PathElement::new(
                vec![(x, series.whisker_low), (x, series.q1)],
                whisker_style,
            )));
            let _ = chart.draw_series(std::iter::once(plotters::element::PathElement::new(
                vec![(x, series.q3), (x, series.whisker_high)],
                whisker_style,
            )));
            let _ = chart.draw_series(std::iter::once(plotters::element::PathElement::new(
                vec![
                    (x - half_width / 2.0, series.whisker_low),
                    (x + half_width / 2.0, series.whisker_low),
                ],
                cap_style,
            )));
            let _ = chart.draw_series(std::iter::once(plotters::element::PathElement::new(
                vec![
                    (x - half_width / 2.0, series.whisker_high),
                    (x + half_width / 2.0, series.whisker_high),
                ],
                cap_style,
            )));
            if use_chamfered_box {
                let box_points = vec![
                    (x - half_width + corner_x, series.q1),
                    (x + half_width - corner_x, series.q1),
                    (x + half_width, series.q1 + corner_y),
                    (x + half_width, series.q3 - corner_y),
                    (x + half_width - corner_x, series.q3),
                    (x - half_width + corner_x, series.q3),
                    (x - half_width, series.q3 - corner_y),
                    (x - half_width, series.q1 + corner_y),
                ];
                let _ = chart.draw_series(std::iter::once(plotters::element::Polygon::new(
                    box_points.clone(),
                    box_fill,
                )));
                let mut box_outline_points = box_points;
                box_outline_points.push(box_outline_points[0]);
                let _ = chart.draw_series(std::iter::once(plotters::element::PathElement::new(
                    box_outline_points,
                    box_outline,
                )));
            } else {
                let _ = chart.draw_series(std::iter::once(plotters::prelude::Rectangle::new(
                    [(x - half_width, series.q1), (x + half_width, series.q3)],
                    box_fill,
                )));
                let _ = chart.draw_series(std::iter::once(plotters::prelude::Rectangle::new(
                    [(x - half_width, series.q1), (x + half_width, series.q3)],
                    box_outline,
                )));
            }
            let _ = chart.draw_series(std::iter::once(plotters::element::PathElement::new(
                vec![(x, series.q1), (x, series.q3)],
                spine_style,
            )));
            let _ = chart.draw_series(std::iter::once(plotters::element::PathElement::new(
                vec![
                    (x - half_width, series.median),
                    (x + half_width, series.median),
                ],
                median_style,
            )));
            let _ = chart.draw_series(series.outliers.iter().map(|value| {
                plotters::element::Circle::new((x, *value), outlier_radius, plot_bg.filled())
            }));
            let _ = chart.draw_series(series.outliers.iter().map(|value| {
                plotters::element::Circle::new(
                    (x, *value),
                    outlier_radius,
                    ShapeStyle::from(&color).stroke_width(2),
                )
            }));
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
