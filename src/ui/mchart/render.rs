use std::ops::Range;

use crate::{configure, ui::cursor::set_input_cursor};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::Style,
    symbols::Marker,
    text::{Line, Span, Text},
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType, Paragraph, Wrap},
};
use ratatui_image::{picker::ProtocolType, StatefulImage};

use super::{
    prompt::ExpressionPromptFocus, ChartSource, ExpressionPromptInputKind,
    ExpressionPromptMessageKind, ExpressionPromptMode, ExpressionPromptSuggestion,
    ExpressionPromptSuggestionKind, MultiChartEditorHitbox, MultiChartItemHitbox, MultiChartState,
    MultiChartViewModeHitbox, EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS,
};

mod backend;
mod panels;
mod prepared;

pub(crate) use backend::render_prepared_chart_request;

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

fn mchart_soft_muted(style: Style) -> Style {
    if configure::prefers_strong_text() {
        style
    } else {
        style.dim()
    }
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

    fn chart_mode_tab_specs(&self) -> Vec<(super::MultiChartViewMode, String, u16)> {
        let labels = [
            (super::MultiChartViewMode::Line, " Line "),
            (super::MultiChartViewMode::Histogram, " Histogram "),
            (super::MultiChartViewMode::BoxPlot, " Box plot "),
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
            super::MultiChartViewMode::BoxPlot => {
                "[visible sample values] - quartiles, whiskers, and outliers".to_string()
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
            super::MultiChartViewMode::BoxPlot => {
                "No box-plot samples in the current visible window.".to_string()
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
            set_input_cursor(f, cursor);
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
