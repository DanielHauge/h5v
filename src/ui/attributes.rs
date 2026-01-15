use std::rc::Rc;

use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin, Offset, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Scrollbar, ScrollbarState},
    Frame,
};

use crate::{
    color_consts::{self, BG_COLOR, FOCUS_BG_COLOR},
    h5f::H5FNode,
};

use super::state::{AppState, AttributeViewSelection, Focus, Mode};

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

fn make_panels_scroll(area: Rect, scroll_size: u16) -> Rc<[Rect]> {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Max(u16::MAX), Constraint::Length(scroll_size)].as_ref())
        .split(area);
    chunks
}

fn render_text_overflow_handled(f: &mut Frame, area: &Rect, line: &Line) {
    let line_width = line.width();
    if line_width < (area.width as usize) {
        f.render_widget(line, *area);
    } else {
        let areas =
            Layout::horizontal([Constraint::Fill(u16::MAX), Constraint::Length(1)]).split(*area);
        f.render_widget(line, areas[0]);
        f.render_widget("_", areas[1]);
    }
}

pub fn render_info_attributes(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    state: &mut AppState,
) -> Result<(), hdf5_metno::Error> {
    let bg = match (&state.focus, &state.mode) {
        (Focus::Attributes, Mode::Normal) => FOCUS_BG_COLOR,
        _ => BG_COLOR,
    };

    let attr_header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title("Attributes".to_string())
        .bg(bg)
        .title_style(Style::default().fg(Color::Yellow).bold())
        .title_alignment(Alignment::Center);
    f.render_widget(attr_header_block, *area);

    let area_inner = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let attributes = node.read_attributes()?;
    let min_first_panel = match attributes.longest_name_length {
        0..5 => 5,
        5..=u16::MAX => attributes.longest_name_length,
    };
    let scroll_size = if area_inner.height as usize >= attributes.rendered_attributes.len() {
        0
    } else {
        3
    };
    let area = make_panels_rect(area_inner, min_first_panel);
    let [name_area, value_area] = area.as_ref() else {
        panic!("Could not get the areas for the info attribute panels");
    };

    let value_scrol_areas = make_panels_scroll(*value_area, scroll_size);
    let [value_area, scroll_area] = value_scrol_areas.as_ref() else {
        panic!("Could not get the areas for scroll panels.");
    };
    let height = name_area.height as i32;
    let heightu = height as usize;

    if scroll_area.height > 0 && scroll_area.width > 0 {
        let scrollbar = Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight)
            .end_symbol(Some("v"))
            .thumb_symbol("█")
            .begin_symbol(Some("^"));
        let mut scrollbar_state = ScrollbarState::new(attributes.rendered_attributes.len())
            .viewport_content_length(height as usize)
            .position(state.attributes_view_cursor.attribute_index);
        f.render_stateful_widget(scrollbar, *scroll_area, &mut scrollbar_state);
    }

    let mut offset = 0;

    let highlighted_index = &state
        .attributes_view_cursor
        .attribute_index
        .saturating_sub(state.attributes_view_cursor.attribute_offset)
        .clamp(0, heightu.saturating_sub(1));

    let highlighted_bg_color = if state.copying {
        color_consts::HIGHLIGHT_BG_COLOR_COPY
    } else {
        color_consts::HIGHLIGHT_BG_COLOR
    };

    let new_attr_offset = if state.attributes_view_cursor.attribute_index
        > heightu
            .saturating_sub(1)
            .saturating_add(state.attributes_view_cursor.attribute_offset)
    {
        state
            .attributes_view_cursor
            .attribute_index
            .saturating_sub(height.saturating_sub(1) as usize)
    } else if state.attributes_view_cursor.attribute_index
        <= state.attributes_view_cursor.attribute_offset
    {
        state.attributes_view_cursor.attribute_index
    } else {
        state.attributes_view_cursor.attribute_offset
    };
    state.attributes_view_cursor.attribute_offset = new_attr_offset;
    let mut attributes_to_skip = new_attr_offset;

    #[allow(clippy::explicit_counter_loop)]
    for (name_line, value_line) in &attributes.rendered_attributes {
        if attributes_to_skip != 0 {
            attributes_to_skip -= 1;
            continue;
        }
        if offset == *highlighted_index as i32 {
            match state.attributes_view_cursor.attribute_view_selection {
                AttributeViewSelection::Name => {
                    f.render_widget(
                        name_line.clone().bg(highlighted_bg_color),
                        name_area.offset(Offset { x: 0, y: offset }),
                    );

                    render_text_overflow_handled(
                        f,
                        &value_area.offset(Offset { x: 1, y: offset }),
                        value_line,
                    );
                }
                AttributeViewSelection::Value => {
                    f.render_widget(name_line, name_area.offset(Offset { x: 0, y: offset }));
                    render_text_overflow_handled(
                        f,
                        &value_area.offset(Offset { x: 1, y: offset }),
                        &value_line.clone().bg(highlighted_bg_color),
                    );
                }
            }
        } else {
            f.render_widget(name_line, name_area.offset(Offset { x: 0, y: offset }));
            render_text_overflow_handled(
                f,
                &value_area.offset(Offset { x: 1, y: offset }),
                value_line,
            );
        }

        if offset >= height - 1 {
            break;
        }
        offset += 1;
    }

    Ok(())
}
