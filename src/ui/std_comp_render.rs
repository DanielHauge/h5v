use itertools::Itertools;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
    Frame,
};
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};

use crate::{
    color_consts,
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

pub fn render_hl_string<T: ToString>(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    string: T,
    hl: String,
) {
    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let syntax = match ps.find_syntax_by_extension(&hl) {
        Some(s) => s,
        None => return render_raw_string(f, area, node, string),
    };
    let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
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
    let mut escaped_lines = Vec::new();

    let mut skips = node.line_offset;
    for line in LinesWithEndings::from(&string) {
        let ranges: Vec<(syntect::highlighting::Style, &str)> =
            h.highlight_line(line, &ps).unwrap();
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
        }
    }
    let line_num = (node.line_offset + area.height as usize).to_string().len() as u16;
    let (line_num_area, text_area) = split_string_linenumber(*area, line_num);
    render_linenums(f, &line_num_area, node);
    let string = Text::from(escaped_lines);
    f.render_widget(string, text_area);
}

fn split_string_linenumber(area: Rect, max: u16) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Length(max), Constraint::Min(0)].as_ref())
        .spacing(1)
        .split(area);
    (chunks[0], chunks[1])
}

fn render_linenums(f: &mut Frame, area: &Rect, node: &mut H5FNode) {
    let first_line_num = node.line_offset + 1;
    let line_nums: Vec<String> = (first_line_num..first_line_num + area.height as usize)
        .map(|n| n.to_string())
        .collect();
    let lines = Text::from(line_nums.join("\n"));
    f.render_widget(
        Paragraph::new(lines)
            .style(ratatui::style::Style::default().fg(color_consts::LINE_NUM_COLOR))
            .alignment(Alignment::Right)
            .wrap(Wrap { trim: false }),
        *area,
    );
}

fn render_raw_string<T: ToString>(f: &mut Frame, area: &Rect, node: &mut H5FNode, string: T) {
    let line_num = (node.line_offset + area.height as usize).to_string().len() as u16;
    let (line_num_area, text_area) = split_string_linenumber(*area, line_num);
    render_linenums(f, &line_num_area, node);
    let string = string.to_string().lines().skip(node.line_offset).join("");
    let string = Text::from(string);
    f.render_widget(string, text_area);
}

pub fn render_error<T: ToString>(f: &mut Frame, area: &Rect, error: T) {
    f.render_widget(
        Paragraph::new(error.to_string())
            .style(ratatui::style::Style::default().fg(color_consts::ERROR_COLOR)),
        *area,
    );
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
    f.render_widget(unsupported_msg, inner_area);
    let why = format!("Reason: {}", desc);
    f.render_widget(
        why,
        inner_area.inner(ratatui::layout::Margin {
            horizontal: 2,
            vertical: 1,
        }),
    );
}
