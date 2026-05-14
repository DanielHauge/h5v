use std::sync::LazyLock;

use itertools::Itertools;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};

use crate::{
    configure,
    h5f::{H5FNode, Node},
};

pub fn render_string<T: ToString>(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    string: T,
    hl: Option<String>,
) {
    match hl {
        Some(hl) => render_hl_string(f, area, node, string, hl),
        None => render_raw_string(f, area, node, string),
    }
}

fn split_display_lines(string: &str) -> Vec<&str> {
    string.split('\n').collect()
}

fn clamp_line_offset(node: &mut H5FNode, total_lines: usize, viewport_height: usize) -> usize {
    let max_offset = total_lines.saturating_sub(viewport_height.max(1));
    let clamped = node.line_offset.min(max_offset);
    node.line_offset = clamped;
    clamped
}

fn syntect_to_ratatui_style(style: syntect::highlighting::Style) -> ratatui::style::Style {
    // let bg = style.background;
    let fg = style.foreground;
    ratatui::style::Style::default()
        .fg(ratatui::style::Color::Rgb(fg.r, fg.g, fg.b))
        // .bg(ratatui::style::Color::Rgb(bg.r, bg.g, bg.b))
        .add_modifier(
            if style
                .font_style
                .contains(syntect::highlighting::FontStyle::BOLD)
            {
                ratatui::style::Modifier::BOLD
            } else {
                ratatui::style::Modifier::empty()
            },
        )
        .add_modifier(
            if style
                .font_style
                .contains(syntect::highlighting::FontStyle::UNDERLINE)
            {
                ratatui::style::Modifier::UNDERLINED
            } else {
                ratatui::style::Modifier::empty()
            },
        )
        .add_modifier(
            if style
                .font_style
                .contains(syntect::highlighting::FontStyle::ITALIC)
            {
                ratatui::style::Modifier::ITALIC
            } else {
                ratatui::style::Modifier::empty()
            },
        )
}

fn primary_text_style() -> Style {
    let mut style = Style::default().fg(configure::themed_color(|colors| colors.text.primary));
    if configure::prefers_strong_text() {
        style = style.bold();
    }
    style
}

fn highlight_theme_name() -> &'static str {
    if configure::prefers_strong_text() {
        "base16-ocean.light"
    } else {
        "base16-ocean.dark"
    }
}

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

pub fn highlighted_lines(string: &str, hl: &str) -> Option<Vec<Line<'static>>> {
    let syntax = SYNTAX_SET.find_syntax_by_extension(hl)?;
    let mut h = HighlightLines::new(syntax, &THEME_SET.themes[highlight_theme_name()]);
    let string = if hl == "json" {
        serde_json::from_str::<serde_json::Value>(string)
            .ok()
            .and_then(|value| serde_json::to_string_pretty(&value).ok())?
    } else {
        string.to_string()
    };

    let mut highlighted = Vec::new();
    for line in LinesWithEndings::from(&string) {
        let ranges = h
            .highlight_line(line, &SYNTAX_SET)
            .unwrap_or_else(|_| vec![(syntect::highlighting::Style::default(), line)]);
        let spans = ranges
            .into_iter()
            .map(|(style, text)| Span::styled(text.to_string(), syntect_to_ratatui_style(style)))
            .collect::<Vec<_>>();
        highlighted.push(Line::from(spans));
    }
    Some(highlighted)
}

pub fn render_hl_string<T: ToString>(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    string: T,
    hl: String,
) {
    let syntax = match SYNTAX_SET.find_syntax_by_extension(&hl) {
        Some(s) => s,
        None => return render_raw_string(f, area, node, string),
    };
    let mut h = HighlightLines::new(syntax, &THEME_SET.themes[highlight_theme_name()]);
    let string = string.to_string();
    let string = if hl == "json" {
        match serde_json::from_str::<serde_json::Value>(&string) {
            Ok(v) => match serde_json::to_string_pretty(&v) {
                Ok(pretty) => pretty,
                Err(e) => {
                    return render_error(
                        f,
                        area,
                        format!("Error pretty-printing JSON: {e}\n{string}"),
                    )
                }
            },
            Err(e) => return render_error(f, area, format!("Error parsing JSON: {e}\n{string}")),
        }
    } else {
        string
    };
    let total_lines = split_display_lines(&string).len().max(1);
    let line_offset = clamp_line_offset(node, total_lines, area.height as usize);
    let mut escaped_lines = Vec::new();

    let mut skips = line_offset;
    for line in LinesWithEndings::from(&string) {
        let ranges: Vec<(syntect::highlighting::Style, &str)> = h
            .highlight_line(line, &SYNTAX_SET)
            .unwrap_or_else(|_| vec![(syntect::highlighting::Style::default(), line)]);
        let mut spans = vec![];
        for (style, text) in ranges {
            let style = syntect_to_ratatui_style(style);
            let mut span = Span::raw(text);
            span.style = style;
            spans.push(span);
        }
        if skips > 0 {
            skips -= 1;
        } else {
            escaped_lines.push(Line::from(spans));
            if escaped_lines.len() >= area.height as usize {
                break;
            }
        }
    }
    let visible_line_count = escaped_lines.len().max(1);
    let line_num = (line_offset + visible_line_count).to_string().len() as u16;
    let (line_num_area, text_area) = split_string_linenumber(*area, line_num);
    render_linenums(f, &line_num_area, line_offset, visible_line_count);
    let string = Text::from(escaped_lines);
    f.render_widget(
        Paragraph::new(string)
            .style(primary_text_style())
            .wrap(Wrap { trim: false }),
        text_area,
    );
}

fn split_string_linenumber(area: Rect, max: u16) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Length(max), Constraint::Min(0)])
        .spacing(1)
        .split(area);
    (chunks[0], chunks[1])
}

fn render_linenums(f: &mut Frame, area: &Rect, line_offset: usize, visible_lines: usize) {
    let first_line_num = line_offset + 1;
    let line_nums: Vec<String> = (first_line_num..first_line_num + visible_lines)
        .map(|n| n.to_string())
        .collect();
    let lines = Text::from(line_nums.join("\n"));
    f.render_widget(
        Paragraph::new(lines)
            .style(
                ratatui::style::Style::default()
                    .fg(configure::themed_color(|colors| colors.text.line_num)),
            )
            .alignment(Alignment::Right)
            .wrap(Wrap { trim: false }),
        *area,
    );
}

fn render_raw_string<T: ToString>(f: &mut Frame, area: &Rect, node: &mut H5FNode, string: T) {
    let string = string.to_string();
    let lines = split_display_lines(&string);
    let line_offset = clamp_line_offset(node, lines.len().max(1), area.height as usize);
    let visible_lines = lines
        .iter()
        .skip(line_offset)
        .take(area.height as usize)
        .map(|line| {
            if line.len() > node.col_offset as usize {
                line[node.col_offset as usize..].to_string()
            } else {
                "".to_string()
            }
        })
        .map(Line::from)
        .collect_vec();
    let visible_line_count = visible_lines.len().max(1);
    let line_num = (line_offset + visible_line_count).to_string().len() as u16;
    let (line_num_area, text_area) = split_string_linenumber(*area, line_num);
    render_linenums(f, &line_num_area, line_offset, visible_line_count);
    let string = Text::from(visible_lines);

    f.render_widget(
        Paragraph::new(string)
            .style(primary_text_style())
            .wrap(Wrap { trim: false }),
        text_area,
    );
}

pub fn render_error<T: ToString>(f: &mut Frame, area: &Rect, error: T) {
    f.render_widget(
        Paragraph::new(error.to_string()).style(
            ratatui::style::Style::default()
                .fg(configure::themed_color(|colors| colors.text.error)),
        ),
        *area,
    );
}

pub fn render_empty_dataset(f: &mut Frame, area: &Rect) {
    let text = Text::from(vec![
        Line::from("This dataset is gloriously empty."),
        Line::from(""),
        Line::from("No rows. No values. Just pure potential."),
        Line::from(""),
        Line::from("   (a tiny void is vibing here)"),
    ]);
    let paragraph = Paragraph::new(text)
        .style(primary_text_style())
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(
                    ratatui::style::Style::default()
                        .fg(configure::themed_color(|colors| colors.surface.break_line)),
                )
                .title(configure::configured_symbol(|symbols| {
                    symbols.title.empty_dataset
                }))
                .title_alignment(Alignment::Center),
        );
    f.render_widget(paragraph, *area);
}

pub fn render_unsupported_rendering(f: &mut Frame, area: &Rect, selected_node: &Node, desc: &str) {
    let (ds, _) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => return,
    };

    let inner_area = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let unsupported_msg = format!("Unsupported preview for dataset: {}", ds.name());
    f.render_widget(
        Paragraph::new(unsupported_msg).style(primary_text_style()),
        inner_area,
    );
    let why = format!("Reason: {}", desc);
    f.render_widget(
        Paragraph::new(why).style(primary_text_style()),
        inner_area.inner(ratatui::layout::Margin {
            horizontal: 2,
            vertical: 1,
        }),
    );
}
