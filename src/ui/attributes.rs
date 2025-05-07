use std::{cell::RefCell, rc::Rc};

use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin, Offset, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders},
    Frame,
};

use crate::{
    color_consts::{self, BG_COLOR, FOCUS_BG_COLOR},
    h5f::H5FNode,
};

use super::state::{AppState, AttributeViewSelection, Focus};

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
    state: &mut AppState,
) -> Result<(), hdf5_metno::Error> {
    let bg = match state.focus {
        Focus::Attributes => FOCUS_BG_COLOR,
        _ => BG_COLOR,
    };

    let attr_header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title("Display".to_string())
        .bg(bg)
        .title_style(Style::default().fg(Color::Yellow).bold())
        .title_alignment(Alignment::Center);
    f.render_widget(attr_header_block, *area);

    let area_inner = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let mut node = selected_node.borrow_mut();
    let attributes = node.read_attributes()?;
    let min_first_panel = match attributes.longest_name_length {
        0..5 => 5,
        5..=u16::MAX => attributes.longest_name_length,
    };
    let area = make_panels_rect(area_inner, min_first_panel);
    let [name_area, value_area] = area.as_ref() else {
        panic!("Could not get the areas for the info attribute panels");
    };
    let mut offset = 0;
    let height = name_area.height as i32;

    let highlighted_index = &state
        .attributes_view_cursor
        .attribute_index
        .clamp(0, (height - 1) as usize);

    #[allow(clippy::explicit_counter_loop)]
    for (name_line, value_line) in &attributes.rendered_attributes {
        if offset == *highlighted_index as i32 {
            match state.attributes_view_cursor.attribute_view_selection {
                AttributeViewSelection::Name => {
                    f.render_widget(
                        name_line.clone().bg(color_consts::HIGHLIGHT_BG_COLOR),
                        name_area.offset(Offset { x: 0, y: offset }),
                    );
                    f.render_widget(value_line, value_area.offset(Offset { x: 1, y: offset }));
                }
                AttributeViewSelection::NameAndValue => {
                    f.render_widget(
                        name_line.clone().bg(color_consts::HIGHLIGHT_BG_COLOR),
                        name_area.offset(Offset { x: 0, y: offset }),
                    );
                    f.render_widget(
                        value_line.clone().bg(color_consts::HIGHLIGHT_BG_COLOR),
                        value_area.offset(Offset { x: 1, y: offset }),
                    );
                }
                AttributeViewSelection::Value => {
                    f.render_widget(name_line, name_area.offset(Offset { x: 0, y: offset }));
                    f.render_widget(
                        value_line.clone().bg(color_consts::HIGHLIGHT_BG_COLOR),
                        value_area.offset(Offset { x: 1, y: offset }),
                    );
                }
            }
        } else {
            f.render_widget(name_line, name_area.offset(Offset { x: 0, y: offset }));
            f.render_widget(value_line, value_area.offset(Offset { x: 1, y: offset }));
        }

        if offset >= height - 1 {
            break;
        }
        offset += 1;
    }

    Ok(())
}
