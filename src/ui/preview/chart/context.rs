use ratatui::{
    layout::{Constraint, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::{
    configure,
    data::{DatasetPlotingData, SliceSelection},
    h5f::H5FNode,
    ui::{
        chart_math::{
            axis_label_area_size, normalized_axis_bounds, raster_chart_layout, RasterChartLayout,
            RasterChartLayoutHints,
        },
        mchart::chart_plot_area_in_rect,
        mchart::ChartAxisScale,
        page_scroll::{compact_count, PageDisplayInfo},
        state::{
            AppState, PreviewAxisScaleHitbox, PreviewChartRoi, PreviewChartViewport,
            PREVIEW_CHART_VISIBLE_POINT_LIMIT,
        },
    },
};

use super::MAX_PAGE_SIZE;

fn req_matches_paged_chart(page_state: &crate::ui::state::PageState) -> bool {
    matches!(page_state.paged, crate::ui::state::PageType::Chart) && page_state.idx > 0
}

pub(super) fn preview_chart_layout(
    width_px: u32,
    height_px: u32,
    max_value: f64,
) -> RasterChartLayout {
    let y_label_area_size = axis_label_area_size(&[max_value], 30);
    raster_chart_layout(
        width_px,
        height_px,
        RasterChartLayoutHints {
            preferred_margin: 10,
            preferred_x_label_area_size: 30,
            preferred_y_label_area_size: y_label_area_size,
            preferred_x_label_font_size: 18,
            preferred_y_label_font_size: 18,
            min_plot_width: 48,
            min_plot_height: 40,
        },
    )
}

pub(super) fn preview_x_min(page_state: &crate::ui::state::PageState) -> f64 {
    if req_matches_paged_chart(page_state) {
        MAX_PAGE_SIZE as f64 * page_state.idx.max(0) as f64
    } else {
        0.0
    }
}

pub(crate) fn preview_effective_x_domain(
    data: &DatasetPlotingData,
    viewport: PreviewChartViewport,
    x_min: f64,
    logarithmic: bool,
) -> Option<(f64, f64)> {
    if !logarithmic {
        return Some((viewport.x_min, viewport.x_max));
    }
    let (start, end) = preview_visible_index_window(data, viewport, x_min)?;
    let (min, max) = data.data[start..=end]
        .iter()
        .map(|(x, _)| x_min + x)
        .filter(|x| x.is_finite() && *x > 0.0)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), x| {
            (min.min(x), max.max(x))
        });
    (min.is_finite() && max.is_finite()).then_some((min, max.max(min * 2.0)))
}

pub(crate) fn preview_x_from_ratio(domain: (f64, f64), logarithmic: bool, ratio: f64) -> f64 {
    let ratio = ratio.clamp(0.0, 1.0);
    if logarithmic {
        (domain.0.ln() + (domain.1.ln() - domain.0.ln()) * ratio).exp()
    } else {
        domain.0 + (domain.1 - domain.0) * ratio
    }
}

pub(crate) fn preview_x_ratio(domain: (f64, f64), logarithmic: bool, x: f64) -> f64 {
    if logarithmic {
        ((x.max(domain.0).min(domain.1).ln() - domain.0.ln()) / (domain.1.ln() - domain.0.ln()))
            .clamp(0.0, 1.0)
    } else {
        ((x - domain.0) / (domain.1 - domain.0)).clamp(0.0, 1.0)
    }
}

pub(crate) fn preview_chart_data_bounds(
    data_preview: &DatasetPlotingData,
    x_min: f64,
) -> Option<PreviewChartViewport> {
    let (x_min, x_max) = normalized_axis_bounds(x_min, x_min + preview_x_axis_max(data_preview))?;
    let (y_min, y_max) = normalized_axis_bounds(data_preview.min, data_preview.max)?;
    Some(PreviewChartViewport {
        x_min,
        x_max,
        y_min,
        y_max,
    })
}

pub(super) fn preview_view_info(
    state: &mut AppState<'_>,
    total_items: usize,
) -> Option<PageDisplayInfo<'static>> {
    if !state.chart_preview_state.has_explicit_viewport() {
        return None;
    }
    let viewport = state.chart_preview_state.viewport?;
    let range_start = viewport.x_min.floor().max(0.0) as usize;
    let range_end = (viewport.x_max.ceil().max(viewport.x_min) as usize)
        .saturating_add(1)
        .min(total_items.max(1));
    Some(PageDisplayInfo {
        title: "View",
        current: 0,
        total: 1,
        range_start,
        range_end: range_end.max(range_start.saturating_add(1).min(total_items.max(1))),
        total_items: total_items.max(1),
        unit: "pts",
    })
}

pub(super) fn copy_page_display_info(info: &PageDisplayInfo<'static>) -> PageDisplayInfo<'static> {
    PageDisplayInfo {
        title: info.title,
        current: info.current,
        total: info.total,
        range_start: info.range_start,
        range_end: info.range_end,
        total_items: info.total_items,
        unit: info.unit,
    }
}

fn preview_stats_lines(
    label: &str,
    slice: &[(f64, f64)],
    precise_value: bool,
) -> Option<Vec<Line<'static>>> {
    if slice.is_empty() {
        return None;
    }
    let label_style = Style::default().fg(configure::themed_color(|colors| colors.text.type_desc));
    let value_style = if configure::prefers_strong_text() {
        Style::default()
            .fg(configure::themed_color(|colors| colors.text.primary))
            .bold()
    } else {
        Style::default().fg(configure::themed_color(|colors| colors.text.primary))
    };
    let start_x = slice.first()?.0;
    let end_x = slice.last()?.0;
    if precise_value {
        let value = slice[0].1;
        return Some(vec![
            Line::from(vec![
                Span::styled(format!("{label} "), label_style),
                Span::styled(format!("x {:.1}", start_x), value_style),
            ]),
            Line::from(vec![
                Span::styled("value ", label_style),
                Span::styled(format!("{value:.4}"), value_style),
            ]),
        ]);
    }
    let count = slice.len();
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut sum = 0.0;
    let mut finite = 0usize;
    for &(_, y) in slice {
        if y.is_finite() {
            min = min.min(y);
            max = max.max(y);
            sum += y;
            finite += 1;
        }
    }
    if finite == 0 {
        return None;
    }
    Some(vec![
        Line::from(vec![
            Span::styled(format!("{label} "), label_style),
            Span::styled(format!("{start_x:.1}..{end_x:.1}  "), value_style),
            Span::styled("count ", label_style),
            Span::styled(format!("{count}"), value_style),
        ]),
        Line::from(vec![
            Span::styled("mean ", label_style),
            Span::styled(format!("{:.4}  ", sum / finite as f64), value_style),
            Span::styled("min/max ", label_style),
            Span::styled(format!("{min:.4}/{max:.4}"), value_style),
        ]),
    ])
}

pub(super) fn preview_stats_info(state: &AppState<'_>) -> Option<Vec<Line<'static>>> {
    let data = state.chart_preview_state.current_data.as_ref()?;
    if data.data.is_empty() {
        return None;
    }
    if let Some(roi) = state.chart_preview_state.roi {
        if roi.start < data.data.len() {
            let end = roi
                .end
                .min(data.data.len().saturating_sub(1))
                .max(roi.start);
            return preview_stats_lines(
                "ROI",
                &data.data[roi.start..=end],
                roi.selection_count == 1 && roi.precise,
            );
        }
    }
    let viewport = state.chart_preview_state.effective_viewport()?;
    let x_min = match state
        .chart_preview_state
        .ds_selection
        .as_ref()
        .map(|selection| &selection.slice)
    {
        Some(SliceSelection::FromTo(start, _)) => *start as f64,
        _ => 0.0,
    };
    let (start, end) = preview_visible_index_window(data, viewport, x_min)?;
    preview_stats_lines("View", &data.data[start..=end], false)
}

fn preview_selection_lines(node: &mut H5FNode, shape: &[usize]) -> Vec<Line<'static>> {
    let label_style = Style::default().fg(configure::themed_color(|colors| colors.text.type_desc));
    let value_style = if configure::prefers_strong_text() {
        Style::default()
            .fg(configure::themed_color(|colors| colors.text.primary))
            .bold()
    } else {
        Style::default().fg(configure::themed_color(|colors| colors.text.primary))
    };
    let separator_style =
        Style::default().fg(configure::themed_color(|colors| colors.surface.break_line));

    let mut shape_spans = vec![Span::styled("Shape ", label_style)];
    for (index, dim) in shape.iter().enumerate() {
        if index > 0 {
            shape_spans.push(Span::styled(" | ", separator_style));
        }
        shape_spans.push(Span::styled(dim.to_string(), value_style));
    }

    let mut slice_spans = vec![Span::styled("Slice ", label_style)];
    for (index, _) in shape.iter().enumerate() {
        if index > 0 {
            slice_spans.push(Span::styled(" | ", separator_style));
        }
        if index == node.selected_x {
            slice_spans.push(Span::styled(
                "X".to_string(),
                Style::default()
                    .fg(configure::themed_color(|colors| colors.accent.selected_dim))
                    .bold(),
            ));
        } else {
            let selected_index = node
                .selected_indexes
                .get(index)
                .copied()
                .unwrap_or_default();
            let style = if index == node.selected_dim {
                Style::default()
                    .fg(configure::themed_color(|colors| colors.text.primary))
                    .bold()
                    .underlined()
                    .underline_color(configure::themed_color(|colors| {
                        colors.accent.selected_index
                    }))
            } else {
                value_style
            };
            slice_spans.push(Span::styled(selected_index.to_string(), style));
        }
    }

    vec![Line::from(shape_spans), Line::from(slice_spans)]
}

fn preview_view_lines(info: Option<&PageDisplayInfo<'_>>) -> Vec<Line<'static>> {
    let label_style = Style::default().fg(configure::themed_color(|colors| colors.text.type_desc));
    let value_style = if configure::prefers_strong_text() {
        Style::default()
            .fg(configure::themed_color(|colors| colors.text.primary))
            .bold()
    } else {
        Style::default().fg(configure::themed_color(|colors| colors.text.primary))
    };
    let Some(info) = info else {
        return vec![
            Line::from(vec![
                Span::styled("View ".to_string(), label_style),
                Span::styled("-".to_string(), value_style),
            ]),
            Line::from(vec![
                Span::styled("range ".to_string(), label_style),
                Span::styled("-".to_string(), value_style),
            ]),
        ];
    };
    let size = info.range_end.saturating_sub(info.range_start);
    vec![
        Line::from(vec![
            Span::styled(
                format!(
                    "{} {}/{}  ",
                    info.title,
                    info.current.saturating_add(1),
                    info.total.max(1)
                ),
                value_style,
            ),
            Span::styled("size ".to_string(), label_style),
            Span::styled(
                format!("{} {}", compact_count(size), info.unit),
                value_style,
            ),
        ]),
        Line::from(vec![
            Span::styled("range ".to_string(), label_style),
            Span::styled(
                format!(
                    "{}..{}",
                    compact_count(info.range_start),
                    compact_count(info.range_end.saturating_sub(1))
                ),
                value_style,
            ),
        ]),
    ]
}

pub(super) fn render_preview_context_panel(
    f: &mut Frame<'_>,
    area: &Rect,
    node: &mut H5FNode,
    shape: &[usize],
    state: &mut AppState<'_>,
    view_info: Option<&PageDisplayInfo<'_>>,
    stats_lines: Option<&[Line<'static>]>,
) {
    let mode = state.chart_preview_state.mode;
    let title = format!("View & selection [{} · t]", mode.label());
    let control_width = "  X  Lin  Log ".chars().count() as u16;
    let mut axes = Vec::new();
    for (x_axis, label, selected) in [
        (true, "X", state.chart_preview_state.x_axis_scale),
        (false, "Y", state.chart_preview_state.y_axis_scale),
    ] {
        if super::preview_axis_scale_supported(state, x_axis)
            && area.width
                > title.chars().count() as u16
                    + control_width.saturating_mul(axes.len() as u16 + 1)
                    + 2
        {
            axes.push((x_axis, label, selected));
        }
    }
    let control_style = |scale, selected| {
        if scale == selected {
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
    };
    let mut title_spans = vec![Span::raw(title.clone())];
    for (_, label, selected) in &axes {
        title_spans.push(Span::raw(format!("  {label} ")));
        title_spans.push(Span::styled(
            " Lin ",
            control_style(ChartAxisScale::Linear, *selected),
        ));
        title_spans.push(Span::styled(
            " Log ",
            control_style(ChartAxisScale::Logarithmic, *selected),
        ));
    }
    let title_line = Line::from(title_spans);
    let block = Block::default()
        .title(title_line)
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
    let mut control_x = area
        .x
        .saturating_add(1)
        .saturating_add(title.chars().count() as u16);
    for (x_axis, _, _) in axes {
        control_x = control_x.saturating_add(4);
        state.ui_layout.preview_axis_scales.extend([
            PreviewAxisScaleHitbox {
                area: Rect::new(control_x, area.y, 5, 1),
                x_axis,
                scale: ChartAxisScale::Linear,
            },
            PreviewAxisScaleHitbox {
                area: Rect::new(control_x.saturating_add(5), area.y, 5, 1),
                x_axis,
                scale: ChartAxisScale::Logarithmic,
            },
        ]);
        control_x = control_x.saturating_add(10);
    }

    let inner = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let columns = Layout::horizontal([
        Constraint::Fill(5),
        Constraint::Length(1),
        Constraint::Fill(4),
        Constraint::Length(1),
        Constraint::Fill(5),
    ])
    .split(inner);
    let separator_style =
        Style::default().fg(configure::themed_color(|colors| colors.surface.break_line));
    f.render_widget(Paragraph::new("│\n│").style(separator_style), columns[1]);
    f.render_widget(Paragraph::new("│\n│").style(separator_style), columns[3]);
    f.render_widget(
        Paragraph::new(preview_selection_lines(node, shape)),
        columns[0],
    );
    f.render_widget(Paragraph::new(preview_view_lines(view_info)), columns[2]);
    let stats = stats_lines
        .map(|lines| lines.to_vec())
        .unwrap_or_else(|| vec![Line::from("Stats -"), Line::from("")]);
    f.render_widget(Paragraph::new(stats), columns[4]);
}

pub(super) fn preview_roi_range(
    data_preview: &DatasetPlotingData,
    roi: PreviewChartRoi,
    x_min: f64,
) -> Option<(usize, usize)> {
    if data_preview.data.is_empty() || roi.start >= data_preview.data.len() {
        return None;
    }
    let start = roi.start.min(data_preview.data.len().saturating_sub(1));
    let end = roi
        .end
        .min(data_preview.data.len().saturating_sub(1))
        .max(start);
    let _ = x_min;
    Some((start, end))
}

pub(super) fn preview_roi_x_bounds(
    data_preview: &DatasetPlotingData,
    start: usize,
    end: usize,
    x_min: f64,
) -> Option<(f64, f64)> {
    let points = &data_preview.data;
    if points.is_empty() || start >= points.len() || end >= points.len() || start > end {
        return None;
    }
    let start_x = x_min + points[start].0;
    let end_x = x_min + points[end].0;
    let left_step = if start > 0 {
        (points[start].0 - points[start - 1].0).abs()
    } else if points.len() > 1 {
        (points[1].0 - points[0].0).abs()
    } else {
        1.0
    };
    let right_step = if end + 1 < points.len() {
        (points[end + 1].0 - points[end].0).abs()
    } else if end > 0 {
        (points[end].0 - points[end - 1].0).abs()
    } else {
        1.0
    };
    Some((start_x - (left_step / 2.0), end_x + (right_step / 2.0)))
}

pub(super) fn preview_visible_index_window(
    data_preview: &DatasetPlotingData,
    viewport: PreviewChartViewport,
    x_min: f64,
) -> Option<(usize, usize)> {
    if data_preview.data.is_empty() {
        return None;
    }
    let start = (viewport.x_min - x_min).floor().max(0.0) as usize;
    let end = (viewport.x_max - x_min)
        .ceil()
        .max(viewport.x_min - x_min)
        .min(data_preview.data.len().saturating_sub(1) as f64) as usize;
    Some((start.min(end), end.max(start.min(end))))
}

pub(super) fn preview_visible_points(
    data_preview: &DatasetPlotingData,
    viewport: PreviewChartViewport,
    x_min: f64,
) -> Option<Vec<(f64, f64)>> {
    let (start, end) = preview_visible_index_window(data_preview, viewport, x_min)?;
    let visible = end.saturating_sub(start).saturating_add(1);
    (visible <= PREVIEW_CHART_VISIBLE_POINT_LIMIT).then(|| {
        data_preview.data[start..=end]
            .iter()
            .map(|(x, y)| (x_min + *x, *y))
            .collect()
    })
}

pub(super) fn preview_chart_plot_area(
    chart_area: Rect,
    image_cell_size: (u16, u16),
    max_value: f64,
) -> Option<Rect> {
    let width_px = chart_area.width as u32 * image_cell_size.0.max(1) as u32;
    let height_px = chart_area.height as u32 * image_cell_size.1.max(1) as u32;
    let layout = preview_chart_layout(width_px, height_px, max_value);
    chart_plot_area_in_rect(
        chart_area,
        width_px,
        height_px,
        (layout.margin + layout.y_label_area_size) as i32..(width_px as i32 - layout.margin as i32),
        layout.margin as i32
            ..(height_px as i32 - (layout.margin + layout.x_label_area_size) as i32),
    )
}

pub(super) fn preview_x_axis_max(data_preview: &DatasetPlotingData) -> f64 {
    match data_preview.length {
        0 | 1 => 1.0,
        len => (len - 1) as f64,
    }
}
