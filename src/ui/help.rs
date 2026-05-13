use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::configure;

pub fn render_help(frame: &mut Frame<'_>, area: Rect) {
    let popup = centered_rect(area, 140, 31);

    frame.render_widget(
        Block::default()
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.bg_val3))),
        area,
    );
    frame.render_widget(Clear, popup);

    let help_block = Block::default()
        .borders(Borders::ALL)
        .border_style(
            Style::default().fg(configure::themed_color(|colors| colors.surface.break_line)),
        )
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(configure::configured_symbol(|symbols| symbols.title.help))
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.help.title))
                .bold(),
        )
        .title_bottom(Line::from(vec![
            Span::styled(" Esc ", help_key_style()),
            Span::styled(" close ", help_desc_style()),
        ]))
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg)));
    frame.render_widget(help_block, popup);

    let inner = popup.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let columns = Layout::horizontal([
        Constraint::Percentage(37),
        Constraint::Percentage(30),
        Constraint::Percentage(33),
    ])
    .split(inner);

    let column_style =
        Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg));
    frame.render_widget(
        Paragraph::new(render_help_column_text(
            "General",
            &[
                (
                    "Move",
                    &[
                        (&["j", "k", "↑", "↓"], "move"),
                        (&["h", "l", "←", "→"], "open / close / move"),
                        (&["g", "Home", "G", "End"], "top / bottom"),
                        (&["Ctrl-U", "PgUp", "Ctrl-D", "PgDn"], "half-page"),
                    ],
                ),
                (
                    "Panes",
                    &[
                        (&["Shift + ←↑↓→"], "focus"),
                        (&["Ctrl-W", "h/j/k/l"], "vim focus"),
                        (&["s", "Ctrl-W o"], "toggle sidebar"),
                    ],
                ),
            ],
        ))
        .style(column_style)
        .wrap(Wrap { trim: true }),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(render_help_column_text(
            "Views",
            &[
                (
                    "View",
                    &[
                        (&["Tab"], "preview / matrix / heatmap / schema"),
                        (&["y"], "copy selected"),
                        (
                            &["a", "d", "Delete"],
                            "create / delete attribute (attrs pane)",
                        ),
                        (&["Esc"], "cancel active popup"),
                        (&["j/k", "PgUp/PgDn"], "navigate large preview segments"),
                        (&["compound root"], "recursive schema preview"),
                        (&["m", "M"], "add / open chart"),
                    ],
                ),
                (
                    "Selectors",
                    &[
                        (&["x", "X"], "preview x-axis"),
                        (&["r", "R"], "matrix row axis"),
                        (&["c", "C"], "matrix col axis"),
                        (&["[", "]"], "selected dim"),
                        (&["Ctrl-X", "Ctrl-A"], "index - / +"),
                    ],
                ),
                (
                    "Heatmap",
                    &[
                        (&["↑↓", "←→"], "setting row / value"),
                        (&["PgUp", "PgDn"], "heatmap page"),
                        (&["z", "Z", "0", "v"], "zoom in / out / reset / clear"),
                        (&["H", "J", "K", "L"], "pan zoomed viewport"),
                        (&["mouse"], "left select, wheel zoom, right-drag pan"),
                    ],
                ),
            ],
        ))
        .style(column_style)
        .wrap(Wrap { trim: true }),
        columns[1],
    );
    frame.render_widget(
        Paragraph::new(render_help_column_text(
            "Modes",
            &[
                (
                    "Search + commands",
                    &[
                        (&["/"], "search"),
                        (&[":"], "command mode"),
                        (&["."], "repeat command"),
                        (&["help", "help reload"], "help overlay / command help"),
                        (&["goto /group/dataset"], "jump to an HDF5 path"),
                        (
                            &["attr create title string \"hello\""],
                            "create scalar attribute on the selected node",
                        ),
                        (
                            &["attr delete title"],
                            "delete attribute from the selected node",
                        ),
                        (
                            &["configure"],
                            "open init.lua in $VISUAL/$EDITOR and reload it on return",
                        ),
                        (
                            &["configure reset"],
                            "overwrite init.lua with the default scaffold and reload it",
                        ),
                        (
                            &["mchart add /group/dataset[..,0]"],
                            "add a dataset to multichart from anywhere",
                        ),
                        (
                            &["mchart expr \"($1, !/ticks + #/calibration/offset)\""],
                            "create a derived multichart series directly",
                        ),
                        (
                            &["press ctrl+w o", "press M j enter"],
                            "drive existing keymaps from scripts or command mode",
                        ),
                        (&["Tab", "Shift-Tab"], "complete next / prev"),
                        (&["↑", "↓"], "suggestion select"),
                        (&["Ctrl-P", "Ctrl-N"], "history prev / next"),
                        (&["42", "+7", "-3"], "legacy seek / down / up"),
                        (&["Enter", "Esc"], "run / cancel"),
                    ],
                ),
                ("File", &[(&["Ctrl-R"], "reload file")]),
                (
                    "Multi chart",
                    &[
                        (&["M", "Esc"], "open / close"),
                        (&["j", "k"], "select series"),
                        (&["m"], "add current previewable selection from tree"),
                        (
                            &["e"],
                            "open expression prompt ($id, !/path[..], #/path, !/path:attr, or (x,y))",
                        ),
                        (&["Space"], "mark / unmark base series"),
                        (
                            &["D", "S", "R", "P", "X"],
                            "base op selected => diff / sum / ratio / product / x-y",
                        ),
                        (&["Enter", "v"], "hide / show selected series"),
                        (&["h", "l", "Shift+←→"], "pan"),
                        (&["+", "-", "Shift+↑↓"], "zoom"),
                        (&["d", "Backspace", "Delete"], "remove"),
                        (&["C"], "clear all"),
                        (&["c"], "reset zoom"),
                        (&["q", "Ctrl-C"], "quit app"),
                    ],
                ),
                ("Other", &[(&["?"], "help"), (&["q", "Ctrl-C"], "quit")]),
            ],
        ))
        .style(column_style)
        .wrap(Wrap { trim: true }),
        columns[2],
    );
}

pub fn centered_rect(area: Rect, max_width: u16, max_height: u16) -> Rect {
    let width = area.width.saturating_sub(4).min(max_width).max(20);
    let height = area.height.saturating_sub(4).min(max_height).max(10);

    let vertical = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .split(area);
    let horizontal = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width),
        Constraint::Fill(1),
    ])
    .split(vertical[1]);
    horizontal[1]
}

fn help_key_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.text.primary))
        .bg(configure::themed_color(|colors| colors.surface.help_key_bg))
        .underlined()
        .bold()
}

fn help_section_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.help.section))
        .bold()
        .underlined()
}

fn help_desc_style() -> Style {
    Style::default().fg(configure::themed_color(|colors| colors.help.description))
}

fn help_muted_style() -> Style {
    Style::default().fg(configure::themed_color(|colors| colors.help.muted))
}

fn help_keys(keys: &[&'static str], desc: &'static str) -> Line<'static> {
    let mut spans = Vec::new();
    for (idx, key) in keys.iter().enumerate() {
        spans.push(Span::styled(format!(" {key} "), help_key_style()));
        if idx + 1 != keys.len() {
            spans.push(Span::styled("  ", help_muted_style()));
        }
    }
    spans.push(Span::raw("  "));
    spans.push(Span::styled(desc.to_string(), help_desc_style()));
    Line::from(spans)
}

fn help_section(
    title: &'static str,
    entries: &[(&[&'static str], &'static str)],
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(Span::styled(
        title.to_string(),
        help_section_style(),
    ))];
    for (keys, desc) in entries {
        lines.push(help_keys(keys, desc));
    }
    lines
}

type HelpSection<'a> = (&'static str, &'a [(&'a [&'static str], &'static str)]);

fn render_help_column_text(title: &'static str, sections: &[HelpSection]) -> Text<'static> {
    let mut lines = vec![
        Line::from(vec![Span::styled(
            title.to_string(),
            Style::default()
                .fg(configure::themed_color(|colors| colors.help.title))
                .bold(),
        )])
        .centered(),
        Line::raw(""),
    ];

    for (idx, (section_title, entries)) in sections.iter().enumerate() {
        lines.extend(help_section(section_title, entries));
        if idx + 1 != sections.len() {
            lines.push(Line::raw(""));
        }
    }

    Text::from(lines)
}
