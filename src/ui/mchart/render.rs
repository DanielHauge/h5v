use std::ops::Range;

use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea},
    style::{Color as _, IntoFont, RGBColor, ShapeStyle},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Style, Stylize},
    symbols::Marker,
    text::{Line, Span, Text},
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType, Paragraph, Wrap},
};
use ratatui_image::{picker::ProtocolType, StatefulImage};

use crate::{configure, error::log_error};

use super::{
    prompt::ExpressionPromptFocus, ChartSource, ExpressionPromptInputKind,
    ExpressionPromptMessageKind, ExpressionPromptMode, ExpressionPromptSuggestion,
    ExpressionPromptSuggestionKind, MultiChartRenderRequest, MultiChartRenderResult,
    MultiChartState, EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS,
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

pub(super) fn render_prepared_chart_request(
    request: MultiChartRenderRequest,
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
        let y_label_area_size = format!("{:.4}", request.prepared.y_max).len() as u32 * 3 + 30;
        let chart = plotters::prelude::ChartBuilder::on(&root)
            .margin(10)
            .x_label_area_size(30)
            .y_label_area_size(y_label_area_size)
            .build_cartesian_2d(
                request.prepared.plot_x_min..request.prepared.plot_x_max,
                request.prepared.y_min..request.prepared.y_max,
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
            .x_desc(request.x_axis_label)
            .y_label_style(("sans-serif", 18).into_font().color(&axis))
            .x_label_style(("sans-serif", 18).into_font().color(&axis))
            .axis_style(ShapeStyle::from(&axis).stroke_width(2))
            .light_line_style(grid.mix(0.35))
            .bold_line_style(grid.mix(0.55))
            .draw()
        {
            log_error(&error);
        }

        for series in request.prepared.series {
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

impl MultiChartState {
    pub(super) fn chart_panel_title(&self) -> String {
        let mut parts = vec![format!("Overlay chart [{}]", self.x_axis_policy.label())];
        if self.viewport.is_some() {
            parts.push(format!("view {}", self.viewport_summary()));
        }
        if self.loading_item_count() != 0 {
            parts.push(format!("{} loading", self.loading_item_count()));
        }
        parts.join(" · ")
    }

    pub(super) fn should_defer_image_protocol_frame(&self, chart_area: Rect) -> bool {
        self.expression_prompt.is_some()
            && !self.modified
            && self.stateful_protocol.is_some()
            && self.last_chart_panel_area == Some(chart_area)
    }

    pub(super) fn prepared_chart_data(&self) -> Option<super::PreparedChartData> {
        let visible_items = self
            .items
            .iter()
            .filter(|item| item.visible && item.has_loaded_series())
            .collect::<Vec<_>>();
        if visible_items.is_empty() {
            return None;
        }

        let viewport = self.effective_viewport()?;

        let selected_item_id = self.selected_item().map(|item| item.id);
        let mut plot_x_min = f64::MAX;
        let mut plot_x_max = f64::MIN;
        let mut series = Vec::new();

        for item in visible_items {
            let points = super::model::sanitize_chart_points(
                item.active_series()
                    .points
                    .iter()
                    .copied()
                    .filter(|(x, _)| *x >= viewport.x_min && *x <= viewport.x_max)
                    .collect(),
            );
            if points.is_empty() {
                continue;
            }

            for &(x, _) in &points {
                plot_x_min = plot_x_min.min(x);
                plot_x_max = plot_x_max.max(x);
            }

            series.push(super::PreparedChartSeries {
                label: item
                    .name
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| item.label.clone()),
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
            if !plot_x_min.is_finite() || !plot_x_max.is_finite() {
                return None;
            }
            if (plot_x_max - plot_x_min).abs() < f64::EPSILON {
                let pad = if plot_x_min == 0.0 {
                    1.0
                } else {
                    plot_x_min.abs() * 0.05
                };
                (plot_x_min - pad, plot_x_max + pad)
            } else {
                (plot_x_min, plot_x_max)
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
            if !y_min.is_finite() || !y_max.is_finite() {
                return None;
            }
            if (y_max - y_min).abs() < f64::EPSILON {
                let pad = if y_min == 0.0 {
                    1.0
                } else {
                    y_min.abs() * 0.05
                };
                (y_min - pad, y_max + pad)
            } else {
                (y_min, y_max)
            }
        };

        Some(super::PreparedChartData {
            plot_x_min,
            plot_x_max,
            y_min,
            y_max,
            series,
        })
    }

    pub(crate) fn render(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let header_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(configure::themed_color(|colors| {
                colors.surface.panel_border
            })))
            .border_type(BorderType::Rounded)
            .title("Multi-Chart Comparison Workspace")
            .bg(configure::themed_color(|colors| colors.surface.bg))
            .title_style(
                Style::default()
                    .fg(configure::themed_color(|colors| colors.surface.panel_title))
                    .bold(),
            )
            .title_alignment(Alignment::Center);
        f.render_widget(header_block, area);

        let inner_area = area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
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
                .constraints([Constraint::Min(10), Constraint::Length(7)])
                .split(panes[1]);

            self.render_item_list(f, sidebar_chunks[0]);
            self.render_selected_details(f, sidebar_chunks[1]);
            self.render_selected_statistics(f, sidebar_chunks[2]);

            if self.items.is_empty() {
                self.render_empty(f, main_chunks[0]);
            } else {
                self.render_chart_panel(f, main_chunks[0]);
            }

            self.render_expression_prompt(f, main_chunks[1]);
            return;
        }

        let panes = {
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Length(7)])
                .split(inner_area);
            let workspace_area = split[0];
            let prompt_area = split[1];
            if self.items.is_empty() {
                self.render_empty(f, workspace_area);
                self.render_expression_prompt(f, prompt_area);
                return;
            }

            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(11),
                    Constraint::Min(12),
                    Constraint::Length(8),
                    Constraint::Length(8),
                ])
                .split(workspace_area)
        };
        self.render_item_list(f, panes[0]);
        self.render_chart_panel(f, panes[1]);
        self.render_selected_details(f, panes[2]);
        self.render_selected_statistics(f, panes[3]);
        self.render_expression_prompt(
            f,
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Length(7)])
                .split(inner_area)[1],
        );
    }

    fn render_empty(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        self.last_chart_area = None;
        self.drag_state = None;
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

    fn render_item_list(&self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let block = Block::default()
            .title(format!(
                "Items ({}/{} visible, {} loading)",
                self.visible_item_count(),
                self.items.len(),
                self.loading_item_count()
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(
                Style::default().fg(configure::themed_color(|colors| colors.surface.break_line)),
            )
            .title_style(
                Style::default()
                    .fg(configure::themed_color(|colors| colors.surface.panel_title))
                    .bold(),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);

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
            .border_style(
                Style::default().fg(configure::themed_color(|colors| colors.mchart.detail_label)),
            )
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

        let viewport = self.viewport_summary();
        let lines = match &item.source {
            ChartSource::DatasetSelection(source) => vec![
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
            .border_style(
                Style::default().fg(configure::themed_color(|colors| colors.mchart.detail_label)),
            )
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
        let block = Block::default()
            .title(self.chart_panel_title())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(
                Style::default().fg(configure::themed_color(|colors| colors.surface.break_line)),
            )
            .title_style(
                Style::default()
                    .fg(configure::themed_color(|colors| colors.surface.panel_title))
                    .bold(),
            );
        let chart_area = block.inner(area);
        f.render_widget(block, area);

        if self.visible_item_count() == 0 {
            self.last_chart_area = None;
            self.last_chart_panel_area = None;
            let paragraph = Paragraph::new(format!(
                "All chart items are hidden.\nPress Space or 'v' to toggle the selected item back on.\nCurrent alignment: {}.",
                self.x_axis_policy.description()
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
        if self.picker.protocol_type() == ProtocolType::Halfblocks {
            self.last_chart_area = Some(chart_area);
            self.last_chart_panel_area = Some(chart_area);
            if !self.render_braille_chart_panel(f, chart_area) {
                let paragraph = Paragraph::new("Rendering failed")
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
                        error.as_str(),
                        configure::themed_color(|colors| colors.text.error),
                    )
                } else if self.pending_render_generation.is_some() {
                    (
                        "Rendering chart...",
                        configure::themed_color(|colors| colors.mchart.empty_state),
                    )
                } else {
                    (
                        "Rendering failed",
                        configure::themed_color(|colors| colors.text.error),
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
        let Some(prepared) = self.prepared_chart_data() else {
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

    fn render_expression_prompt(&self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let prompt = self.expression_prompt.as_ref();
        let panel_bg = configure::themed_color(|colors| colors.surface.bg);
        let title = match prompt {
            Some(prompt)
                if prompt
                    .messages
                    .iter()
                    .any(|message| message.kind == ExpressionPromptMessageKind::Error) =>
            {
                "Expression editor [invalid]"
            }
            Some(prompt) if matches!(prompt.mode, ExpressionPromptMode::EditExisting(_)) => {
                "Expression editor [edit]"
            }
            Some(_) => "Expression editor",
            None => "Expression editor",
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().bg(panel_bg))
            .border_style(
                Style::default().fg(configure::themed_color(|colors| colors.surface.break_line)),
            )
            .title_style(
                Style::default()
                    .fg(configure::themed_color(|colors| colors.surface.panel_title))
                    .bold(),
            );
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
        f.render_widget(input_block.clone(), chunks[0]);
        f.render_widget(
            Paragraph::new(input_text).style(mchart_body_style().bg(panel_bg)),
            input_block.inner(chunks[0]),
        );

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
            let prefix_width = 1
                + prompt.item_id.0.to_string().chars().count()
                + 1
                + name_display.chars().count()
                + 3;
            let cursor = ratatui::layout::Position::new(
                if prompt.focus == ExpressionPromptFocus::Name {
                    let id_width = prompt.item_id.0.to_string().chars().count();
                    let name_offset = 1;
                    chunks[0].x.saturating_add(
                        (1 + 2 + id_width + name_offset + prompt.name_cursor) as u16,
                    )
                } else {
                    chunks[0]
                        .x
                        .saturating_add((1 + prefix_width + prompt.cursor) as u16)
                },
                chunks[0].y.saturating_add(1),
            );
            f.set_cursor_position(cursor);
        }
    }
}

pub(super) fn chart_plot_area_in_rect(
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
    use super::{mchart_body_span, mchart_body_style};

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
}
