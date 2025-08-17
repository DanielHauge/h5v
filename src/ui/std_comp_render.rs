use ratatui::{
    layout::Rect,
    style::Style,
    text::{Span, Text},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::{color_consts, h5f::Node};

pub fn render_string<T: ToString>(f: &mut Frame, area: &Rect, string: T) {
    let string = string.to_string();
    let string = string.lines().collect::<Vec<_>>().join("\n");
    let string = Span::styled(string, color_consts::COLOR_WHITE);
    let string = Text::from(string);
    let string = Paragraph::new(string).wrap(Wrap { trim: true });
    f.render_widget(string, *area);
}

pub fn render_error<T: ToString>(f: &mut Frame, area: &Rect, error: T) {
    f.render_widget(
        Paragraph::new(error.to_string()).style(Style::default().fg(color_consts::ERROR_COLOR)),
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
