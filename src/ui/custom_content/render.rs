use ratatui::{
    style::Style,
    symbols::border,
    text::{Line, Span},
};

use crate::{configure, ui::std_comp_render::highlighted_lines};

use super::types::{LuaContentNode, LuaSplitDirection};

pub(crate) fn render_ui_nodes(nodes: &[LuaContentNode], width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for node in nodes {
        render_node(node, 0, width, &mut lines);
    }
    lines
}

fn render_node(node: &LuaContentNode, indent: usize, width: usize, lines: &mut Vec<Line<'static>>) {
    match node {
        LuaContentNode::Text(text) => {
            append_multiline_text(text, indent, text_style(), lines);
        }
        LuaContentNode::Code { body, kind } => {
            render_code_block(body, kind.as_deref(), indent, width, lines);
        }
        LuaContentNode::Badge(text) => {
            lines.push(Line::from(Span::styled(
                format!("{}[{}]", " ".repeat(indent), text),
                badge_style(),
            )));
        }
        LuaContentNode::KeyValue { key, value } => {
            let prefix = format!("{}{}: ", " ".repeat(indent), key);
            lines.push(Line::from(vec![
                Span::styled(prefix, key_style()),
                Span::styled(value.clone(), text_style()),
            ]));
        }
        LuaContentNode::Separator {
            label,
            empty,
            height,
        } => {
            render_separator(label.as_deref(), *empty, *height, indent, width, lines);
        }
        LuaContentNode::Row { children } => {
            if !render_inline_row(children, indent, lines) {
                for child in children {
                    render_node(child, indent, width, lines);
                }
            }
        }
        LuaContentNode::Column { children } => {
            for child in children {
                render_node(child, indent, width, lines);
            }
        }
        LuaContentNode::Split {
            direction,
            ratio_millis,
            gap,
            left,
            right,
        } => render_split(
            *direction,
            *ratio_millis,
            *gap,
            left,
            right,
            indent,
            width,
            lines,
        ),
        LuaContentNode::Table { rows } => render_table(rows, indent, lines),
        LuaContentNode::Block { title, children } => {
            let inner_width = width.saturating_sub(indent + 4).max(1);
            let mut body = render_ui_nodes(children, inner_width);
            if body.is_empty() {
                body.push(Line::from(Span::styled(String::new(), text_style())));
            }
            render_framed_lines(title.as_deref(), body, indent, width, false, lines);
        }
    }
}

fn render_inline_row(
    children: &[LuaContentNode],
    indent: usize,
    lines: &mut Vec<Line<'static>>,
) -> bool {
    let mut spans = vec![Span::raw(" ".repeat(indent))];
    for (index, child) in children.iter().enumerate() {
        let Some(mut inline) = inline_spans(child) else {
            return false;
        };
        if index > 0 {
            spans.push(Span::raw("  ".to_string()));
        }
        spans.append(&mut inline);
    }
    lines.push(Line::from(spans));
    true
}

fn inline_spans(node: &LuaContentNode) -> Option<Vec<Span<'static>>> {
    match node {
        LuaContentNode::Text(text) => Some(vec![Span::styled(text.clone(), text_style())]),
        LuaContentNode::Code { .. } => None,
        LuaContentNode::Badge(text) => {
            Some(vec![Span::styled(format!("[{}]", text), badge_style())])
        }
        LuaContentNode::KeyValue { key, value } => Some(vec![
            Span::styled(format!("{key}: "), key_style()),
            Span::styled(value.clone(), text_style()),
        ]),
        LuaContentNode::Separator { .. }
        | LuaContentNode::Row { .. }
        | LuaContentNode::Column { .. }
        | LuaContentNode::Split { .. }
        | LuaContentNode::Table { .. }
        | LuaContentNode::Block { .. } => None,
    }
}

fn render_table(rows: &[Vec<String>], indent: usize, lines: &mut Vec<Line<'static>>) {
    if rows.is_empty() {
        return;
    }
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths = (0..column_count)
        .map(|index| {
            rows.iter()
                .filter_map(|row| row.get(index))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();

    for (row_index, row) in rows.iter().enumerate() {
        let mut spans = vec![Span::raw(" ".repeat(indent))];
        for (index, width) in widths.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw(" ".to_string()));
            }
            let cell = row.get(index).cloned().unwrap_or_default();
            let padding = width.saturating_sub(cell.chars().count());
            let style = table_cell_style(row_index, index);
            spans.push(Span::styled(" ".to_string(), style));
            spans.push(Span::styled(
                format!("{cell}{}", " ".repeat(padding)),
                style,
            ));
            spans.push(Span::styled(" ".to_string(), style));
        }
        lines.push(Line::from(spans));
    }
}

fn render_framed_lines(
    title: Option<&str>,
    body: Vec<Line<'static>>,
    indent: usize,
    width: usize,
    code_block: bool,
    lines: &mut Vec<Line<'static>>,
) {
    let inner_width = width.saturating_sub(indent + 4).max(1);
    lines.push(framed_top_line(title, indent, inner_width, code_block));
    for line in body {
        let mut spans = vec![
            Span::raw(" ".repeat(indent)),
            Span::styled(
                border::ROUNDED.vertical_left.to_string(),
                frame_border_style(code_block),
            ),
            Span::styled(" ".to_string(), frame_fill_style(code_block)),
        ];
        let line_width = line.width();
        spans.extend(line.spans);
        if line_width < inner_width {
            spans.push(Span::styled(
                " ".repeat(inner_width - line_width),
                frame_fill_style(code_block),
            ));
        }
        spans.push(Span::styled(" ".to_string(), frame_fill_style(code_block)));
        spans.push(Span::styled(
            border::ROUNDED.vertical_right.to_string(),
            frame_border_style(code_block),
        ));
        lines.push(Line::from(spans));
    }
    lines.push(framed_bottom_line(indent, inner_width, code_block));
}

fn framed_top_line(
    title: Option<&str>,
    indent: usize,
    inner_width: usize,
    code_block: bool,
) -> Line<'static> {
    let total_width = inner_width.saturating_add(2);
    let title = title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(|title| format!(" {title} "));
    let mut spans = vec![
        Span::raw(" ".repeat(indent)),
        Span::styled(
            border::ROUNDED.top_left.to_string(),
            frame_border_style(code_block),
        ),
    ];
    if let Some(title) = title {
        let title_width = title.chars().count().min(total_width);
        let left = total_width.saturating_sub(title_width) / 2;
        let right = total_width.saturating_sub(title_width + left);
        spans.push(Span::styled(
            border::ROUNDED.horizontal_top.repeat(left),
            frame_border_style(code_block),
        ));
        spans.push(Span::styled(title, frame_title_style(code_block)));
        spans.push(Span::styled(
            border::ROUNDED.horizontal_top.repeat(right),
            frame_border_style(code_block),
        ));
    } else {
        spans.push(Span::styled(
            border::ROUNDED.horizontal_top.repeat(total_width),
            frame_border_style(code_block),
        ));
    }
    spans.push(Span::styled(
        border::ROUNDED.top_right.to_string(),
        frame_border_style(code_block),
    ));
    Line::from(spans)
}

fn framed_bottom_line(indent: usize, inner_width: usize, code_block: bool) -> Line<'static> {
    Line::from(vec![
        Span::raw(" ".repeat(indent)),
        Span::styled(
            border::ROUNDED.bottom_left.to_string(),
            frame_border_style(code_block),
        ),
        Span::styled(
            border::ROUNDED
                .horizontal_bottom
                .repeat(inner_width.saturating_add(2)),
            frame_border_style(code_block),
        ),
        Span::styled(
            border::ROUNDED.bottom_right.to_string(),
            frame_border_style(code_block),
        ),
    ])
}

fn separator_line(label: Option<&str>, indent: usize, line_width: usize) -> Line<'static> {
    let mut spans = vec![Span::raw(" ".repeat(indent))];
    if let Some(label) = label {
        let text = format!(" {label} ");
        let label_width = text.chars().count().min(line_width);
        let left = line_width.saturating_sub(label_width) / 2;
        let right = line_width.saturating_sub(label_width + left);
        spans.push(Span::styled("─".repeat(left), separator_style()));
        spans.push(Span::styled(text, frame_title_style(false)));
        spans.push(Span::styled("─".repeat(right), separator_style()));
    } else {
        spans.push(Span::styled("─".repeat(line_width), separator_style()));
    }
    Line::from(spans)
}

fn padded_line_spans(
    line: Option<&Line<'static>>,
    width: usize,
    filler_style: Style,
) -> Vec<Span<'static>> {
    match line {
        Some(line) => {
            let mut spans = line.spans.clone();
            let line_width = line.width();
            if line_width < width {
                spans.push(Span::styled(" ".repeat(width - line_width), filler_style));
            }
            spans
        }
        None => vec![Span::styled(" ".repeat(width), filler_style)],
    }
}

fn table_cell_style(row_index: usize, column_index: usize) -> Style {
    let bg = if row_index == 0 {
        configure::themed_color(|colors| colors.surface.bg_val3)
    } else if (row_index + column_index) % 2 == 0 {
        configure::themed_color(|colors| colors.surface.bg_val2)
    } else {
        configure::themed_color(|colors| colors.surface.bg_val1)
    };
    Style::default()
        .fg(configure::themed_color(|colors| colors.text.primary))
        .bg(bg)
}

fn render_code_block(
    body: &str,
    kind: Option<&str>,
    indent: usize,
    width: usize,
    lines: &mut Vec<Line<'static>>,
) {
    let mut body_lines = kind
        .and_then(|kind| highlighted_lines(body, kind))
        .unwrap_or_else(|| {
            body.lines()
                .map(|line| Line::from(Span::styled(line.to_string(), code_style())))
                .collect()
        });
    if body_lines.is_empty() {
        body_lines.push(Line::from(Span::styled(String::new(), code_style())));
    }
    render_framed_lines(kind, body_lines, indent, width, true, lines);
}

fn render_separator(
    label: Option<&str>,
    empty: bool,
    height: usize,
    indent: usize,
    width: usize,
    lines: &mut Vec<Line<'static>>,
) {
    let line_width = width.saturating_sub(indent).max(1);
    for _ in 0..height {
        if empty {
            lines.push(Line::from(Span::styled(
                " ".repeat(indent + line_width),
                separator_fill_style(),
            )));
            continue;
        }
        let text = label.filter(|label| !label.trim().is_empty());
        lines.push(separator_line(text, indent, line_width));
    }
}

fn render_split(
    direction: LuaSplitDirection,
    ratio_millis: u16,
    gap: usize,
    left: &[LuaContentNode],
    right: &[LuaContentNode],
    indent: usize,
    width: usize,
    lines: &mut Vec<Line<'static>>,
) {
    match direction {
        LuaSplitDirection::Vertical => {
            for child in left {
                render_node(child, indent, width, lines);
            }
            for _ in 0..gap {
                lines.push(Line::from(Span::raw(" ".repeat(indent))));
            }
            for child in right {
                render_node(child, indent, width, lines);
            }
        }
        LuaSplitDirection::Horizontal => {
            let available = width.saturating_sub(indent);
            if available <= gap + 2 {
                for child in left {
                    render_node(child, indent, width, lines);
                }
                for child in right {
                    render_node(child, indent, width, lines);
                }
                return;
            }
            let content_width = available.saturating_sub(gap);
            let left_width = ((content_width as u32 * ratio_millis as u32) / 1000) as usize;
            let left_width = left_width.clamp(1, content_width.saturating_sub(1));
            let right_width = content_width.saturating_sub(left_width);
            let left_lines = render_ui_nodes(left, left_width);
            let right_lines = render_ui_nodes(right, right_width);
            let total_lines = left_lines.len().max(right_lines.len());
            for index in 0..total_lines {
                let mut spans = vec![Span::raw(" ".repeat(indent))];
                spans.extend(padded_line_spans(
                    left_lines.get(index),
                    left_width,
                    text_style(),
                ));
                spans.push(Span::raw(" ".repeat(gap)));
                spans.extend(padded_line_spans(
                    right_lines.get(index),
                    right_width,
                    text_style(),
                ));
                lines.push(Line::from(spans));
            }
        }
    }
}

fn append_multiline_text(text: &str, indent: usize, style: Style, lines: &mut Vec<Line<'static>>) {
    if text.is_empty() {
        lines.push(Line::from(Span::styled(" ".repeat(indent), style)));
        return;
    }
    for line in text.lines() {
        lines.push(Line::from(Span::styled(
            format!("{}{}", " ".repeat(indent), line),
            style,
        )));
    }
}

fn frame_title_style(code_block: bool) -> Style {
    if code_block {
        Style::default()
            .fg(configure::themed_color(|colors| colors.help.section))
            .bg(configure::themed_color(|colors| colors.surface.bg_val3))
            .bold()
    } else {
        Style::default()
            .fg(configure::themed_color(|colors| colors.content.app_brand))
            .bold()
    }
}

fn frame_border_style(code_block: bool) -> Style {
    let mut style = Style::default()
        .fg(configure::themed_color(|colors| colors.help.muted))
        .dim();
    if code_block {
        style = style.bg(configure::themed_color(|colors| colors.surface.bg_val3));
    }
    style
}

fn frame_fill_style(code_block: bool) -> Style {
    if code_block {
        Style::default().bg(configure::themed_color(|colors| colors.surface.bg_val3))
    } else {
        Style::default()
    }
}

fn key_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.help.section))
        .bold()
}

fn text_style() -> Style {
    Style::default().fg(configure::themed_color(|colors| colors.text.primary))
}

fn code_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.text.primary))
        .bg(configure::themed_color(|colors| colors.surface.bg_val3))
}

fn badge_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.accent.selection_fg))
        .bg(configure::themed_color(|colors| colors.accent.selection_bg))
        .bold()
}

fn separator_style() -> Style {
    Style::default().fg(configure::themed_color(|colors| {
        colors.surface.panel_border
    }))
}

fn separator_fill_style() -> Style {
    separator_style().bg(configure::themed_color(|colors| colors.surface.bg_val3))
}
