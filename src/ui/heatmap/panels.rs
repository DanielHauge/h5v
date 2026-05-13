use image::{DynamicImage, RgbImage};
use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea, Rectangle as PlotRectangle},
    style::{RGBColor, ShapeStyle},
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
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

use super::render::heatmap_colormap_rgb;

pub(super) fn render_heatmap_frame(f: &mut Frame, area: &Rect) -> Rect {
    let heatmap_block = Block::default()
        .title(" # Heatmap ")
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
    let inner = heatmap_block.inner(*area);
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
    );
    Ok(())
}

pub(super) fn render_heatmap_loading(f: &mut Frame, area: &Rect) {
    f.render_widget(
        Paragraph::new("Seeking heatmap page...")
            .style(Style::default().fg(configure::themed_color(|colors| colors.help.description))),
        *area,
    );
}

pub(super) fn render_heatmap_loading_sidebar(f: &mut Frame, area: &Rect) -> Result<(), AppError> {
    f.render_widget(
        Paragraph::new("Loading legend...")
            .style(Style::default().fg(configure::themed_color(|colors| colors.help.description))),
        *area,
    );
    Ok(())
}

pub(super) fn render_heatmap_settings(f: &mut Frame, area: &Rect, state: &AppState) {
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
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let settings = &state.heatmap_render.settings;
    let rows = [
        ("Color", heatmap_colormap_label(settings.colormap)),
        ("Range", heatmap_range_label(settings.range)),
        ("InvertX", if settings.invert_x { "Yes" } else { "No" }),
        ("InvertY", if settings.invert_y { "Yes" } else { "No" }),
        ("Norm", heatmap_normalization_label(settings.normalization)),
    ];
    let highlight_bg = configure::themed_color(|colors| colors.surface.highlight_bg);
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
    match value {
        HeatmapColormap::Turbo => "Turbo",
        HeatmapColormap::Grayscale => "Gray",
        HeatmapColormap::Inferno => "Inferno",
    }
}

fn heatmap_range_label(value: HeatmapRangeMode) -> &'static str {
    match value {
        HeatmapRangeMode::Auto => "Auto",
        HeatmapRangeMode::Percentile1 => "1-99%",
    }
}

fn heatmap_normalization_label(value: HeatmapNormalization) -> &'static str {
    match value {
        HeatmapNormalization::Linear => "Linear",
        HeatmapNormalization::Log => "Log",
        HeatmapNormalization::Sqrt => "Sqrt",
    }
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
                format!("mean={:.5} std={:.5}", viewport.mean, viewport.stddev),
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
                    format!("mean={:.5} std={:.5}", region.mean, region.stddev),
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
    if let Some(window) = state.heatmap_render.segment.as_ref() {
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

fn render_heatmap_legend_histogram(
    f: &mut Frame,
    area: &Rect,
    state: &AppState,
    summary: &HeatmapLegendSummary,
    colormap: HeatmapColormap,
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
        render_heatmap_legend_plot(f, &split[1], state, summary, colormap);
    } else {
        render_heatmap_legend_plot(f, &split[0], state, summary, colormap);
    }
}

fn render_heatmap_legend_plot(
    f: &mut Frame,
    area: &Rect,
    state: &AppState,
    summary: &HeatmapLegendSummary,
    colormap: HeatmapColormap,
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
            let (r, g, b) = heatmap_colormap_rgb(norm, colormap);
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
                let (hist_r, hist_g, hist_b) = heatmap_colormap_rgb(norm, colormap);
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
