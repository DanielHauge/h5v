use image::{DynamicImage, ImageBuffer, Rgb};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Style, Stylize},
    symbols::Marker,
    text::{Line, Span, Text},
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType, Paragraph, Wrap},
};
use ratatui_image::{picker::ProtocolType, StatefulImage};

use crate::{configure, error::log_error};

use super::{ChartSource, MultiChartState};

impl MultiChartState {
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
        let (workspace_area, prompt_area) = if self.expression_prompt.is_some() {
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Length(6)])
                .split(inner_area);
            (split[0], Some(split[1]))
        } else {
            (inner_area, None)
        };

        if self.items.is_empty() {
            self.render_empty(f, workspace_area);
            if let Some(prompt_area) = prompt_area {
                self.render_expression_prompt(f, prompt_area);
            }
            return;
        }

        let panes = if workspace_area.width < 110 {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(11), Constraint::Min(12)])
                .split(workspace_area)
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(42), Constraint::Min(20)])
                .split(workspace_area)
        };

        let (sidebar_area, chart_area) = (panes[0], panes[1]);
        let sidebar_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(6)])
            .split(sidebar_area);
        self.render_item_list(f, sidebar_chunks[0]);
        self.render_selected_details(f, sidebar_chunks[1]);
        self.render_chart_panel(f, chart_area);
        if let Some(prompt_area) = prompt_area {
            self.render_expression_prompt(f, prompt_area);
        }
    }

    fn render_empty(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        self.last_chart_area = None;
        self.drag_state = None;
        let no_data_message = concat!(
            "No chart items yet.\n\n",
            "Press 'm' on any previewable dataset view to add it here.\n",
            "The same dataset can appear multiple times with different x dimensions or fixed indices.\n",
            "Use Space to mark a base series, then D/S/R/P to derive difference, sum, ratio, or product."
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
                "Items ({}/{} visible)",
                self.visible_item_count(),
                self.items.len()
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
        if available_rows == 0 {
            return;
        }

        let half = available_rows / 2;
        let start = self.idx.saturating_sub(half);
        let end = usize::min(start + available_rows, self.items.len());
        let start = end.saturating_sub(available_rows);

        let lines = self.items[start..end]
            .iter()
            .enumerate()
            .map(|(offset, item)| {
                let absolute_idx = start + offset;
                let marker_color = configure::themed_color(|colors| {
                    colors.chart.series[item.color_slot % colors.chart.series.len()]
                });
                let marker = if item.visible {
                    configure::configured_symbol(|symbols| symbols.chart.visibility_visible)
                } else {
                    configure::configured_symbol(|symbols| symbols.chart.visibility_hidden)
                };
                let prefix = if absolute_idx == self.idx { "> " } else { "  " };
                let is_selected = absolute_idx == self.idx;
                let is_base = self.marked_base_item == Some(item.id);
                let id_style = if is_selected {
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.detail_label))
                        .bold()
                } else {
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.detail_label))
                };
                let label_style = match (is_selected, item.visible) {
                    (true, true) => Style::default()
                        .fg(configure::themed_color(|colors| {
                            colors.mchart.item_selected
                        }))
                        .bold(),
                    (true, false) => Style::default()
                        .fg(configure::themed_color(|colors| {
                            colors.mchart.item_selected_hidden
                        }))
                        .bold()
                        .dim(),
                    (false, true) => Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.item_visible)),
                    (false, false) => Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.item_hidden))
                        .dim(),
                };
                let label_style = if is_base {
                    label_style.underlined()
                } else {
                    label_style
                };
                Line::from(vec![
                    Span::styled(
                        prefix,
                        if is_selected {
                            Style::default()
                                .fg(configure::themed_color(|colors| {
                                    colors.mchart.prefix_selected
                                }))
                                .bold()
                        } else {
                            Style::default()
                                .fg(configure::themed_color(|colors| colors.mchart.prefix))
                        },
                    ),
                    Span::styled(marker, Style::default().fg(marker_color).bold()),
                    Span::raw(" "),
                    Span::styled(format!("(${}) ", item.id.0), id_style),
                    Span::styled(item.list_label(), label_style),
                ])
            })
            .collect::<Vec<_>>();
        f.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true }),
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

        let viewport = match (self.aoi_from, self.aoi_to) {
            (None, None) => "full range".to_string(),
            (Some(from), Some(to)) => format!("{from}..{to}"),
            (Some(from), None) => format!("{from}..end"),
            (None, Some(to)) => format!("start..{to}"),
        };
        let base_line = self
            .marked_base_item_ref()
            .map(|item| item.reference_label())
            .unwrap_or_else(|| "none".to_string());

        let lines = match &item.source {
            ChartSource::DatasetSelection(source) => vec![
                Line::from(vec![
                    Span::styled(
                        "base ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(base_line),
                ]),
                Line::from(vec![
                    Span::styled(
                        "path ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(source.display_path.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "type ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(source.kind_label()),
                    Span::raw("  "),
                    Span::styled(
                        "shape ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(source.shape_summary()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "view ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(source.selection_summary()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "stats ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(item.stats_summary()),
                    Span::raw("  "),
                    Span::styled(
                        "align ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(self.x_axis_policy.label()),
                    Span::raw("  "),
                    Span::styled(
                        "zoom ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(viewport),
                ]),
            ],
            ChartSource::BuiltinDerived(source) => vec![
                Line::from(vec![
                    Span::styled(
                        "expr ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(source.expression()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "lhs ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(source.lhs_view.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "rhs ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(source.rhs_view.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "align ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(source.alignment_summary()),
                    Span::raw("  "),
                    Span::styled(
                        "zoom ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(viewport),
                ]),
            ],
            ChartSource::DerivedExpression { expression, .. } => vec![
                Line::from(vec![
                    Span::styled(
                        "expr ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(expression.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "base ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(base_line),
                ]),
                Line::from(vec![
                    Span::styled(
                        "type ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(item.source.source_kind_label()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "stats ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(item.stats_summary()),
                    Span::raw("  "),
                    Span::styled(
                        "align ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(self.x_axis_policy.label()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "zoom ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    Span::raw(viewport),
                ]),
            ],
        };

        f.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true }),
            inner,
        );
    }

    fn render_chart_panel(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let block = Block::default()
            .title(format!("Overlay chart [{}]", self.x_axis_policy.label()))
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
        self.last_chart_area =
            (chart_area.width > 0 && chart_area.height > 0).then_some(chart_area);
        f.render_widget(block, area);

        if self.visible_item_count() == 0 {
            let paragraph = Paragraph::new(format!(
                "All chart items are hidden.\nPress 'v' to toggle the selected item back on.\nCurrent alignment: {}.",
                self.x_axis_policy.description()
            ))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
            f.render_widget(paragraph, chart_area);
            return;
        }
        if chart_area.width == 0 || chart_area.height == 0 {
            return;
        }
        if self.picker.protocol_type() == ProtocolType::Halfblocks {
            if !self.render_braille_chart_panel(f, chart_area) {
                let paragraph = Paragraph::new("Rendering failed")
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: true });
                f.render_widget(paragraph, chart_area);
            }
            return;
        }

        let (cell_w, cell_h) = self.picker.font_size();
        let new_height = chart_area.height as u32 * cell_h as u32;
        let new_width = chart_area.width as u32 * cell_w as u32;
        if new_height != self.height || new_width != self.width {
            self.height = new_height;
            self.width = new_width;
            self.modified = true;
            self.stateful_protocol = None;
        }

        if self.render_chart() {
            let image = ImageBuffer::<Rgb<u8>, _>::from_raw(
                self.width,
                self.height,
                self.plot_buffer.clone(),
            );
            let Some(image) = image else {
                log_error("Failed to create image buffer from plot buffer");
                return;
            };
            let dyn_img = DynamicImage::ImageRgb8(image);
            self.stateful_protocol = Some(self.picker.new_resize_protocol(dyn_img));
        }

        match self.stateful_protocol {
            None => {
                let paragraph = Paragraph::new("Rendering failed")
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
                if series.is_base || series.is_selected {
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
        let Some(prompt) = self.expression_prompt.as_ref() else {
            return;
        };
        let title = match &prompt.error {
            Some(_) => "Expression prompt [invalid]",
            None => "Expression prompt",
        };
        let block = Block::default()
            .title(title)
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

        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    "= ",
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.prompt_prefix))
                        .bold(),
                ),
                Span::raw(prompt.buffer.clone()),
            ]),
            Line::from(vec![
                Span::styled(
                    "Syntax ",
                    Style::default().fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                ),
                Span::raw("$1 + !/ds[..,0] * #/cal:scale   or   (!/x_ticks, $2 + #/cal/offset)"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Rules ",
                    Style::default().fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                ),
                Span::raw(
                    "single expr => y-series; (x,y) => x/y series. Use $id or !/path[..] for series, #/path for scalar datasets, and :ATTR on ! or # for explicit attributes",
                ),
            ]),
        ];
        if let Some(error) = &prompt.error {
            lines.push(Line::from(vec![
                Span::styled(
                    "Error ",
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.text.error))
                        .bold(),
                ),
                Span::raw(error.clone()),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    "Keys ",
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                ),
                Span::raw("Enter create  Esc cancel"),
            ]));
        }

        f.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true }),
            inner,
        );
        let cursor = ratatui::layout::Position::new(
            inner.x.saturating_add(2 + prompt.cursor as u16),
            inner.y,
        );
        f.set_cursor_position(cursor);
    }
}
