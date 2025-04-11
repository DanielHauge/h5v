use std::{cell::RefCell, rc::Rc, u16};

use ratatui::{
    layout::{Constraint, Layout, Margin, Offset, Rect},
    Frame,
};

use crate::h5f::H5FNode;

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
    let min_first_panel = match attributes.longest_name_length {
        0..5 => 5,
        5..=u16::MAX => attributes.longest_name_length,
    };
    let area = make_panels_rect(area_inner, min_first_panel);
    let [name_area, value_area] = area.as_ref() else {
        panic!("Could not get the areas for the panels");
    };
    let mut offset = 0;
    let height = name_area.height as i32;

    for (name_line, value_line) in &attributes.rendered_attributes {
        f.render_widget(name_line, name_area.offset(Offset { x: 0, y: offset }));
        f.render_widget(value_line, value_area.offset(Offset { x: 1, y: offset }));

        if offset >= height - 1 {
            break;
        }
        offset += 1;
    }

    Ok(())
}
