use std::io::IsTerminal;

use ratatui::text::{Line, Span};

use crate::{
    compat::run_runtime_healthcheck, configure, health::HealthStatus, ui::state::AppState,
};

use super::{
    framed_example_lines, help_desc_style, help_muted_style, metadata_line, paragraph_line,
    section_title_line,
};

pub(in crate::ui::help) fn health_panel_text(
    state: &AppState<'_>,
    section: usize,
) -> (String, Vec<Line<'static>>) {
    if section == 0 {
        return runtime_health_panel_text(state);
    }
    let plugins = configure::current_registry_snapshot()
        .plugins()
        .cloned()
        .collect::<Vec<_>>();
    let Some(plugin) = plugins.get(section.saturating_sub(1)) else {
        return (
            "Health".to_string(),
            vec![paragraph_line("Health section unavailable.")],
        );
    };
    plugin_health_panel_text(plugin)
}

fn runtime_health_panel_text(state: &AppState<'_>) -> (String, Vec<Line<'static>>) {
    let runtime = crate::compat::current();
    let runtime_results = run_runtime_healthcheck(runtime, state.image_protocol_enabled);
    let reported_issues = crate::health::reported_health_issues();
    let status = runtime_results
        .iter()
        .map(|result| result.status)
        .chain(reported_issues.iter().map(|issue| issue.result.status))
        .max()
        .unwrap_or(HealthStatus::Healthy);
    let config_path = configure::config_path()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "unavailable".to_string());
    let config_dir = std::path::Path::new(&config_path)
        .parent()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "unavailable".to_string());
    let config_load_time = configure::last_config_load_metrics()
        .map(|metrics| format!("{} ms", metrics.total_duration_ms))
        .unwrap_or_else(|| "unavailable".to_string());
    let stdout_is_tty = yes_no(std::io::stdout().is_terminal());
    let shell = env_value("SHELL");
    let editor = env_value("EDITOR");
    let visual = env_value("VISUAL");
    let term = env_value("TERM");
    let colorterm = env_value("COLORTERM");
    let graphics_capable = yes_no(runtime.terminal_graphics && state.image_protocol_enabled);

    let mut lines = vec![
        health_status_line(
            status,
            "h5v",
            Some("Built-in runtime and terminal health overview"),
        ),
        Line::raw(""),
        section_title_line("Runtime"),
        metadata_line("config path", config_path),
        metadata_line("config dir", config_dir),
        metadata_line("config load time", config_load_time),
        metadata_line(
            "compatibility mode",
            yes_no(runtime.compatibility_mode).to_string(),
        ),
        metadata_line("stdout is tty", stdout_is_tty.to_string()),
        metadata_line(
            "graphics enabled",
            yes_no(runtime.terminal_graphics).to_string(),
        ),
        metadata_line("graphics capable", graphics_capable.to_string()),
        metadata_line("shell", shell),
        metadata_line("editor", editor),
        metadata_line("visual", visual),
        metadata_line("TERM", term),
        metadata_line("COLORTERM", colorterm),
        Line::raw(""),
        section_title_line("Checks"),
    ];
    let check_lines = runtime_results
        .into_iter()
        .map(|result| {
            health_status_line(
                result.status,
                "runtime check",
                Some(result.message.as_str()),
            )
        })
        .collect::<Vec<_>>();
    lines.extend(check_lines);
    if !reported_issues.is_empty() {
        lines.push(Line::raw(""));
        lines.push(section_title_line("Configuration and plugin load issues"));
        lines.extend(reported_issues.into_iter().map(|issue| {
            health_status_line(
                issue.result.status,
                issue.source.as_str(),
                Some(issue.result.message.as_str()),
            )
        }));
    }
    ("Health: h5v".to_string(), lines)
}

fn plugin_health_panel_text(
    plugin: &configure::registry::PluginMetadata,
) -> (String, Vec<Line<'static>>) {
    let version = plugin
        .version
        .as_ref()
        .map(|version| format!(" v{version}"))
        .unwrap_or_default();
    let title = format!("Health: {}{}", plugin.name, version);
    let mut lines = vec![
        health_status_line(
            plugin.health_status,
            &format!("{}{}", plugin.name, version),
            Some("Plugin health result"),
        ),
        Line::raw(""),
        section_title_line("Plugin"),
        metadata_line("handle", plugin.handle.as_str().to_string()),
    ];
    if let Some(source) = plugin.source.as_deref() {
        lines.push(metadata_line("source", source.to_string()));
    }
    if let Some(requested_ref) = plugin.requested_ref.as_deref() {
        lines.push(metadata_line("requested ref", requested_ref.to_string()));
    }
    if let Some(commit) = plugin.resolved_commit.as_deref() {
        lines.push(metadata_line("resolved commit", commit.to_string()));
    }
    lines.push(metadata_line(
        "auto pull",
        yes_no(plugin.auto_pull).to_string(),
    ));
    lines.push(Line::raw(""));
    lines.push(section_title_line("Message"));
    if let Some(document) = plugin.health_ui_document.as_deref() {
        match crate::ui::custom_content::render_serialized_ui_document(document, 72) {
            Ok(rendered) => lines.extend(framed_example_lines(Some("health"), rendered)),
            Err(error) => lines.extend(framed_example_lines(
                Some("health"),
                vec![Line::from(vec![
                    Span::styled(
                        "Failed to render health UI: ".to_string(),
                        help_muted_style(),
                    ),
                    Span::styled(error, help_desc_style()),
                ])],
            )),
        }
    } else {
        let message = plugin
            .health_message
            .as_deref()
            .filter(|message| !message.trim().is_empty())
            .unwrap_or("No details provided.");
        lines.extend(framed_example_lines(
            Some("health"),
            message.lines().map(paragraph_line).collect(),
        ));
    }
    (title, lines)
}

fn env_value(name: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unset".to_string())
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn health_status_line(status: HealthStatus, label: &str, message: Option<&str>) -> Line<'static> {
    let (symbol, style) = match status {
        HealthStatus::Healthy => (
            "●",
            ratatui::style::Style::default()
                .fg(configure::themed_color(|colors| colors.toast.info))
                .bold(),
        ),
        HealthStatus::Warning => (
            "▲",
            ratatui::style::Style::default()
                .fg(configure::themed_color(|colors| colors.toast.warning))
                .bold(),
        ),
        HealthStatus::Fail => (
            "✖",
            ratatui::style::Style::default()
                .fg(configure::themed_color(|colors| colors.text.error))
                .bold(),
        ),
    };
    let mut spans = vec![
        Span::styled(format!("{symbol} "), style),
        Span::styled(label.to_string(), super::help_function_name_style()),
        Span::styled(format!(" ({})", status.as_str()), help_muted_style()),
    ];
    if let Some(message) = message.filter(|message| !message.trim().is_empty()) {
        spans.push(Span::styled(": ".to_string(), help_muted_style()));
        spans.push(Span::styled(message.to_string(), help_desc_style()));
    }
    Line::from(spans)
}
