use ratatui::{
    layout::{Position, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::color_consts;

use super::{
    command::{
        command_keybindings, command_matches, command_usage, current_command_descriptor,
        selected_command_descriptor,
    },
    state::AppState,
};

pub fn render_command_dialog(f: &mut Frame, area: Rect, state: &mut AppState) {
    let title = match state.command_state.history_status() {
        Some((idx, total)) => format!("Command [{idx}/{total}]"),
        None => "Command".to_string(),
    };
    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(color_consts::panel_title_color()))
        .borders(Borders::ALL)
        .style(Style::default().bg(color_consts::focus_bg_color()))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color_consts::panel_border_color()));

    let command_line = Line::from(vec![
        Span::styled(
            ":",
            Style::default().fg(color_consts::key_hint_color()).bold(),
        ),
        Span::raw(" "),
        Span::raw(state.command_state.command_buffer.clone()),
    ]);

    let selected_descriptor = selected_command_descriptor(
        &state.command_state.command_buffer,
        state.command_state.selected_suggestion,
    );
    let info_descriptor =
        current_command_descriptor(&state.command_state.command_buffer).or(selected_descriptor);
    let info_line = info_descriptor.map(|descriptor| {
        let mut spans = vec![
            Span::styled(
                command_usage(descriptor),
                Style::default()
                    .fg(color_consts::command_usage_color())
                    .bold(),
            ),
            Span::raw(" "),
            Span::styled(
                format!("- {}", descriptor.description),
                Style::default().fg(color_consts::type_desc_color()),
            ),
        ];
        let keys = command_keybindings(descriptor);
        if !keys.is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("[{}]", keys),
                Style::default().fg(color_consts::key_hint_color()),
            ));
        }
        Line::from(spans)
    });

    let matches = command_matches(&state.command_state.command_buffer);
    let suggestions_line = if matches.is_empty() {
        Line::from(Span::styled(
            "No matching commands",
            Style::default().fg(color_consts::command_no_match_color()),
        ))
    } else {
        let mut spans = vec![Span::styled(
            "Suggestions: ",
            Style::default().fg(color_consts::type_desc_color()),
        )];
        for (index, descriptor) in matches.iter().take(5).enumerate() {
            if index > 0 {
                spans.push(Span::raw("  "));
            }
            let is_selected = Some(descriptor.id) == selected_descriptor.map(|d| d.id);
            let style = if is_selected {
                Style::default()
                    .fg(color_consts::selection_fg_color())
                    .bg(color_consts::selection_bg_color())
                    .bold()
            } else {
                Style::default().fg(color_consts::primary_text_color())
            };
            spans.push(Span::styled(descriptor.name, style));
        }
        Line::from(spans)
    };

    let history_hint = Line::from(vec![
        Span::styled(
            "History: ",
            Style::default().fg(color_consts::type_desc_color()),
        ),
        Span::styled(
            "Ctrl+p",
            Style::default().fg(color_consts::key_hint_color()),
        ),
        Span::raw(" / "),
        Span::styled(
            "Ctrl+n",
            Style::default().fg(color_consts::key_hint_color()),
        ),
        Span::raw("   "),
        Span::styled(
            "Complete: ",
            Style::default().fg(color_consts::type_desc_color()),
        ),
        Span::styled("Tab", Style::default().fg(color_consts::key_hint_color())),
        Span::raw("   "),
        Span::styled(
            "Legacy: ",
            Style::default().fg(color_consts::type_desc_color()),
        ),
        Span::styled("42", Style::default().fg(color_consts::key_hint_color())),
        Span::raw(" / "),
        Span::styled("+7", Style::default().fg(color_consts::key_hint_color())),
        Span::raw(" / "),
        Span::styled("-3", Style::default().fg(color_consts::key_hint_color())),
    ]);

    let command_text_widget = Paragraph::new(Text::from(vec![
        command_line,
        info_line.unwrap_or_else(|| Line::from("")),
        suggestions_line,
        history_hint,
    ]))
    .block(block)
    .style(Style::default().fg(color_consts::primary_text_color()))
    .wrap(Wrap { trim: true });

    f.render_widget(command_text_widget, area);
    let cursor_position = Position::new(area.x + 3 + state.command_state.cursor as u16, area.y + 1);
    f.set_cursor_position(cursor_position);
}
