use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::{
    configure,
    ui::{
        cursor::set_input_cursor,
        help::centered_rect,
        state::{self, AppState, FixedStringOverflowChoice},
    },
};

use super::primary_text_style;

fn render_dialog_popup(
    frame: &mut Frame<'_>,
    area: Rect,
    max_width: u16,
    max_height: u16,
    title: &'static str,
) -> Rect {
    let popup = centered_rect(area, max_width, max_height);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.bg_val3))),
        popup,
    );
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(
                Style::default().fg(configure::themed_color(|colors| colors.surface.panel_title)),
            )
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(title)
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg))),
        popup,
    );
    popup
}

pub(super) fn render_attribute_create_dialog(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &AppState<'_>,
) {
    let Some(dialog) = state.attribute_create_dialog.as_ref() else {
        return;
    };

    let popup = render_dialog_popup(
        frame,
        area,
        84,
        13,
        configure::configured_symbol(|symbols| symbols.title.create_attribute),
    );

    let inner = popup.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    frame.render_widget(
        Paragraph::new("Tab/Shift-Tab switch fields, Left/Right changes type, Enter creates")
            .style(Style::default().fg(configure::themed_color(|colors| colors.text.type_desc))),
        rows[0],
    );

    let active_style = Style::default()
        .fg(configure::themed_color(|colors| colors.accent.selection_fg))
        .bg(configure::themed_color(|colors| colors.accent.selection_bg))
        .bold();
    let idle_style = Style::default().fg(configure::themed_color(|colors| colors.text.primary));
    let name_style = if dialog.active_field == state::AttributeCreateField::Name {
        active_style
    } else {
        idle_style
    };
    let type_style = if dialog.active_field == state::AttributeCreateField::Type {
        active_style
    } else {
        idle_style
    };
    let value_style = if dialog.active_field == state::AttributeCreateField::Value {
        active_style
    } else {
        idle_style
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Name: ",
                Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)),
            ),
            Span::styled(dialog.name.clone(), name_style),
        ])),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Type: ",
                Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)),
            ),
            Span::styled(
                format!(
                    "< {} >  ({})",
                    dialog.attr_type.label(),
                    dialog.attr_type.description()
                ),
                type_style,
            ),
        ])),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Value: ",
                Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)),
            ),
            Span::styled(dialog.value.clone(), value_style),
        ]))
        .wrap(Wrap { trim: false }),
        rows[3],
    );
    frame.render_widget(
        Paragraph::new("Types: bool, i64, u64, f64, string, ascii")
            .style(Style::default().fg(configure::themed_color(|colors| colors.text.type_desc))),
        rows[5],
    );

    match dialog.active_field {
        state::AttributeCreateField::Name => set_input_cursor(
            frame,
            ratatui::layout::Position::new(rows[1].x + 6 + dialog.name_cursor as u16, rows[1].y),
        ),
        state::AttributeCreateField::Type => {}
        state::AttributeCreateField::Value => set_input_cursor(
            frame,
            ratatui::layout::Position::new(rows[3].x + 7 + dialog.value_cursor as u16, rows[3].y),
        ),
    }
}

pub(super) fn render_attribute_delete_dialog(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &AppState<'_>,
) {
    let Some(dialog) = state.attribute_delete_dialog.as_ref() else {
        return;
    };

    let popup = render_dialog_popup(
        frame,
        area,
        64,
        9,
        configure::configured_symbol(|symbols| symbols.title.delete_attribute),
    );
    let inner = popup.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let rows = Layout::vertical([Constraint::Length(2), Constraint::Length(1)]).split(inner);
    frame.render_widget(
        Paragraph::new(format!(
            "Delete attribute '{}'?\nPress Enter to confirm or Esc to cancel.",
            dialog.attr_name
        ))
        .style(primary_text_style())
        .wrap(Wrap { trim: true }),
        rows[0],
    );
}

pub(super) fn render_fixed_string_overflow_dialog(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &AppState<'_>,
) {
    let Some(dialog) = state.fixed_string_overflow_dialog.as_ref() else {
        return;
    };

    let popup = render_dialog_popup(
        frame,
        area,
        72,
        12,
        configure::configured_symbol(|symbols| symbols.title.fixed_string_overflow),
    );
    let inner = popup.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(3),
    ])
    .split(inner);

    let message = Paragraph::new(format!(
        "{} needs {} bytes, current fixed size is {} bytes.",
        dialog.overflow.kind, dialog.overflow.required_size, dialog.overflow.current_size
    ))
    .style(primary_text_style())
    .wrap(Wrap { trim: true });
    frame.render_widget(message, rows[0]);

    let choices = [
        (FixedStringOverflowChoice::Cancel, "Cancel"),
        (FixedStringOverflowChoice::ChangeToVarLen, "Change to Vlen"),
        (FixedStringOverflowChoice::ChangeSize, "Change size"),
    ]
    .into_iter()
    .map(|(choice, label)| {
        let style = if dialog.selected_choice == choice {
            Style::default()
                .fg(configure::themed_color(|colors| colors.accent.selection_fg))
                .bg(configure::themed_color(|colors| colors.accent.selection_bg))
                .bold()
        } else {
            Style::default().fg(configure::themed_color(|colors| colors.text.primary))
        };
        Span::styled(format!(" {label} "), style)
    })
    .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(Line::from(choices))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        rows[2],
    );
}

pub(super) fn render_fixed_string_resize_dialog(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &AppState<'_>,
) {
    let Some(dialog) = state.fixed_string_overflow_dialog.as_ref() else {
        return;
    };

    let popup = render_dialog_popup(
        frame,
        area,
        56,
        10,
        configure::configured_symbol(|symbols| symbols.title.fixed_string_resize),
    );
    let inner = popup.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let rows = Layout::vertical([Constraint::Length(2), Constraint::Length(1)]).split(inner);

    frame.render_widget(
        Paragraph::new(format!(
            "Enter new byte size (minimum {}).",
            dialog.overflow.required_size
        ))
        .style(primary_text_style()),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(format!("> {}", dialog.size_input)).style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.text.primary))
                .bold(),
        ),
        rows[1],
    );
    set_input_cursor(
        frame,
        ratatui::layout::Position::new(rows[1].x + 2 + dialog.size_input.len() as u16, rows[1].y),
    );
}
