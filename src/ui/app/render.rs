use std::rc::Rc;

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    configure,
    health::HealthStatus,
    ui::{
        chrome::truncate_to_width,
        command::render_command_dialog,
        help::render_help,
        logs::render_logs,
        main_display::render_main_display,
        state::{self, AppState, AppToast, Focus, LastFocused, Mode},
        tree_view::{render_tree, TreeItem},
    },
    GIT_VERSION,
};

use super::dialogs::{
    render_attribute_create_dialog, render_attribute_delete_dialog,
    render_fixed_string_overflow_dialog, render_fixed_string_resize_dialog,
};

const HEADER_HEIGHT: u16 = 1;
const COMMAND_BAR_HEIGHT: u16 = 6;

pub(crate) fn primary_text_style() -> Style {
    let mut style = Style::default().fg(configure::themed_color(|colors| colors.text.primary));
    if configure::prefers_strong_text() {
        style = style.bold();
    }
    style
}

pub(crate) fn main_content_focus(focus: &Focus) -> LastFocused {
    match focus {
        Focus::Tree(last_focused) => last_focused.clone(),
        Focus::Attributes => LastFocused::Attributes,
        Focus::Content => LastFocused::Content,
    }
}

pub(super) fn draw_app_frame(
    frame: &mut Frame<'_>,
    state: &mut AppState<'_>,
    new_version: Option<&str>,
) {
    let command_over_multichart = matches!(state.mode, Mode::Command)
        && matches!(state.command_return_mode, Mode::MultiChart);
    state.ui_layout = state::UiLayoutState::default();
    let content_area = render_header(frame, frame.area(), state, new_version);
    let command_area = match state.mode {
        Mode::Command => command_modal_area(content_area),
        _ => Rect::new(0, 0, 0, 0),
    };

    if let Mode::Help = state.mode {
        render_help(frame, content_area, state);
        render_toast_overlay(frame, state, command_area);
        return;
    }
    if let Mode::Logs = state.mode {
        render_logs(frame, content_area, state);
        render_toast_overlay(frame, state, command_area);
        return;
    }
    if matches!(state.mode, Mode::MultiChart) || command_over_multichart {
        state.multi_chart.render(frame, content_area);
        if matches!(state.mode, Mode::Command) {
            render_command_dialog(frame, command_area, state);
        }
        render_toast_overlay(frame, state, command_area);
        return;
    }

    let show_tree_view = state.show_tree_view;
    state.stacked_tree_layout =
        use_stacked_tree_layout(content_area, &state.mode, state.show_tree_view);

    let main_display_area = match show_tree_view {
        true => {
            let areas = make_panels_rect(content_area, &state.mode, &state.focus, &state.treeview);
            let (tree_area, main_display_area) = (areas[0], areas[1]);
            render_tree(frame, tree_area, state);
            main_display_area
        }
        false => content_area,
    };

    match state.mode {
        Mode::Search => {}
        Mode::Command
        | Mode::Normal
        | Mode::AttributeCreateDialog
        | Mode::AttributeDeleteDialog
        | Mode::FixedStringOverflowDialog
        | Mode::FixedStringResizeDialog => {
            let Some(selected_node) = state
                .treeview
                .get(state.tree_view_cursor)
                .map(|item| item.node.clone())
            else {
                render_error(frame, "Error: no tree node is currently selected");
                return;
            };
            match render_main_display(frame, &main_display_area, &selected_node, state) {
                Ok(()) => {}
                Err(error) => render_error(frame, &format!("Error: {error}")),
            }
        }
        Mode::Help => {}
        Mode::Logs => {}
        Mode::MultiChart => {}
    }

    match state.mode {
        Mode::Command => render_command_dialog(frame, command_area, state),
        Mode::AttributeCreateDialog => render_attribute_create_dialog(frame, content_area, state),
        Mode::AttributeDeleteDialog => render_attribute_delete_dialog(frame, content_area, state),
        Mode::FixedStringOverflowDialog => {
            render_fixed_string_overflow_dialog(frame, content_area, state)
        }
        Mode::FixedStringResizeDialog => {
            render_fixed_string_resize_dialog(frame, content_area, state)
        }
        _ => {}
    }
    render_toast_overlay(frame, state, command_area);
}

pub(super) fn render_error(frame: &mut Frame<'_>, error: &str) {
    let error_text = Text::from(error);
    let error_paragraph = Paragraph::new(error_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(
                    Style::default().fg(configure::themed_color(|colors| colors.text.error)),
                )
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(configure::configured_symbol(|symbols| symbols.title.error))
                .title_style(
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.surface.panel_title))
                        .bold(),
                )
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(error_paragraph, frame.area());
}

fn make_panels_rect(
    area: Rect,
    mode: &Mode,
    focus: &Focus,
    treeview: &[TreeItem<'_>],
) -> Rc<[Rect]> {
    if let Mode::Search = mode {
        Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(100), Constraint::Percentage(0)])
            .split(area)
    } else {
        let layout = configure::current_auto_layout_settings();
        let tree_focus = match focus {
            Focus::Tree(_) => PanelFocus::Focused,
            Focus::Attributes | Focus::Content => PanelFocus::Unfocused,
        };
        let focused_tree_constraint =
            tree_constraint(&layout.tree.focused, preferred_tree_panel_width(treeview));
        let tree_constraint = match tree_focus {
            PanelFocus::Focused => focused_tree_constraint,
            PanelFocus::Unfocused => layout.tree.unfocused.as_constraint(),
        };
        if area.width < 100 {
            let chunks = Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([tree_constraint, Constraint::Fill(1)])
                .split(area);
            return chunks;
        }

        Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([tree_constraint, Constraint::Fill(1)])
            .split(area)
    }
}

fn preferred_tree_panel_width(treeview: &[TreeItem<'_>]) -> Option<u16> {
    let widest_line = treeview.iter().map(|item| item.line.width() as u16).max()?;
    Some(widest_line.saturating_add(4).max(12))
}

fn tree_constraint(size: &configure::LayoutSize, preferred_width: Option<u16>) -> Constraint {
    match (size, preferred_width) {
        (configure::LayoutSize::Max(cap), Some(preferred)) => {
            Constraint::Length(preferred.min(*cap).max(12))
        }
        (configure::LayoutSize::Min(floor), Some(preferred)) => {
            Constraint::Length(preferred.max(*floor))
        }
        _ => size.as_constraint(),
    }
}

#[derive(Clone, Copy)]
enum PanelFocus {
    Focused,
    Unfocused,
}

fn use_stacked_tree_layout(area: Rect, mode: &Mode, show_tree_view: bool) -> bool {
    show_tree_view && !matches!(mode, Mode::Search) && area.width < 100
}

fn render_header(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState<'_>,
    new_version: Option<&str>,
) -> Rect {
    if area.height <= HEADER_HEIGHT {
        return area;
    }

    let sections =
        Layout::vertical([Constraint::Length(HEADER_HEIGHT), Constraint::Min(0)]).split(area);
    let header_area = sections[0];
    let body_area = sections[1];

    let columns = Layout::horizontal([
        Constraint::Percentage(32),
        Constraint::Percentage(40),
        Constraint::Percentage(28),
    ])
    .split(header_area);

    frame.render_widget(
        Paragraph::new(Line::raw("")).style(
            Style::default()
                .bg(configure::themed_color(|colors| colors.surface.bg_val3))
                .fg(configure::themed_color(|colors| colors.text.primary)),
        ),
        header_area,
    );

    let left = Line::from(vec![
        Span::styled(
            if state.readonly {
                configure::configured_symbol(|symbols| symbols.badge.readonly)
            } else {
                configure::configured_symbol(|symbols| symbols.badge.writable)
            },
            Style::default()
                .fg(if state.readonly {
                    configure::themed_color(|colors| colors.status.readonly)
                } else {
                    configure::themed_color(|colors| colors.status.writable)
                })
                .bold(),
        ),
        if state.file_watch.linked {
            Span::styled(
                configure::configured_symbol(|symbols| symbols.badge.linked),
                Style::default().fg(configure::themed_color(|colors| colors.status.linked)),
            )
        } else {
            Span::raw("")
        },
        if state.compatibility_mode {
            Span::styled(
                configure::configured_symbol(|symbols| symbols.badge.compatibility_mode),
                Style::default()
                    .fg(configure::themed_color(|colors| colors.status.compability))
                    .bold(),
            )
        } else {
            Span::raw("")
        },
        if state.configuration_warning.is_some() {
            Span::styled(
                " ! config ",
                Style::default()
                    .fg(configure::themed_color(|colors| colors.toast.warning))
                    .bold(),
            )
        } else {
            Span::raw("")
        },
    ]);
    frame.render_widget(Paragraph::new(left).style(primary_text_style()), columns[0]);

    let mut center = vec![
        Span::styled(
            " h5v ",
            Style::default()
                .fg(configure::themed_color(|colors| colors.content.app_brand))
                .bg(configure::themed_color(|colors| colors.surface.title_bg))
                .bold(),
        ),
        Span::raw(" "),
        Span::styled(
            GIT_VERSION,
            Style::default()
                .fg(configure::themed_color(|colors| colors.content.app_version))
                .bold(),
        ),
    ];
    if let Some(new_version) = new_version {
        center.push(Span::raw("  "));
        center.push(Span::styled(
            format!("update available: {new_version}"),
            Style::default()
                .fg(configure::themed_color(|colors| {
                    colors.status.update_available
                }))
                .bold(),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(center))
            .style(primary_text_style())
            .alignment(Alignment::Center),
        columns[1],
    );

    let mchart_label = format!(
        " 📊 mchart [{}/{}] ",
        state.multi_chart.visible_item_count(),
        state.multi_chart.chart_items().len()
    );
    let mchart_style = if matches!(state.mode, Mode::MultiChart) {
        Style::default()
            .fg(configure::themed_color(|colors| colors.accent.selection_fg))
            .bg(configure::themed_color(|colors| colors.accent.selection_bg))
            .bold()
    } else {
        Style::default()
            .fg(configure::themed_color(|colors| colors.help.description))
            .bg(configure::themed_color(|colors| colors.surface.help_key_bg))
            .bold()
    };
    let right = Line::from(vec![
        Span::styled(mchart_label.clone(), mchart_style),
        Span::raw(" "),
        Span::styled(
            "(type ? for help)",
            Style::default().fg(configure::themed_color(|colors| colors.content.help_hint)),
        ),
    ]);
    let health_badge = health_badge_spans(state);
    let right = if health_badge.is_empty() {
        right
    } else {
        let mut spans = right.spans;
        spans.push(Span::raw(" "));
        spans.extend(health_badge);
        Line::from(spans)
    };
    let right_width = right.width() as u16;
    let mchart_width = Line::from(mchart_label.as_str()).width() as u16;
    let right_start_x = columns[2]
        .x
        .saturating_add(columns[2].width.saturating_sub(right_width));
    state.ui_layout.mchart_toggle = Some(Rect {
        x: right_start_x,
        y: columns[2].y,
        width: mchart_width,
        height: columns[2].height,
    });
    state.ui_layout.help_toggle = Some(Rect {
        x: right_start_x.saturating_add(mchart_width + 1),
        y: columns[2].y,
        width: right_width.saturating_sub(mchart_width + 1),
        height: columns[2].height,
    });
    frame.render_widget(
        Paragraph::new(right).alignment(Alignment::Right),
        columns[2],
    );
    body_area
}

fn health_badge_spans(state: &AppState<'_>) -> Vec<Span<'static>> {
    let (warning_count, fail_count) = health_issue_counts(state);
    let mut spans = Vec::new();
    if warning_count > 0 {
        spans.push(Span::styled(
            format!(
                "{warning_count}{}",
                health_status_symbol(HealthStatus::Warning)
            ),
            Style::default()
                .fg(configure::themed_color(|colors| colors.toast.warning))
                .bold(),
        ));
    }
    if fail_count > 0 {
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(
            format!("{fail_count}{}", health_status_symbol(HealthStatus::Fail)),
            Style::default()
                .fg(configure::themed_color(|colors| colors.text.error))
                .bold(),
        ));
    }
    spans
}

fn health_issue_counts(state: &AppState<'_>) -> (usize, usize) {
    let runtime = crate::compat::run_runtime_healthcheck(
        crate::compat::current(),
        state.image_protocol_enabled,
    );
    let snapshot = configure::current_registry_snapshot();
    let plugin_statuses = snapshot.plugins().map(|plugin| plugin.health_status);
    let reported_statuses = crate::health::reported_health_issues()
        .into_iter()
        .map(|issue| issue.result.status);
    runtime
        .into_iter()
        .map(|result| result.status)
        .chain(plugin_statuses)
        .chain(reported_statuses)
        .fold((0usize, 0usize), |(warning, fail), status| match status {
            HealthStatus::Healthy => (warning, fail),
            HealthStatus::Warning => (warning + 1, fail),
            HealthStatus::Fail => (warning, fail + 1),
        })
}

fn health_status_symbol(status: HealthStatus) -> &'static str {
    match status {
        HealthStatus::Healthy => "●",
        HealthStatus::Warning => "▲",
        HealthStatus::Fail => "✖",
    }
}

fn command_modal_area(area: Rect) -> Rect {
    if area.width == 0 || area.height == 0 {
        return Rect::new(0, 0, 0, 0);
    }
    let width = area.width.clamp(24, 96);
    let height = area.height.min(COMMAND_BAR_HEIGHT.max(6));
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(3)
        .min(area.bottom().saturating_sub(height).max(area.y));
    Rect::new(x, y, width, height)
}

fn render_toast_overlay(frame: &mut Frame<'_>, state: &AppState<'_>, command_area: Rect) {
    let Some((label, message, accent_color)) = toast_parts(&state.toast) else {
        return;
    };
    let area = toast_overlay_area(frame.area(), command_area);
    if area.width == 0 || area.height == 0 {
        return;
    }

    let base_bg = configure::themed_color(|colors| colors.surface.title_bg);
    let base_fg = configure::themed_color(|colors| colors.text.primary);
    let label_fg = configure::themed_color(|colors| colors.surface.bg);
    let label_text = format!(" {label} ");
    let available_message_width = area
        .width
        .saturating_sub(label_text.chars().count() as u16)
        .saturating_sub(1) as usize;
    let message = truncate_to_width(message, available_message_width);

    let line = Line::from(vec![
        Span::styled(
            label_text,
            Style::default().fg(label_fg).bg(accent_color).bold(),
        ),
        Span::styled(" ", Style::default().bg(base_bg)),
        Span::styled(message, Style::default().fg(base_fg).bg(base_bg)),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(base_bg)),
        area,
    );
}

fn toast_overlay_area(frame_area: Rect, command_area: Rect) -> Rect {
    if frame_area.width == 0 || frame_area.height == 0 {
        return Rect::new(0, 0, 0, 0);
    }
    let command_is_bottom_docked =
        command_area.height > 0 && command_area.y > frame_area.y + frame_area.height / 2;
    let y = if command_is_bottom_docked && command_area.y > frame_area.y {
        command_area.y.saturating_sub(1)
    } else {
        frame_area.bottom().saturating_sub(1)
    };
    Rect::new(frame_area.x, y, frame_area.width, 1)
}

fn toast_parts(toast: &AppToast) -> Option<(&'static str, &str, ratatui::style::Color)> {
    match toast {
        AppToast::Empty => None,
        AppToast::Info(message) => Some((
            "INFO",
            message.as_str(),
            configure::themed_color(|colors| colors.toast.info),
        )),
        AppToast::Warning(message) => Some((
            "WARNING",
            message.as_str(),
            configure::themed_color(|colors| colors.toast.warning),
        )),
        AppToast::Error(message) => Some((
            "ERROR",
            message.as_str(),
            configure::themed_color(|colors| colors.text.error),
        )),
    }
}
