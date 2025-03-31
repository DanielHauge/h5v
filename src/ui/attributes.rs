use std::{cell::RefCell, rc::Rc, u16};

use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin, Offset, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    color_consts,
    h5f::{H5FNode, HasAttributes},
    sprint_attributes::sprint_attribute,
};

fn make_panels_rect(area: Rect, min_first_panel: u16) -> Rc<[Rect]> {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints(
            [
                Constraint::Length(min_first_panel + 3),
                Constraint::Fill(u16::MAX),
            ]
            .as_ref(),
        )
        .split(area);
    chunks
}

pub fn render_info_attributes(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Rc<RefCell<H5FNode>>,
) -> Result<(), hdf5_metno::Error> {
    let area_inner = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let mut node = selected_node.borrow_mut();
    let attributes = node.read_attributes()?;
    let area = make_panels_rect(area_inner, attributes.longest_name_length);
    let [name_area, value_area] = area.as_ref() else {
        panic!("Could not get the areas for the panels");
    };
    let mut offset = 0;
    for a in &attributes.attributes {
        let name = a.0.to_string();
        let name_len = name.len();
        let name_styled = Span::styled(
            name,
            Style::default().fg(color_consts::VARIABLE_BLUE).bold(),
        );
        let extra_name_space = name_area.width as usize - name_len;
        let name_helper_line = Span::styled(
            "─".repeat(extra_name_space - 1),
            Style::default().fg(color_consts::LINES_COLOR),
        );
        let equals_sign = Span::styled("=", Style::default().fg(color_consts::EQUAL_SIGN_COLOR));
        let name_paragraph = Paragraph::new(name_styled + name_helper_line + equals_sign)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });
        f.render_widget(name_paragraph, name_area.offset(Offset { x: 0, y: offset }));

        let value_line = match sprint_attribute(&a.1) {
            Ok(l) => l,
            Err(e) => Line::styled(
                format!("Error: {}", e),
                Style::default().fg(color_consts::ERROR_COLOR),
            ),
        };
        f.render_widget(value_line, value_area.offset(Offset { x: 1, y: offset }));
        offset += 1;
    }

    Ok(())
}
