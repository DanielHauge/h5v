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

use super::{
    ChartSource, ExpressionPromptInputKind, ExpressionPromptMessageKind, ExpressionPromptMode,
    ExpressionPromptSuggestion, ExpressionPromptSuggestionKind, MultiChartState,
    EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS,
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
                let is_selected = absolute_idx == self.idx;
                let marker_color = configure::themed_color(|colors| {
                    colors.chart.series[item.color_slot % colors.chart.series.len()]
                });
                let marker = if item.visible {
                    configure::configured_symbol(|symbols| symbols.chart.visibility_visible)
                } else {
                    configure::configured_symbol(|symbols| symbols.chart.visibility_hidden)
                };
                let row_style = match (is_selected, item.visible) {
                    (true, true) => Style::default()
                        .fg(configure::themed_color(|colors| colors.text.primary))
                        .bg(configure::themed_color(|colors| colors.surface.focus_bg))
                        .bold(),
                    (true, false) => Style::default()
                        .fg(configure::themed_color(|colors| colors.text.primary))
                        .bg(configure::themed_color(|colors| colors.surface.focus_bg))
                        .bold()
                        .dim(),
                    (false, true) => Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.item_visible)),
                    (false, false) => Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.item_hidden))
                        .dim(),
                };
                Line::from(vec![
                    Span::styled(" ", row_style),
                    Span::styled(marker, row_style.fg(marker_color).bold()),
                    Span::styled(" ", row_style),
                    Span::styled(format!("${} ", item.id.0), row_style),
                    Span::styled(item.list_label(), row_style),
                ])
            })
            .collect::<Vec<_>>();
        f.render_widget(
            Paragraph::new(Text::from(lines))
                .style(mchart_body_style())
                .wrap(Wrap { trim: true }),
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
                        "zoom ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(viewport),
                ]),
            ],
            ChartSource::BuiltinDerived(source) => vec![
                Line::from(vec![
                    Span::styled(
                        "expr ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(source.expression()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "lhs ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(source.lhs_view.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "rhs ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(source.rhs_view.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "align ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(source.alignment_summary()),
                    mchart_body_span("  "),
                    Span::styled(
                        "zoom ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(viewport),
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
                        "zoom ",
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.detail_label)),
                    ),
                    mchart_body_span(viewport),
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
        f.render_widget(block, area);

        if self.visible_item_count() == 0 {
            self.last_chart_area = None;
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
        if chart_area.width == 0 || chart_area.height == 0 {
            self.last_chart_area = None;
            return;
        }
        if self.picker.protocol_type() == ProtocolType::Halfblocks {
            self.last_chart_area = Some(chart_area);
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
        if new_height != self.height || new_width != self.width {
            self.height = new_height;
            self.width = new_width;
            self.modified = true;
            self.stateful_protocol = None;
        }

        if self.render_chart_with_area(Some(chart_area)) {
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
                    .style(Style::default().fg(configure::themed_color(|colors| colors.text.error)))
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
            Some(prompt) if prompt.buffer.is_empty() => Line::from(vec![
                Span::styled(
                    "= ",
                    Style::default()
                        .fg(configure::themed_color(|colors| {
                            colors.mchart.prompt_prefix
                        }))
                        .bold(),
                ),
                Span::styled(
                    "$1 + !/path[..,0]",
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.text.type_desc))
                        .italic(),
                ),
            ]),
            Some(prompt) => {
                let mut spans = vec![Span::styled(
                    "= ",
                    Style::default()
                        .fg(configure::themed_color(|colors| {
                            colors.mchart.prompt_prefix
                        }))
                        .bold(),
                )];
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
                Line::from(spans)
            }
            None => Line::from(vec![
                Span::styled(
                    "= ",
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
            ]),
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
                        let selected_bg = (prompt.selected_suggestion == Some(idx))
                            .then(|| configure::themed_color(|colors| colors.surface.focus_bg))
                            .unwrap_or(panel_bg);
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
            let cursor = ratatui::layout::Position::new(
                chunks[0].x.saturating_add(3 + prompt.cursor as u16),
                chunks[0].y.saturating_add(1),
            );
            f.set_cursor_position(cursor);
        }
    }
}

#[cfg(test)]
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
