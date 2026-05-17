use std::collections::BTreeSet;

use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use serde_json::Value;

use crate::{
    configure,
    logging::APP_LOG_HANDLE,
    ui::{
        help::centered_rect,
        state::{
            AppState, HelpScrollbarHitbox, LogLevelFilter, LogsFilterFocus, LogsFilterHitbox,
            LogsFilterTarget,
        },
    },
};

#[derive(Debug, Clone)]
struct ParsedLogEntry {
    timestamp: String,
    level: String,
    handle: String,
    kind: Option<String>,
    phase: Option<String>,
    message: Option<String>,
    fields: Vec<(String, String)>,
}

pub fn render_logs(frame: &mut Frame<'_>, area: Rect, state: &mut AppState<'_>) {
    let popup = centered_rect(area, 186, 48);
    state.ui_layout.logs_top_bar = Some(Rect {
        x: popup.x.saturating_add(1),
        y: popup.y,
        width: popup.width.saturating_sub(2),
        height: 1,
    });
    state.ui_layout.logs_content = None;
    state.ui_layout.logs_scrollbar = None;
    state.ui_layout.logs_filters.clear();

    frame.render_widget(
        Block::default()
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.bg_val3))),
        area,
    );
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(
            Style::default().fg(configure::themed_color(|colors| colors.surface.break_line)),
        )
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title("Logs")
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.help.title))
                .bold(),
        )
        .title_bottom(Line::from(vec![
            Span::styled(" Esc ", shortcut_style()),
            Span::styled(" close ", desc_style()),
            Span::styled("  ", muted_style()),
            Span::styled(" PgUp / PgDn ", shortcut_style()),
            Span::styled(" scroll ", desc_style()),
            Span::styled("  ", muted_style()),
            Span::styled(" Tab / Shift+Tab ", shortcut_style()),
            Span::styled(" setting ", desc_style()),
            Span::styled("  ", muted_style()),
            Span::styled(" ←→ h/l ", shortcut_style()),
            Span::styled(" setting ", desc_style()),
            Span::styled("  ", muted_style()),
            Span::styled(" ↑↓ j/k ", shortcut_style()),
            Span::styled(" filter ", desc_style()),
        ]))
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg)));
    frame.render_widget(block, popup);

    let inner = popup.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let layout = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(inner);

    let parsed = match parsed_log_entries(state.logs.session_only) {
        Ok(entries) => entries,
        Err(error) => {
            render_filters(frame, layout[0], state, &["all".to_string()]);
            render_log_content(
                frame,
                layout[1],
                state,
                vec![Line::from(vec![
                    Span::styled("Failed to read log file: ".to_string(), error_style()),
                    Span::styled(error, desc_style()),
                ])],
            );
            return;
        }
    };
    let handle_options = handle_options(&parsed);
    let handle = current_handle_filter(state, &handle_options);
    render_filters(frame, layout[0], state, &handle_options);
    let lines = render_log_lines(
        parsed
            .iter()
            .filter(|entry| entry_matches_filters(entry, state.logs.level_filter, handle))
            .collect(),
    );
    render_log_content(frame, layout[1], state, lines);
}

pub fn cycle_logs_filter(state: &mut AppState<'_>, target: LogsFilterTarget, delta: isize) -> bool {
    match target {
        LogsFilterTarget::Scope => {
            state.logs.session_only = !state.logs.session_only;
            state.logs.scroll_offset = usize::MAX;
            true
        }
        LogsFilterTarget::Level => {
            let levels = [
                LogLevelFilter::All,
                LogLevelFilter::Error,
                LogLevelFilter::Warning,
                LogLevelFilter::Info,
                LogLevelFilter::Debug,
                LogLevelFilter::Trace,
            ];
            let current = levels
                .iter()
                .position(|level| *level == state.logs.level_filter)
                .unwrap_or(0) as isize;
            let next = (current + delta).rem_euclid(levels.len() as isize) as usize;
            let next_level = levels[next];
            if next_level == state.logs.level_filter {
                false
            } else {
                state.logs.level_filter = next_level;
                state.logs.scroll_offset = usize::MAX;
                true
            }
        }
        LogsFilterTarget::Handle => {
            let Ok(entries) = parsed_log_entries(state.logs.session_only) else {
                return false;
            };
            let handles = handle_options(&entries);
            if handles.is_empty() {
                return false;
            }
            let current = state
                .logs
                .handle_filter
                .min(handles.len().saturating_sub(1)) as isize;
            let next = (current + delta).rem_euclid(handles.len() as isize) as usize;
            if next == state.logs.handle_filter {
                false
            } else {
                state.logs.handle_filter = next;
                state.logs.scroll_offset = usize::MAX;
                true
            }
        }
    }
}

pub fn cycle_active_logs_filter(state: &mut AppState<'_>, delta: isize) -> bool {
    let target = match state.logs.filter_focus {
        LogsFilterFocus::Scope => LogsFilterTarget::Scope,
        LogsFilterFocus::Level => LogsFilterTarget::Level,
        LogsFilterFocus::Handle => LogsFilterTarget::Handle,
    };
    cycle_logs_filter(state, target, delta)
}

fn render_filters(frame: &mut Frame<'_>, area: Rect, state: &mut AppState<'_>, handles: &[String]) {
    let scope_label = if state.logs.session_only {
        "Current session"
    } else {
        "All sessions"
    };
    let handle_label = current_handle_filter(state, handles);
    let items = [
        (
            LogsFilterTarget::Scope,
            format!(" Scope: {scope_label} "),
            matches!(state.logs.filter_focus, LogsFilterFocus::Scope),
        ),
        (
            LogsFilterTarget::Level,
            format!(" Level: {} ", state.logs.level_filter.as_str()),
            matches!(state.logs.filter_focus, LogsFilterFocus::Level),
        ),
        (
            LogsFilterTarget::Handle,
            format!(" Handle: {handle_label} "),
            matches!(state.logs.filter_focus, LogsFilterFocus::Handle),
        ),
    ];
    let mut spans = Vec::new();
    let mut pill_layout = Vec::new();
    for (index, (target, label, selected)) in items.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", muted_style()));
        }
        let style = if *selected {
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
        pill_layout.push((*target, label.clone(), label.chars().count() as u16));
        spans.push(Span::styled(label.clone(), style));
    }
    let line = Line::from(spans);
    let line_width = line.width() as u16;
    let start_x = area
        .x
        .saturating_add(area.width.saturating_sub(line_width) / 2);
    let separator_width = 2u16;
    let mut current_x = start_x;
    for (index, (target, _, width)) in pill_layout.iter().enumerate() {
        state.ui_layout.logs_filters.push(LogsFilterHitbox {
            area: Rect {
                x: current_x,
                y: area.y.saturating_add(1),
                width: *width,
                height: 1,
            },
            target: *target,
        });
        current_x = current_x.saturating_add(*width);
        if index + 1 != pill_layout.len() {
            current_x = current_x.saturating_add(separator_width);
        }
    }
    frame.render_widget(
        Paragraph::new(line)
            .alignment(Alignment::Center)
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg))),
        area,
    );
}

fn render_log_content(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState<'_>,
    lines: Vec<Line<'static>>,
) {
    frame.render_widget(panel_block("Entries"), area);
    let inner = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    state.ui_layout.logs_content = Some(inner);
    if inner.width == 0 || inner.height == 0 {
        state.logs.content_lines = 0;
        state.logs.viewport_lines = 0;
        return;
    }

    let viewport_lines = inner.height as usize;
    let mut wrapped_lines = wrap_lines(&lines, inner.width as usize);
    let mut total_lines = wrapped_lines.len().max(1);
    let mut max_scroll = total_lines.saturating_sub(viewport_lines);
    let show_scrollbar = max_scroll > 0 && inner.width > 3;
    let (content_area, scrollbar_area) = if show_scrollbar {
        let split = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)])
            .spacing(1)
            .split(inner);
        wrapped_lines = wrap_lines(&lines, split[0].width as usize);
        total_lines = wrapped_lines.len().max(1);
        max_scroll = total_lines.saturating_sub(viewport_lines);
        (split[0], Some(split[1]))
    } else {
        (inner, None)
    };

    state.logs.viewport_lines = viewport_lines;
    state.logs.content_lines = total_lines;
    if state.logs.scroll_offset > max_scroll {
        state.logs.scroll_offset = max_scroll;
    }
    state.ui_layout.logs_content = Some(content_area);
    frame.render_widget(
        Paragraph::new(Text::from(wrapped_lines))
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg)))
            .scroll((state.logs.scroll_offset.min(u16::MAX as usize) as u16, 0)),
        content_area,
    );
    if let Some(scrollbar_area) = scrollbar_area {
        state.ui_layout.logs_scrollbar = Some(HelpScrollbarHitbox {
            area: scrollbar_area,
            content_lines: total_lines,
            viewport_lines,
        });
        render_scrollbar(
            frame,
            scrollbar_area,
            state.logs.scroll_offset,
            total_lines,
            viewport_lines,
        );
    }
}

fn parsed_log_entries(session_only: bool) -> Result<Vec<ParsedLogEntry>, String> {
    let contents = crate::logging::read_log_text(session_only)?;
    let lines = contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let start = lines.len().saturating_sub(800);
    Ok(lines[start..]
        .iter()
        .filter_map(|line| parse_log_entry(line))
        .collect())
}

fn parse_log_entry(line: &str) -> Option<ParsedLogEntry> {
    match serde_json::from_str::<Value>(line) {
        Ok(Value::Object(mut entry)) => {
            let timestamp = take_string(&mut entry, "timestamp").unwrap_or_default();
            let level = take_string(&mut entry, "level").unwrap_or_else(|| "info".to_string());
            let handle =
                take_string(&mut entry, "handle").unwrap_or_else(|| APP_LOG_HANDLE.to_string());
            let kind = take_string(&mut entry, "kind");
            let phase = take_string(&mut entry, "phase");
            let message = take_string(&mut entry, "message");
            let mut fields = entry
                .into_iter()
                .filter(|(_, value)| !value.is_null())
                .map(|(key, value)| (key, format_value(&value)))
                .collect::<Vec<_>>();
            fields.sort_by(|(left, _), (right, _)| left.cmp(right));
            Some(ParsedLogEntry {
                timestamp,
                level,
                handle,
                kind,
                phase,
                message,
                fields,
            })
        }
        Ok(other) => Some(ParsedLogEntry {
            timestamp: String::new(),
            level: "info".to_string(),
            handle: APP_LOG_HANDLE.to_string(),
            kind: Some("json".to_string()),
            phase: None,
            message: Some(format_value(&other)),
            fields: Vec::new(),
        }),
        Err(_) => Some(ParsedLogEntry {
            timestamp: String::new(),
            level: "info".to_string(),
            handle: APP_LOG_HANDLE.to_string(),
            kind: Some("log".to_string()),
            phase: None,
            message: Some(line.to_string()),
            fields: Vec::new(),
        }),
    }
}

fn handle_options(entries: &[ParsedLogEntry]) -> Vec<String> {
    let mut handles = BTreeSet::from([APP_LOG_HANDLE.to_string()]);
    handles.extend(
        configure::current_registry_snapshot()
            .plugins()
            .map(|plugin| plugin.handle.as_str().to_string()),
    );
    handles.extend(entries.iter().map(|entry| entry.handle.clone()));
    std::iter::once("all".to_string()).chain(handles).collect()
}

fn current_handle_filter<'a>(state: &mut AppState<'_>, handles: &'a [String]) -> &'a str {
    if handles.is_empty() {
        return "all";
    }
    if state.logs.handle_filter >= handles.len() {
        state.logs.handle_filter = 0;
    }
    handles[state.logs.handle_filter].as_str()
}

fn entry_matches_filters(
    entry: &ParsedLogEntry,
    level_filter: LogLevelFilter,
    handle: &str,
) -> bool {
    let level_matches = match level_filter {
        LogLevelFilter::All => true,
        LogLevelFilter::Error => entry.level.eq_ignore_ascii_case("error"),
        LogLevelFilter::Warning => {
            entry.level.eq_ignore_ascii_case("warn") || entry.level.eq_ignore_ascii_case("warning")
        }
        LogLevelFilter::Info => entry.level.eq_ignore_ascii_case("info"),
        LogLevelFilter::Debug => entry.level.eq_ignore_ascii_case("debug"),
        LogLevelFilter::Trace => entry.level.eq_ignore_ascii_case("trace"),
    };
    let handle_matches = handle.eq_ignore_ascii_case("all") || entry.handle == handle;
    level_matches && handle_matches
}

fn render_log_lines(entries: Vec<&ParsedLogEntry>) -> Vec<Line<'static>> {
    if entries.is_empty() {
        return vec![Line::from(Span::styled(
            "No log entries match the current filters.".to_string(),
            desc_style(),
        ))];
    }
    let mut lines = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        lines.push(log_header_line(entry));
        for (key, value) in &entry.fields {
            lines.push(Line::from(vec![
                Span::styled("  ".to_string(), muted_style()),
                Span::styled(format!("{key}: "), muted_style()),
                Span::styled(value.clone(), desc_style()),
            ]));
        }
        if index + 1 != entries.len() {
            lines.push(Line::raw(""));
        }
    }
    lines
}

fn log_header_line(entry: &ParsedLogEntry) -> Line<'static> {
    let mut spans = Vec::new();
    if !entry.timestamp.trim().is_empty() {
        spans.push(Span::styled(format!("{} ", entry.timestamp), muted_style()));
    }
    spans.push(Span::styled(
        format!("[{}]", display_level_label(&entry.level)),
        level_style(&entry.level),
    ));
    spans.push(Span::styled(" ".to_string(), desc_style()));
    spans.push(Span::styled(entry.handle.clone(), handle_style()));
    if let Some(kind) = entry.kind.as_deref().filter(|kind| !kind.trim().is_empty()) {
        spans.push(Span::styled(" / ".to_string(), muted_style()));
        spans.push(Span::styled(kind.to_string(), title_style()));
    }
    if let Some(phase) = entry
        .phase
        .as_deref()
        .filter(|phase| !phase.trim().is_empty())
    {
        spans.push(Span::styled(" / ".to_string(), muted_style()));
        spans.push(Span::styled(phase.to_string(), desc_style()));
    }
    if let Some(message) = entry
        .message
        .as_deref()
        .filter(|message| !message.trim().is_empty())
    {
        spans.push(Span::styled(": ".to_string(), muted_style()));
        spans.push(Span::styled(message.to_string(), desc_style()));
    }
    Line::from(spans)
}

fn take_string(entry: &mut serde_json::Map<String, Value>, key: &str) -> Option<String> {
    entry.remove(key).and_then(|value| match value {
        Value::String(value) => Some(value),
        other => Some(format_value(&other)),
    })
}

fn format_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| other.to_string()),
    }
}

fn render_scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    scroll_offset: usize,
    total_lines: usize,
    viewport_lines: usize,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let track_len = area.height as usize;
    let thumb_len = ((viewport_lines.saturating_mul(track_len) + total_lines.saturating_sub(1))
        / total_lines.max(1))
    .max(1)
    .min(track_len);
    let max_scroll = total_lines.saturating_sub(viewport_lines);
    let thumb_start = if max_scroll == 0 || track_len <= thumb_len {
        0
    } else {
        scroll_offset.saturating_mul(track_len.saturating_sub(thumb_len)) / max_scroll
    };
    let thumb_end = thumb_start.saturating_add(thumb_len).min(track_len);
    let lines = (0..track_len)
        .map(|idx| {
            if (thumb_start..thumb_end).contains(&idx) {
                Line::from(Span::styled("█", scrollbar_thumb_style()))
            } else {
                Line::from(Span::styled("│", scrollbar_track_style()))
            }
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg))),
        area,
    );
}

fn wrap_lines(lines: &[Line<'static>], width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut wrapped = Vec::new();
    for line in lines {
        if line.spans.is_empty() || line.width() == 0 {
            wrapped.push(Line::default());
            continue;
        }
        let mut current_spans = Vec::new();
        let mut current_width = 0usize;
        for span in &line.spans {
            let mut remaining = span.content.to_string();
            if remaining.is_empty() {
                continue;
            }
            while !remaining.is_empty() {
                if current_width == width {
                    wrapped.push(Line::from(current_spans));
                    current_spans = Vec::new();
                    current_width = 0;
                }
                let available = width.saturating_sub(current_width).max(1);
                let (chunk, rest) = split_prefix_by_width(&remaining, available);
                if chunk.is_empty() {
                    break;
                }
                current_width += chunk.chars().count();
                current_spans.push(Span::styled(chunk, span.style));
                remaining = rest;
                if current_width == width {
                    wrapped.push(Line::from(current_spans));
                    current_spans = Vec::new();
                    current_width = 0;
                }
            }
        }
        if !current_spans.is_empty() {
            wrapped.push(Line::from(current_spans));
        }
    }
    if wrapped.is_empty() {
        wrapped.push(Line::default());
    }
    wrapped
}

fn split_prefix_by_width(text: &str, width: usize) -> (String, String) {
    if width == 0 {
        return (String::new(), text.to_string());
    }
    let mut end = text.len();
    let mut count = 0usize;
    for (idx, ch) in text.char_indices() {
        if count == width {
            end = idx;
            break;
        }
        count += 1;
        end = idx + ch.len_utf8();
    }
    if count <= width && end == text.len() {
        (text.to_string(), String::new())
    } else {
        (text[..end].to_string(), text[end..].to_string())
    }
}

fn panel_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(
            Style::default().fg(configure::themed_color(|colors| colors.surface.break_line)),
        )
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(title.to_string())
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.help.title))
                .bold(),
        )
}

fn desc_style() -> Style {
    Style::default().fg(configure::themed_color(|colors| colors.help.description))
}

fn muted_style() -> Style {
    Style::default().fg(configure::themed_color(|colors| colors.help.muted))
}

fn title_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.help.section))
        .bold()
}

fn handle_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.accent.selection_fg))
        .bg(configure::themed_color(|colors| colors.accent.selection_bg))
        .bold()
}

fn shortcut_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.text.primary))
        .bg(configure::themed_color(|colors| colors.surface.help_key_bg))
        .underlined()
        .bold()
}

fn error_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.text.error))
        .bold()
}

fn level_style(level: &str) -> Style {
    let color = match level {
        "error" => configure::themed_color(|colors| colors.text.error),
        "warn" | "warning" => configure::themed_color(|colors| colors.toast.warning),
        "debug" | "trace" => configure::themed_color(|colors| colors.help.muted),
        _ => configure::themed_color(|colors| colors.toast.info),
    };
    Style::default().fg(color).bold()
}

fn display_level_label(level: &str) -> &'static str {
    match level {
        "warn" | "warning" => "WARNING",
        "error" => "ERROR",
        "debug" => "DEBUG",
        "trace" => "TRACE",
        _ => "INFO",
    }
}

fn scrollbar_track_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.help.muted))
        .bg(configure::themed_color(|colors| colors.surface.focus_bg))
}

fn scrollbar_thumb_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.text.primary))
        .bg(configure::themed_color(|colors| colors.surface.focus_bg))
        .bold()
}
