use ratatui::{
    layout::{Position, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Clear, Paragraph, Wrap},
    Frame,
};

use crate::{
    configure,
    ui::{chrome::rounded_panel_with_style, cursor::set_input_cursor, state::AppState},
};

use super::{
    command_keybindings_metadata, command_matches, command_usage_metadata,
    current_command_metadata, selected_command_metadata,
};

fn command_body_style() -> Style {
    Style::default().fg(configure::themed_color(|colors| colors.text.primary))
}

pub fn render_command_dialog(f: &mut Frame, area: Rect, state: &mut AppState) {
    let title = match state.command_state.history_status() {
        Some((idx, total)) => format!("Command [{idx}/{total}]"),
        None => "Command".to_string(),
    };
    let block = rounded_panel_with_style(
        title,
        Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg)),
    )
    .title_style(Style::default().fg(configure::themed_color(|colors| colors.surface.panel_title)));

    let command_line = Line::from(vec![
        Span::styled(
            ":",
            Style::default()
                .fg(configure::themed_color(|colors| colors.command.key_hint))
                .bold(),
        ),
        Span::styled(" ", command_body_style()),
        Span::styled(
            state.command_state.command_buffer.clone(),
            command_body_style(),
        ),
    ]);

    let selected_descriptor = selected_command_metadata(
        &state.command_state.command_buffer,
        state.command_state.selected_suggestion,
    );
    let info_descriptor = current_command_metadata(&state.command_state.command_buffer)
        .or(selected_descriptor.clone());
    let info_line = info_descriptor.map(|descriptor| {
        let mut spans = vec![
            Span::styled(
                command_usage_metadata(&descriptor),
                Style::default()
                    .fg(configure::themed_color(|colors| colors.command.usage))
                    .bold(),
            ),
            Span::styled(" ", command_body_style()),
            Span::styled(
                format!("- {}", descriptor.summary),
                Style::default().fg(configure::themed_color(|colors| colors.command.description)),
            ),
        ];
        let keys = command_keybindings_metadata(&descriptor);
        if !keys.is_empty() {
            spans.push(Span::styled(" ", command_body_style()));
            spans.push(Span::styled(
                format!("[{}]", keys),
                Style::default().fg(configure::themed_color(|colors| colors.command.key_hint)),
            ));
        }
        Line::from(spans)
    });

    let matches = command_matches(&state.command_state.command_buffer);
    let suggestions_line = if matches.is_empty() {
        Line::from(Span::styled(
            "No matching commands",
            Style::default().fg(configure::themed_color(|colors| colors.command.no_match)),
        ))
    } else {
        let mut spans = vec![Span::styled(
            "Suggestions: ",
            Style::default().fg(configure::themed_color(|colors| {
                colors.command.suggestion_label
            })),
        )];
        for (index, descriptor) in matches.iter().take(5).enumerate() {
            if index > 0 {
                spans.push(Span::styled("  ", command_body_style()));
            }
            let is_selected = selected_descriptor
                .as_ref()
                .map(|selected| selected.handle == descriptor.handle)
                .unwrap_or(false);
            let style = if is_selected {
                Style::default()
                    .fg(configure::themed_color(|colors| colors.accent.selection_fg))
                    .bg(configure::themed_color(|colors| colors.accent.selection_bg))
                    .bold()
            } else {
                Style::default().fg(configure::themed_color(|colors| colors.text.primary))
            };
            spans.push(Span::styled(descriptor.name.clone(), style));
        }
        Line::from(spans)
    };

    let history_hint = Line::from(vec![
        Span::styled(
            "History: ",
            Style::default().fg(configure::themed_color(|colors| {
                colors.command.suggestion_label
            })),
        ),
        Span::styled(
            "Ctrl+p",
            Style::default().fg(configure::themed_color(|colors| colors.command.key_hint)),
        ),
        Span::styled(" / ", command_body_style()),
        Span::styled(
            "Ctrl+n",
            Style::default().fg(configure::themed_color(|colors| colors.command.key_hint)),
        ),
        Span::styled("   ", command_body_style()),
        Span::styled(
            "Complete: ",
            Style::default().fg(configure::themed_color(|colors| {
                colors.command.suggestion_label
            })),
        ),
        Span::styled(
            "Tab",
            Style::default().fg(configure::themed_color(|colors| colors.command.key_hint)),
        ),
        Span::styled("   ", command_body_style()),
    ]);

    let command_text_widget = Paragraph::new(Text::from(vec![
        command_line,
        info_line.unwrap_or_else(|| Line::from("")),
        suggestions_line,
        history_hint,
    ]))
    .block(block)
    .style(command_body_style())
    .wrap(Wrap { trim: true });

    f.render_widget(Clear, area);
    f.render_widget(command_text_widget, area);
    let cursor_position = Position::new(area.x + 3 + state.command_state.cursor as u16, area.y + 1);
    set_input_cursor(f, cursor_position);
}
