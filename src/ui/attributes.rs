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

use super::state::{AppState, AttributeViewSelection, AttributesHitbox, Focus, Mode};

fn make_panels_rect(area: Rect, min_first_panel: u16) -> Rc<[Rect]> {
    Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Length(min_first_panel + 3),
            Constraint::Fill(u16::MAX),
        ])
        .split(area)
}

fn make_panels_scroll(area: Rect, scroll_size: u16) -> Rc<[Rect]> {
    Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Max(u16::MAX), Constraint::Length(scroll_size)])
        .split(area)
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
    let outer_area = *area;
    let bg = match (&state.focus, &state.mode) {
        (
            Focus::Attributes,
            Mode::Normal | Mode::FixedStringOverflowDialog | Mode::FixedStringResizeDialog,
        ) => FOCUS_BG_COLOR,
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
    f.render_widget(attr_header_block, outer_area);

    let area_inner = outer_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let node_attributes_view_cursor = node.attributes_view_cursor.clone();
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
        return Err(hdf5_metno::Error::Internal(
            "Could not get the areas for attribute panels.".to_string(),
        ));
    };

    let value_scrol_areas = make_panels_scroll(*value_area, scroll_size);
    let [value_area, scroll_area] = value_scrol_areas.as_ref() else {
        return Err(hdf5_metno::Error::Internal(
            "Could not get the areas for attribute panels.".to_string(),
        ));
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
            .position(node_attributes_view_cursor.attribute_index);
        f.render_stateful_widget(scrollbar, *scroll_area, &mut scrollbar_state);
    }

    let mut offset = 0;

    let highlighted_index = &node_attributes_view_cursor
        .attribute_index
        .saturating_sub(node_attributes_view_cursor.attribute_offset)
        .clamp(0, heightu.saturating_sub(1));

    let highlighted_bg_color = if let Focus::Attributes = state.focus {
        if state.copying {
            color_consts::HIGHLIGHT_BG_COLOR_COPY
        } else {
            color_consts::HIGHLIGHT_BG_COLOR
        }
    } else {
        color_consts::HIGHLIGHT_BG_COLOR
    };

    let new_attr_offset = if node_attributes_view_cursor.attribute_index
        > heightu
            .saturating_sub(1)
            .saturating_add(node_attributes_view_cursor.attribute_offset)
    {
        node_attributes_view_cursor
            .attribute_index
            .saturating_sub(height.saturating_sub(1) as usize)
    } else if node_attributes_view_cursor.attribute_index
        <= node_attributes_view_cursor.attribute_offset
    {
        node_attributes_view_cursor.attribute_index
    } else {
        node_attributes_view_cursor.attribute_offset
    };
    state.ui_layout.attributes = Some(AttributesHitbox {
        outer: outer_area,
        inner: area_inner,
        name_area: *name_area,
        value_area: *value_area,
        row_offset: new_attr_offset,
        visible_rows: heightu.min(
            attributes
                .rendered_attributes
                .len()
                .saturating_sub(new_attr_offset),
        ),
        total_rows: attributes.rendered_attributes.len(),
    });
    let mut attributes_to_skip = new_attr_offset;

    #[allow(clippy::explicit_counter_loop)]
    for (name_line, value_line, type_line) in &attributes.rendered_attributes {
        if attributes_to_skip != 0 {
            attributes_to_skip -= 1;
            continue;
        }
        if offset == *highlighted_index as i32 {
            match node_attributes_view_cursor.attribute_view_selection {
                AttributeViewSelection::Name => {
                    f.render_widget(
                        name_line.clone().bg(highlighted_bg_color),
                        name_area.offset(Offset { x: 0, y: offset }),
                    );

                    render_text_overflow_handled(
                        f,
                        &value_area.offset(Offset { x: 1, y: offset }),
                        &value_line.clone(),
                    );
                    render_text_overflow_handled(
                        f,
                        &value_area.offset(Offset {
                            x: 1 + value_line.width() as i32,
                            y: offset,
                        }),
                        &type_line.clone(),
                    );
                }
                AttributeViewSelection::Value => {
                    f.render_widget(name_line, name_area.offset(Offset { x: 0, y: offset }));
                    render_text_overflow_handled(
                        f,
                        &value_area.offset(Offset { x: 1, y: offset }),
                        &value_line.clone().bg(highlighted_bg_color),
                    );
                    render_text_overflow_handled(
                        f,
                        &value_area.offset(Offset {
                            x: 1 + value_line.width() as i32,
                            y: offset,
                        }),
                        &type_line.clone().bg(highlighted_bg_color),
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
            render_text_overflow_handled(
                f,
                &value_area.offset(Offset {
                    x: 1 + value_line.width() as i32,
                    y: offset,
                }),
                type_line,
            );
        }

        if offset >= height - 1 {
            break;
        }
        offset += 1;
    }

    node.attributes_view_cursor.attribute_offset = new_attr_offset;

    Ok(())
}
