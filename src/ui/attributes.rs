use std::{cell::RefCell, rc::Rc};

use ratatui::{
    layout::{Alignment, Margin, Offset, Rect},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    color_consts,
    h5f::{H5FNode, HasAttributes},
    sprint_attributes::sprint_attribute,
};

pub fn render_info_attributes(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Rc<RefCell<H5FNode>>,
) -> Result<(), hdf5_metno::Error> {
    let mut area = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let mut node = selected_node.borrow_mut();
    let attributes = node.read_attributes()?;
    for a in &attributes.attributes {
        let name = a.0.to_string();
        let value = sprint_attribute(&a.1)?;
        let attr_text = format!("{}: {}", name, value);
        let text = Text::from(attr_text);
        f.render_widget(text, area);
        area = area.offset(Offset { x: 0, y: 1 });
    }

    Ok(())
}
