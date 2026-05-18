use super::*;
use crate::ui::{
    chrome::{rounded_panel, truncate_to_width},
    mchart::MultiChartLoadState,
};

impl MultiChartState {
    pub(super) fn render_empty(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
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

    pub(super) fn render_item_list(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let block = rounded_panel(" 🧾 Items ").title_alignment(Alignment::Center);
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
                let has_error = matches!(item.load_state, MultiChartLoadState::Error(_));
                let marker_color = if has_error {
                    configure::themed_color(|colors| colors.text.error)
                } else {
                    configure::themed_color(|colors| {
                        colors.chart.series[item.color_slot % colors.chart.series.len()]
                    })
                };
                let marker = if has_error {
                    configure::configured_symbol(|symbols| symbols.chart.error_marker)
                } else if item.visible {
                    configure::configured_symbol(|symbols| symbols.chart.visibility_visible)
                } else {
                    configure::configured_symbol(|symbols| symbols.chart.visibility_hidden)
                };
                let row_style = match (has_error, is_selected, item.visible) {
                    (true, true, _) => mchart_soft_muted(
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.text.error))
                            .bg(configure::themed_color(|colors| colors.surface.focus_bg)),
                    ),
                    (true, false, _) => mchart_soft_muted(
                        Style::default().fg(configure::themed_color(|colors| colors.text.error)),
                    ),
                    (false, true, true) => Style::default()
                        .fg(configure::themed_color(|colors| colors.text.primary))
                        .bg(configure::themed_color(|colors| colors.surface.focus_bg))
                        .bold(),
                    (false, true, false) => Style::default()
                        .fg(configure::themed_color(|colors| colors.text.primary))
                        .bg(configure::themed_color(|colors| colors.surface.focus_bg))
                        .bold(),
                    (false, false, true) => Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.item_visible)),
                    (false, false, false) => mchart_soft_muted(
                        Style::default()
                            .fg(configure::themed_color(|colors| colors.mchart.item_hidden)),
                    ),
                };
                let id_text = format!("${}", item.id.0);
                let prefix_width = 5 + id_text.chars().count();
                let label_width = inner.width as usize - prefix_width.min(inner.width as usize);
                let label = truncate_to_width(&item.list_label(), label_width);
                let selected_bg =
                    is_selected.then(|| configure::themed_color(|colors| colors.surface.focus_bg));
                let label_style = if item.name.is_some() {
                    let mut style = Style::default()
                        .fg(configure::themed_color(|colors| colors.mchart.item_named))
                        .underlined();
                    if let Some(bg) = selected_bg {
                        style = style.bg(bg);
                    }
                    if !item.visible || has_error {
                        style = mchart_soft_muted(style);
                    }
                    style
                } else {
                    row_style
                };
                let mut id_style = Style::default()
                    .fg(configure::themed_color(|colors| colors.mchart.item_id))
                    .bold();
                if let Some(bg) = selected_bg {
                    id_style = id_style.bg(bg);
                }
                if !item.visible || has_error {
                    id_style = mchart_soft_muted(id_style);
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
                            mchart_soft_muted(
                                row_style.fg(configure::themed_color(|colors| colors.text.error)),
                            )
                        } else {
                            mchart_soft_muted(
                                row_style.fg(configure::themed_color(|colors| {
                                    colors.mchart.detail_label
                                })),
                            )
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

    pub(super) fn render_selected_details(&self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let block = rounded_panel("Active item");
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

    pub(super) fn render_selected_statistics(&self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let block = rounded_panel("Statistics");
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
}
