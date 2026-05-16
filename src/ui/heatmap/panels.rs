use image::{DynamicImage, RgbImage};
use plotters::{
    prelude::{
        BitMapBackend, ChartBuilder, IntoDrawingArea, PathElement, Rectangle as PlotRectangle,
    },
    style::{Color, IntoFont, RGBColor, ShapeStyle},
};
use ratatui::{
    layout::{Constraint, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};
use ratatui_image::StatefulImage;

use crate::{
    configure,
    error::AppError,
    h5f::{DatasetMeta, H5FNode},
    ui::state::{
        AppState, HeatmapColormap, HeatmapLegendSummary, HeatmapNormalization, HeatmapRangeMode,
    },
};

use super::render::{apply_invert_colors, heatmap_colormap_rgb, normalize_heatmap_value};

struct HeatmapProfilePlotStyle {
    x_max: f64,
    y_min: f64,
    y_max: f64,
    value_min: f64,
    value_max: f64,
    colormap: HeatmapColormap,
    invert_colors: bool,
    normalization: HeatmapNormalization,
}

pub(super) fn heatmap_frame_inner(area: &Rect) -> Rect {
    area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    })
}

pub(super) fn render_heatmap_frame(f: &mut Frame, area: &Rect, loading: bool) -> Rect {
    let heatmap_block = Block::default()
        .title(if loading {
            " # Heatmap * "
        } else {
            " # Heatmap "
        })
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.surface.panel_title))
                .bold(),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(configure::themed_color(|colors| {
            colors.surface.panel_border
        })));
    let inner = heatmap_frame_inner(area);
    f.render_widget(heatmap_block, *area);
    inner
}

pub(super) fn render_heatmap_sidebar(
    f: &mut Frame,
    area: &Rect,
    state: &AppState,
    legend_summary: &HeatmapLegendSummary,
) -> Result<(), AppError> {
    render_heatmap_legend_histogram(
        f,
        area,
        state,
        legend_summary,
        state.heatmap_render.settings.colormap,
        state.heatmap_render.settings.invert_c,
    );
    Ok(())
}

pub(super) fn render_heatmap_settings(f: &mut Frame, area: &Rect, state: &mut AppState) {
    let block = Block::default()
        .title(" * Settings ")
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.surface.panel_title))
                .bold(),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(configure::themed_color(|colors| {
            colors.surface.panel_border
        })));
    let inner = block.inner(*area);
    f.render_widget(block, *area);
    state.ui_layout.heatmap_settings.clear();
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let settings = &state.heatmap_render.settings;
    let rows = [
        (
            "Color".to_string(),
            heatmap_colormap_label(settings.colormap).to_string(),
        ),
        ("Range".to_string(), heatmap_range_label(&settings.range)),
        (
            "InvertX".to_string(),
            if settings.invert_x { "Yes" } else { "No" }.to_string(),
        ),
        (
            "InvertY".to_string(),
            if settings.invert_y { "Yes" } else { "No" }.to_string(),
        ),
        (
            "InvertC".to_string(),
            if settings.invert_c { "Yes" } else { "No" }.to_string(),
        ),
        (
            "Norm".to_string(),
            heatmap_normalization_label(settings.normalization).to_string(),
        ),
    ];
    let highlight_bg = configure::themed_color(|colors| colors.surface.highlight_bg);
    for idx in 0..rows.len().min(inner.height as usize) {
        state
            .ui_layout
            .heatmap_settings
            .push(crate::ui::state::HeatmapSettingHitbox {
                area: Rect {
                    x: inner.x,
                    y: inner.y + idx as u16,
                    width: inner.width,
                    height: 1,
                },
                setting: idx,
            });
    }
    let lines = rows
        .iter()
        .enumerate()
        .map(|(idx, (label, value))| {
            let is_selected = idx == state.heatmap_render.selected_setting;
            let value_style = if is_selected {
                Style::default()
                    .bg(highlight_bg)
                    .fg(configure::themed_color(|colors| colors.text.primary))
                    .bold()
            } else {
                Style::default().fg(configure::themed_color(|colors| colors.text.primary))
            };
            let label_style = if is_selected {
                Style::default()
                    .bg(highlight_bg)
                    .fg(configure::themed_color(|colors| colors.file.label))
                    .bold()
            } else {
                Style::default()
                    .fg(configure::themed_color(|colors| colors.file.label))
                    .bold()
            };
            Line::from(vec![
                Span::styled(format!("{label:<7}"), label_style),
                Span::styled(format!(" {value}"), value_style),
            ])
        })
        .collect::<Vec<_>>();
    f.render_widget(Paragraph::new(lines), inner);
}

fn heatmap_colormap_label(value: HeatmapColormap) -> &'static str {
    value.label()
}

fn heatmap_range_label(value: &HeatmapRangeMode) -> String {
    value.label()
}

fn heatmap_normalization_label(value: HeatmapNormalization) -> &'static str {
    value.label()
}

pub(super) fn render_heatmap_region_panel(
    f: &mut Frame,
    area: &Rect,
    attr: &DatasetMeta,
    node: &H5FNode,
    state: &AppState,
) {
    let Some(viewport) = state.heatmap_viewport_region.as_ref() else {
        return;
    };
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "View ",
                Style::default()
                    .fg(configure::themed_color(|colors| colors.file.label))
                    .bold(),
            ),
            Span::styled(
                format!(
                    "x={} y={} w={} h={}  ",
                    viewport.x, viewport.y, viewport.width, viewport.height
                ),
                Style::default().fg(configure::themed_color(|colors| colors.text.primary)),
            ),
            Span::styled(
                viewport.value_summary(),
                Style::default().fg(configure::themed_color(|colors| colors.help.description)),
            ),
        ]),
        Line::from(match state.heatmap_region.as_ref() {
            Some(region) if state.heatmap_render.selected_cells.is_some() => vec![
                Span::styled(
                    "Sel  ",
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.file.label))
                        .bold(),
                ),
                Span::styled(
                    format!(
                        "x={} y={} w={} h={}  ",
                        region.x, region.y, region.width, region.height
                    ),
                    Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)),
                ),
                Span::styled(
                    region.value_summary(),
                    Style::default().fg(configure::themed_color(|colors| colors.help.description)),
                ),
            ],
            _ => vec![
                Span::styled(
                    "Sel  ",
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.file.label))
                        .bold(),
                ),
                Span::styled(
                    "none",
                    Style::default().fg(configure::themed_color(|colors| colors.help.muted)),
                ),
            ],
        }),
    ];
    lines.push(Line::from(vec![Span::styled(
        format!(
            "Dims Y={} X={}  slice {}x{}",
            node.selected_row,
            node.selected_col,
            attr.shape[node.selected_row],
            attr.shape[node.selected_col]
        ),
        Style::default()
            .fg(configure::themed_color(|colors| colors.file.label))
            .bold(),
    )]));
    if let Some(window) = state.heatmap_render.page_window.as_ref() {
        let (start, end) = window.current_range();
        lines.push(Line::from(Span::styled(
            format!(
                "page {}/{}  {}={}..{}",
                window.page + 1,
                window.page_count.max(1),
                window.label(),
                start,
                end.saturating_sub(1)
            ),
            Style::default()
                .fg(configure::themed_color(|colors| colors.file.section_title))
                .bold(),
        )));
    }
    if let Some(profile) = state.heatmap_render.current_line_profile.as_ref() {
        lines.push(Line::from(vec![
            Span::styled(
                "Line ",
                Style::default()
                    .fg(configure::themed_color(|colors| colors.file.label))
                    .bold(),
            ),
            Span::styled(
                format!(
                    "x={} y={} -> x={} y={}",
                    profile.start_x, profile.start_y, profile.end_x, profile.end_y
                ),
                Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled(
                "Prof ",
                Style::default()
                    .fg(configure::themed_color(|colors| colors.file.label))
                    .bold(),
            ),
            Span::styled(
                format!(
                    "n={} finite={} mean={:.5}",
                    profile.sample_count, profile.finite_count, profile.mean
                ),
                Style::default().fg(configure::themed_color(|colors| colors.help.description)),
            ),
        ]));
    }
    let block = Block::default()
        .title(" [] Regions ")
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.surface.panel_title))
                .bold(),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(configure::themed_color(|colors| {
            colors.surface.panel_border
        })));
    f.render_widget(Paragraph::new(lines).block(block), *area);
}

pub(super) fn render_heatmap_profile_slot(
    f: &mut Frame,
    area: &Rect,
    state: &AppState,
) -> Result<(), AppError> {
    if state.heatmap_render.current_line_profile.is_some() {
        if let Some(legend_summary) = state.heatmap_render.current_legend_summary.as_ref() {
            return render_heatmap_profile_panel(f, area, state, legend_summary);
        }
    }

    let block = Block::default()
        .title(" / Profile ")
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.surface.panel_title))
                .bold(),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(configure::themed_color(|colors| {
            colors.surface.panel_border
        })));
    f.render_widget(block, *area);
    Ok(())
}

fn render_heatmap_profile_panel(
    f: &mut Frame,
    area: &Rect,
    state: &AppState,
    legend_summary: &HeatmapLegendSummary,
) -> Result<(), AppError> {
    let block = Block::default()
        .title(" / Profile ")
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.surface.panel_title))
                .bold(),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(configure::themed_color(|colors| {
            colors.surface.panel_border
        })));
    let inner = block.inner(*area);
    f.render_widget(block, *area);
    let Some(profile) = state.heatmap_render.current_line_profile.as_ref() else {
        return Ok(());
    };
    if inner.width < 4 || inner.height < 3 {
        return Ok(());
    }
    let points = profile
        .samples
        .iter()
        .filter(|sample| sample.value.is_finite())
        .map(|sample| (sample.distance, sample.value))
        .collect::<Vec<_>>();
    if points.is_empty() {
        f.render_widget(Paragraph::new("no finite samples"), inner);
        return Ok(());
    }

    let mut x_max = points.last().map(|(distance, _)| *distance).unwrap_or(1.0);
    if x_max <= 0.0 {
        x_max = 1.0;
    }
    let mut y_min = profile.min;
    let mut y_max = profile.max;
    if !y_min.is_finite() || !y_max.is_finite() {
        y_min = 0.0;
        y_max = 1.0;
    } else if (y_max - y_min).abs() < f64::EPSILON {
        let margin = y_min.abs().max(1.0) * 0.1;
        y_min -= margin;
        y_max += margin;
    }
    let pixel_width = u32::from(inner.width.max(1)) * u32::from(state.image_cell_size.0.max(1));
    let pixel_height = u32::from(inner.height.max(1)) * u32::from(state.image_cell_size.1.max(1));
    let mut buffer = vec![0u8; (pixel_width * pixel_height * 3) as usize];
    render_heatmap_profile_plot(
        &mut buffer,
        pixel_width,
        pixel_height,
        &points,
        HeatmapProfilePlotStyle {
            x_max,
            y_min,
            y_max,
            value_min: legend_summary.min,
            value_max: legend_summary.max,
            colormap: state.heatmap_render.settings.colormap,
            invert_colors: state.heatmap_render.settings.invert_c,
            normalization: state.heatmap_render.settings.normalization,
        },
    )?;
    if let Some(image) = RgbImage::from_raw(pixel_width, pixel_height, buffer) {
        let dyn_img = DynamicImage::ImageRgb8(image);
        let mut protocol = state.multi_chart.picker.new_resize_protocol(dyn_img);
        f.render_stateful_widget(StatefulImage::default(), inner, &mut protocol);
    }
    Ok(())
}

fn render_heatmap_profile_plot(
    buffer: &mut [u8],
    width: u32,
    height: u32,
    points: &[(f64, f64)],
    style: HeatmapProfilePlotStyle,
) -> Result<(), AppError> {
    let (bg_r, bg_g, bg_b) =
        configure::rgb_channels(configure::themed_color(|colors| colors.chart.plot_bg));
    let (grid_r, grid_g, grid_b) =
        configure::rgb_channels(configure::themed_color(|colors| colors.chart.grid));
    let (axis_r, axis_g, axis_b) =
        configure::rgb_channels(configure::themed_color(|colors| colors.chart.axis));
    let plot_bg = RGBColor(bg_r, bg_g, bg_b);
    let grid = RGBColor(grid_r, grid_g, grid_b);
    let axis = RGBColor(axis_r, axis_g, axis_b);

    let root = BitMapBackend::with_buffer(buffer, (width, height)).into_drawing_area();
    root.fill(&plot_bg)
        .map_err(|e| AppError::DrawingError(format!("Error filling profile background: {e}")))?;
    let y_label_area_size =
        format!("{:.4}", style.y_max.abs().max(style.y_min.abs())).len() as u32 * 4 + 28;

    let mut chart = ChartBuilder::on(&root)
        .margin(8)
        .x_label_area_size(22)
        .y_label_area_size(y_label_area_size)
        .build_cartesian_2d(0.0..style.x_max, style.y_min..style.y_max)
        .map_err(|e| AppError::DrawingError(format!("Error building profile chart: {e}")))?;

    chart
        .configure_mesh()
        .disable_x_mesh()
        .y_labels(3)
        .x_label_style(("sans-serif", 16).into_font().color(&axis))
        .y_label_style(("sans-serif", 16).into_font().color(&axis))
        .axis_style(ShapeStyle::from(&axis).stroke_width(2))
        .light_line_style(grid.mix(0.35))
        .bold_line_style(grid.mix(0.55))
        .draw()
        .map_err(|e| AppError::DrawingError(format!("Error drawing profile mesh: {e}")))?;

    chart
        .draw_series(points.windows(2).map(|window| {
            let value = (window[0].1 + window[1].1) * 0.5;
            let normalized = apply_invert_colors(
                normalize_heatmap_value(
                    value,
                    style.value_min,
                    style.value_max,
                    style.normalization,
                ),
                style.invert_colors,
            );
            let (r, g, b) = heatmap_colormap_rgb(normalized, style.colormap);
            PathElement::new(
                vec![window[0], window[1]],
                ShapeStyle::from(&RGBColor(r, g, b)).stroke_width(3),
            )
        }))
        .map_err(|e| AppError::DrawingError(format!("Error drawing profile series: {e}")))?;

    root.present()
        .map_err(|e| AppError::DrawingError(format!("Error presenting profile chart: {e}")))?;
    Ok(())
}

fn render_heatmap_legend_histogram(
    f: &mut Frame,
    area: &Rect,
    state: &AppState,
    summary: &HeatmapLegendSummary,
    colormap: HeatmapColormap,
    invert_colors: bool,
) {
    let block = Block::default()
        .title(" = Legend ")
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.surface.panel_title))
                .bold(),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(configure::themed_color(|colors| {
            colors.surface.panel_border
        })));
    let inner = block.inner(*area);
    f.render_widget(block, *area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    if !summary.has_finite {
        f.render_widget(Paragraph::new("no finite values"), inner);
        return;
    }
    let split = if inner.height >= 3 {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner)
    } else {
        Layout::vertical([Constraint::Min(1)]).split(inner)
    };
    if split.len() == 3 {
        f.render_widget(
            Paragraph::new(Line::from(format!("{:.4}", summary.max))).right_aligned(),
            split[0],
        );
        f.render_widget(
            Paragraph::new(Line::from(format!("{:.4}", summary.min))).right_aligned(),
            split[2],
        );
        render_heatmap_legend_plot(f, &split[1], state, summary, colormap, invert_colors);
    } else {
        render_heatmap_legend_plot(f, &split[0], state, summary, colormap, invert_colors);
    }
}

fn render_heatmap_legend_plot(
    f: &mut Frame,
    area: &Rect,
    state: &AppState,
    summary: &HeatmapLegendSummary,
    colormap: HeatmapColormap,
    invert_colors: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let pixel_width = u32::from(area.width.max(1)) * u32::from(state.image_cell_size.0.max(1));
    let pixel_height = u32::from(area.height.max(1)) * u32::from(state.image_cell_size.1.max(1));
    let mut buffer = vec![0u8; (pixel_width * pixel_height * 3) as usize];
    let (bg_r, bg_g, bg_b) =
        configure::rgb_channels(configure::themed_color(|colors| colors.chart.plot_bg));
    let (axis_r, axis_g, axis_b) =
        configure::rgb_channels(configure::themed_color(|colors| colors.chart.axis));
    {
        let root = BitMapBackend::with_buffer(&mut buffer, (pixel_width, pixel_height))
            .into_drawing_area();
        let _ = root.fill(&RGBColor(bg_r, bg_g, bg_b));
        let plot_height = pixel_height.max(1) as i32;
        let legend_width = (pixel_width as i32 / 3).max(10);
        let separator_x = legend_width + 4;
        let hist_start_x = separator_x + 4;
        let hist_width = (pixel_width as i32 - hist_start_x).max(1);

        for y in 0..plot_height {
            let norm = 1.0 - (f64::from(y) / f64::from(plot_height.max(1)));
            let (r, g, b) =
                heatmap_colormap_rgb(apply_invert_colors(norm, invert_colors), colormap);
            let _ = root.draw(&PlotRectangle::new(
                [(0, y), (legend_width, y + 1)],
                ShapeStyle::from(&RGBColor(r, g, b)).filled(),
            ));
        }
        let _ = root.draw(&PlotRectangle::new(
            [(separator_x, 0), (separator_x + 1, plot_height)],
            ShapeStyle::from(&RGBColor(axis_r, axis_g, axis_b)).filled(),
        ));

        if !summary.histogram.is_empty() {
            let max_count = summary.histogram.iter().copied().max().unwrap_or(0).max(1);
            for y in 0..plot_height {
                let norm = 1.0 - (f64::from(y) / f64::from(plot_height.max(1)));
                let (hist_r, hist_g, hist_b) =
                    heatmap_colormap_rgb(apply_invert_colors(norm, invert_colors), colormap);
                let idx = ((usize::try_from(plot_height - 1 - y).unwrap_or(0)
                    * summary.histogram.len())
                    / usize::try_from(plot_height.max(1)).unwrap_or(1))
                .min(summary.histogram.len().saturating_sub(1));
                let count = summary.histogram[idx];
                let bar_width =
                    ((count as f64 / max_count as f64) * f64::from(hist_width)).round() as i32;
                if bar_width <= 0 {
                    continue;
                }
                let _ = root.draw(&PlotRectangle::new(
                    [(hist_start_x, y), (hist_start_x + bar_width, y + 1)],
                    ShapeStyle::from(&RGBColor(hist_r, hist_g, hist_b)).filled(),
                ));
            }
        }
    }
    if let Some(image) = RgbImage::from_raw(pixel_width, pixel_height, buffer) {
        let dyn_img = DynamicImage::ImageRgb8(image);
        let mut protocol = state.multi_chart.picker.new_resize_protocol(dyn_img);
        f.render_stateful_widget(StatefulImage::default(), *area, &mut protocol);
    }
}
