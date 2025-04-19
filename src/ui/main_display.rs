use std::{cell::RefCell, rc::Rc};

use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin, Offset, Rect},
    style::{Color, Style, Stylize},
    text::Span,
    widgets::{block::Title, Block, Borders},
    Frame,
};

use crate::{color_consts, h5f::H5FNode};

use super::{
    app::{AppState, ContentShowMode},
    attributes::render_info_attributes,
    preview::render_preview,
};

fn split_main_display(area: Rect, attributes_count: usize) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(
            [
                Constraint::Length(attributes_count as u16 + 2),
                Constraint::Min(0),
            ]
            .as_ref(),
        )
        .split(area);
    (chunks[0], chunks[1])
}
pub fn render_main_display(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Rc<RefCell<H5FNode>>,
    state: &mut AppState,
) -> std::result::Result<(), hdf5_metno::Error> {
    let attr_count = selected_node
        .borrow_mut()
        .read_attributes()?
        .rendered_attributes
        .len();

    let (attr_area, content_area) = split_main_display(*area, attr_count);
    let attr_header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(format!("Attributes"))
        .bg(color_consts::BG2_COLOR)
        .title_style(Style::default().fg(Color::Yellow).bold())
        .title_alignment(Alignment::Center);
    f.render_widget(attr_header_block, *area);
    render_info_attributes(f, &attr_area, selected_node)?;

    let current_display_mode = &state.content_mode;
    let supported_display_modes = selected_node.borrow().content_show_modes();
    if supported_display_modes.is_empty() {
        return Ok(());
    }
    let is_supported = supported_display_modes.contains(current_display_mode);
    let supported_modes_count = supported_display_modes.len();
    let display_mode = if is_supported {
        current_display_mode
    } else {
        &supported_display_modes[0]
    };
    let display_index = supported_display_modes
        .iter()
        .position(|x| x == display_mode)
        .expect("Display mode expected to be found in list otherwise not reach this point");

    // Do tab titles:

    let mut tab_titles = vec![];
    for (i, x) in supported_display_modes.iter().enumerate() {
        let title = match x {
            ContentShowMode::Preview => "Preview 🗠",
            ContentShowMode::Matrix => "Matrix",
            ContentShowMode::Heatmap => "Heatmap",
        };

        if i == display_index {
            tab_titles.push(Span::styled(title, color_consts::TITLE).bold().underlined());
        } else {
            tab_titles.push(Span::styled(title, color_consts::TITLE));
        }
        if i != supported_modes_count - 1 {
            tab_titles.push(Span::styled(" | ", crate::ui::main_display::Color::Green));
        }
    }

    let title = Title::from(tab_titles);
    let break_line = Block::default()
        .title(title)
        .borders(ratatui::widgets::Borders::TOP)
        .border_style(Style::default().fg(color_consts::BREAK_COLOR))
        .title_alignment(Alignment::Center)
        .title_style(Style::default().fg(color_consts::TITLE))
        .style(Style::default().bg(color_consts::BG2_COLOR));
    f.render_widget(
        break_line,
        content_area.offset(Offset { x: 0, y: -1 }).inner(Margin {
            vertical: 0,
            horizontal: 2,
        }),
    );

    render_preview(f, &content_area, selected_node, state)?;

    Ok(())
}
