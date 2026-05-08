use image::{DynamicImage, ImageBuffer, Rgb};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};
use ratatui_image::StatefulImage;

use crate::{color_consts, compat, error::log_error};

use super::{ChartSource, MultiChartState};

impl MultiChartState {
    pub(crate) fn render(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let header_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color_consts::panel_border_color()))
            .border_type(BorderType::Rounded)
            .title("Multi-Chart Comparison Workspace")
            .bg(color_consts::bg_color())
            .title_style(Style::default().fg(color_consts::title_color()).bold())
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
            .style(Style::default().fg(color_consts::title_color()))
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
            .border_style(Style::default().fg(color_consts::break_color()))
            .title_style(Style::default().fg(color_consts::title_color()).bold());
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
                let (r, g, b) =
                    color_consts::rgb_channels(color_consts::chart_series_color(item.color_slot));
                let marker = compat::chart_visibility_marker(item.visible);
                let prefix = if absolute_idx == self.idx { "> " } else { "  " };
                let is_selected = absolute_idx == self.idx;
                let is_base = self.marked_base_item == Some(item.id);
                let id_style = if is_selected {
                    Style::default()
                        .fg(color_consts::variable_blue_builtin_color())
                        .bold()
                } else {
                    Style::default().fg(color_consts::variable_blue_builtin_color())
                };
                let label_style = match (is_selected, item.visible) {
                    (true, true) => Style::default().fg(color_consts::title_color()).bold(),
                    (true, false) => Style::default()
                        .fg(color_consts::title_color())
                        .bold()
                        .dim(),
                    (false, true) => Style::default().fg(color_consts::built_in_value_color()),
                    (false, false) => Style::default().fg(color_consts::type_desc_color()).dim(),
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
                            Style::default().fg(color_consts::title_color()).bold()
                        } else {
                            Style::default().fg(color_consts::break_color())
                        },
                    ),
                    Span::styled(marker, Style::default().fg(Color::Rgb(r, g, b)).bold()),
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
            .border_style(Style::default().fg(color_consts::variable_blue_builtin_color()))
            .title_style(Style::default().fg(color_consts::title_color()).bold());
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
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(base_line),
                ]),
                Line::from(vec![
                    Span::styled(
                        "path ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(source.display_path.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "type ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(source.kind_label()),
                    Span::raw("  "),
                    Span::styled(
                        "shape ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(source.shape_summary()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "view ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(source.selection_summary()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "stats ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(item.stats_summary()),
                    Span::raw("  "),
                    Span::styled(
                        "align ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(self.x_axis_policy.label()),
                    Span::raw("  "),
                    Span::styled(
                        "zoom ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(viewport),
                ]),
            ],
            ChartSource::BuiltinDerived(source) => vec![
                Line::from(vec![
                    Span::styled(
                        "expr ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(source.expression()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "lhs ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(source.lhs_view.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "rhs ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(source.rhs_view.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "align ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(source.alignment_summary()),
                    Span::raw("  "),
                    Span::styled(
                        "zoom ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(viewport),
                ]),
            ],
            ChartSource::DerivedExpression { expression, .. } => vec![
                Line::from(vec![
                    Span::styled(
                        "expr ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(expression.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "base ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(base_line),
                ]),
                Line::from(vec![
                    Span::styled(
                        "type ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(item.source.source_kind_label()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "stats ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(item.stats_summary()),
                    Span::raw("  "),
                    Span::styled(
                        "align ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
                    ),
                    Span::raw(self.x_axis_policy.label()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "zoom ",
                        Style::default().fg(color_consts::variable_blue_builtin_color()),
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
            .border_style(Style::default().fg(color_consts::break_color()))
            .title_style(Style::default().fg(color_consts::title_color()).bold());
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
            .border_style(Style::default().fg(color_consts::break_color()))
            .title_style(Style::default().fg(color_consts::title_color()).bold());
        let inner = block.inner(area);
        f.render_widget(block, area);

        let mut lines = vec![
            Line::from(vec![
                Span::styled("= ", Style::default().fg(color_consts::title_color()).bold()),
                Span::raw(prompt.buffer.clone()),
            ]),
            Line::from(vec![
                Span::styled(
                    "Syntax ",
                    Style::default().fg(color_consts::variable_blue_builtin_color()),
                ),
                Span::raw("$1 + !/ds[..,0] * #/cal:scale   or   (!/x_ticks, $2 + #/cal/offset)"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Rules ",
                    Style::default().fg(color_consts::variable_blue_builtin_color()),
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
                    Style::default().fg(color_consts::error_color()).bold(),
                ),
                Span::raw(error.clone()),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    "Keys ",
                    Style::default().fg(color_consts::variable_blue_builtin_color()),
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
