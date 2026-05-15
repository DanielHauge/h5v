use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Scrollbar, ScrollbarState},
    Frame,
};

use crate::{
    configure,
    h5f::{ComputedAttributes, H5FNode, MetadataRowKind},
};

use super::state::{
    AppState, AttributeViewSelection, AttributesHitbox, Focus, MetadataCellHitbox, Mode,
};

const PROPERTY_GRID_GAP: u16 = 3;
const PROPERTY_GRID_MIN_VALUE_WIDTH: u16 = 16;

#[derive(Debug, Clone)]
enum MetadataDisplayRow {
    SectionHeader(String),
    Cells(Vec<usize>),
}

pub struct PreparedMetadataLayout {
    initial_display_rows: Vec<MetadataDisplayRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MetadataCellPosition {
    display_row: usize,
    cell: usize,
}

fn split_main_scroll(area: Rect, scroll_size: u16) -> [Rect; 2] {
    let split = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Length(scroll_size)])
        .split(area);
    [split[0], split[1]]
}

fn split_name_value(area: Rect, min_first_panel: u16) -> [Rect; 2] {
    let split = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Length(min_first_panel + 3), Constraint::Fill(1)])
        .split(area);
    [split[0], split[1]]
}

fn should_use_property_grid(rows_area: Rect, min_first_panel: u16) -> bool {
    let min_cell_width = min_first_panel
        .saturating_add(3)
        .saturating_add(PROPERTY_GRID_MIN_VALUE_WIDTH);
    rows_area.width
        >= min_cell_width
            .saturating_mul(2)
            .saturating_add(PROPERTY_GRID_GAP)
}

fn build_display_rows(
    attributes: &ComputedAttributes,
    use_property_grid: bool,
) -> Vec<MetadataDisplayRow> {
    let mut display_rows = Vec::new();
    let mut property_buffer = Vec::new();

    let flush_properties = |display_rows: &mut Vec<MetadataDisplayRow>,
                            property_buffer: &mut Vec<usize>| {
        if use_property_grid {
            for chunk in property_buffer.chunks(2) {
                display_rows.push(MetadataDisplayRow::Cells(chunk.to_vec()));
            }
        } else {
            for row_index in property_buffer.drain(..) {
                display_rows.push(MetadataDisplayRow::Cells(vec![row_index]));
            }
            return;
        }
        property_buffer.clear();
    };

    for (row_index, row) in attributes.rendered_rows.iter().enumerate() {
        match row.kind {
            MetadataRowKind::SectionHeader => {
                flush_properties(&mut display_rows, &mut property_buffer);
                display_rows.push(MetadataDisplayRow::SectionHeader(row.name_line.to_string()));
            }
            MetadataRowKind::Property => property_buffer.push(row_index),
            MetadataRowKind::Attribute => {
                flush_properties(&mut display_rows, &mut property_buffer);
                display_rows.push(MetadataDisplayRow::Cells(vec![row_index]));
            }
        }
    }

    flush_properties(&mut display_rows, &mut property_buffer);
    display_rows
}

fn display_row_index(display_rows: &[MetadataDisplayRow], selected_row_index: usize) -> usize {
    display_rows
        .iter()
        .position(|row| match row {
            MetadataDisplayRow::SectionHeader(_) => false,
            MetadataDisplayRow::Cells(cells) => cells.contains(&selected_row_index),
        })
        .unwrap_or(0)
}

fn find_cell_position(
    display_rows: &[MetadataDisplayRow],
    selected_row_index: usize,
) -> Option<MetadataCellPosition> {
    display_rows
        .iter()
        .enumerate()
        .find_map(|(display_row, row)| match row {
            MetadataDisplayRow::SectionHeader(_) => None,
            MetadataDisplayRow::Cells(cells) => cells
                .iter()
                .position(|idx| *idx == selected_row_index)
                .map(|cell| MetadataCellPosition { display_row, cell }),
        })
}

fn build_rows_for_width(
    attributes: &ComputedAttributes,
    outer_width: u16,
) -> Vec<MetadataDisplayRow> {
    let inner_width = outer_width.saturating_sub(4);
    let rows_area = Rect {
        x: 0,
        y: 0,
        width: inner_width,
        height: 1,
    };
    let use_property_grid = should_use_property_grid(rows_area, attributes.longest_name_length);
    build_display_rows(attributes, use_property_grid)
}

pub(crate) fn navigate_metadata_grid(
    attributes: &ComputedAttributes,
    outer_width: u16,
    current_row_index: usize,
    selection: AttributeViewSelection,
    direction: crate::ui::input::keymap::Direction,
) -> Option<(usize, AttributeViewSelection)> {
    use crate::ui::input::keymap::Direction;

    let display_rows = build_rows_for_width(attributes, outer_width);
    let current = find_cell_position(&display_rows, current_row_index)?;

    match direction {
        Direction::Up | Direction::Down => {
            let delta = if matches!(direction, Direction::Up) {
                -1
            } else {
                1
            };
            let mut display_row = current.display_row as isize + delta;
            while display_row >= 0 && display_row < display_rows.len() as isize {
                if let MetadataDisplayRow::Cells(cells) = &display_rows[display_row as usize] {
                    let cell = current.cell.min(cells.len().saturating_sub(1));
                    return Some((cells[cell], selection));
                }
                display_row += delta;
            }
            None
        }
        Direction::Left => match selection {
            AttributeViewSelection::Value => {
                Some((current_row_index, AttributeViewSelection::Name))
            }
            AttributeViewSelection::Name => {
                if let MetadataDisplayRow::Cells(cells) = &display_rows[current.display_row] {
                    if current.cell > 0 {
                        Some((cells[current.cell - 1], AttributeViewSelection::Value))
                    } else {
                        let mut display_row = current.display_row as isize - 1;
                        while display_row >= 0 {
                            if let MetadataDisplayRow::Cells(prev_cells) =
                                &display_rows[display_row as usize]
                            {
                                return Some((*prev_cells.last()?, AttributeViewSelection::Value));
                            }
                            display_row -= 1;
                        }
                        None
                    }
                } else {
                    None
                }
            }
        },
        Direction::Right => match selection {
            AttributeViewSelection::Name => {
                Some((current_row_index, AttributeViewSelection::Value))
            }
            AttributeViewSelection::Value => {
                if let MetadataDisplayRow::Cells(cells) = &display_rows[current.display_row] {
                    if current.cell + 1 < cells.len() {
                        Some((cells[current.cell + 1], AttributeViewSelection::Name))
                    } else {
                        let mut display_row = current.display_row + 1;
                        while display_row < display_rows.len() {
                            if let MetadataDisplayRow::Cells(next_cells) =
                                &display_rows[display_row]
                            {
                                return Some((next_cells[0], AttributeViewSelection::Name));
                            }
                            display_row += 1;
                        }
                        None
                    }
                } else {
                    None
                }
            }
        },
    }
}

fn render_text_overflow_handled(f: &mut Frame, area: Rect, line: &Line) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let line_width = line.width();
    if line_width <= area.width as usize {
        f.render_widget(line, area);
    } else {
        let areas = Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]).split(area);
        f.render_widget(line, areas[0]);
        f.render_widget(
            Span::styled(
                "_",
                Style::default().fg(configure::themed_color(|colors| colors.text.primary)),
            ),
            areas[1],
        );
    }
}

fn render_section_header(f: &mut Frame, area: Rect, title: &str) {
    let title = format!(" {} ", title.trim());
    let total_width = area.width as usize;
    let title_width = title.chars().count().min(total_width);
    let line_width = total_width.saturating_sub(title_width);
    let left_width = line_width / 2;
    let right_width = line_width.saturating_sub(left_width);
    let rendered = Line::from(vec![
        Span::styled(
            configure::configured_symbol(|symbols| symbols.tree.horizontal_rule).repeat(left_width),
            Style::default().fg(configure::themed_color(|colors| colors.tree.lines)),
        ),
        Span::styled(
            title.chars().take(total_width).collect::<String>(),
            Style::default()
                .fg(configure::themed_color(|colors| colors.metadata.section))
                .bold(),
        ),
        Span::styled(
            configure::configured_symbol(|symbols| symbols.tree.horizontal_rule)
                .repeat(right_width),
            Style::default().fg(configure::themed_color(|colors| colors.tree.lines)),
        ),
    ]);
    render_text_overflow_handled(f, area, &rendered);
}

fn render_metadata_cell(
    f: &mut Frame,
    row: &crate::h5f::RenderedAttributeRow,
    name_area: Rect,
    value_area: Rect,
    selection: AttributeViewSelection,
    is_selected: bool,
    highlighted_bg_color: Color,
) {
    let value_text_area = Rect {
        x: value_area.x.saturating_add(1),
        y: value_area.y,
        width: value_area.width.saturating_sub(1),
        height: value_area.height,
    };

    if is_selected && matches!(selection, AttributeViewSelection::Name) {
        f.render_widget(row.name_line.clone().bg(highlighted_bg_color), name_area);
        render_text_overflow_handled(f, value_text_area, &row.value_line);
        render_text_overflow_handled(
            f,
            Rect {
                x: value_text_area
                    .x
                    .saturating_add(row.value_line.width() as u16),
                y: value_text_area.y,
                width: value_text_area
                    .width
                    .saturating_sub(row.value_line.width() as u16),
                height: value_text_area.height,
            },
            &row.type_line,
        );
        return;
    }

    f.render_widget(row.name_line.clone(), name_area);
    if is_selected && matches!(selection, AttributeViewSelection::Value) {
        render_text_overflow_handled(
            f,
            value_text_area,
            &row.value_line.clone().bg(highlighted_bg_color),
        );
        render_text_overflow_handled(
            f,
            Rect {
                x: value_text_area
                    .x
                    .saturating_add(row.value_line.width() as u16),
                y: value_text_area.y,
                width: value_text_area
                    .width
                    .saturating_sub(row.value_line.width() as u16),
                height: value_text_area.height,
            },
            &row.type_line.clone().bg(highlighted_bg_color),
        );
    } else {
        render_text_overflow_handled(f, value_text_area, &row.value_line);
        render_text_overflow_handled(
            f,
            Rect {
                x: value_text_area
                    .x
                    .saturating_add(row.value_line.width() as u16),
                y: value_text_area.y,
                width: value_text_area
                    .width
                    .saturating_sub(row.value_line.width() as u16),
                height: value_text_area.height,
            },
            &row.type_line,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_property_grid_row(
    f: &mut Frame,
    rows_area: Rect,
    min_first_panel: u16,
    row_indices: &[usize],
    attributes: &ComputedAttributes,
    selected_row_index: usize,
    selection: AttributeViewSelection,
    highlighted_bg_color: Color,
    hitboxes: &mut Vec<MetadataCellHitbox>,
) {
    let split = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(PROPERTY_GRID_GAP),
            Constraint::Fill(1),
        ])
        .split(rows_area);
    let cell_areas = [split[0], split[2]];

    let separator = Line::styled(
        " | ",
        Style::default().fg(configure::themed_color(|colors| colors.tree.lines)),
    );
    render_text_overflow_handled(f, split[1], &separator);

    for (slot, row_index) in row_indices.iter().enumerate() {
        let Some(row) = attributes.row(*row_index) else {
            continue;
        };
        let [name_area, value_area] = split_name_value(cell_areas[slot], min_first_panel);
        hitboxes.push(MetadataCellHitbox {
            row_index: *row_index,
            name_area,
            value_area,
        });
        render_metadata_cell(
            f,
            row,
            name_area,
            value_area,
            selection,
            *row_index == selected_row_index,
            highlighted_bg_color,
        );
    }
}

fn selected_attribute_bg_color(focus: &Focus, copying: bool, fallback_bg: Color) -> Color {
    match (focus, copying) {
        (Focus::Attributes, true) => {
            configure::themed_color(|colors| colors.surface.highlight_bg_copy)
        }
        (Focus::Attributes, false) => configure::themed_color(|colors| colors.surface.highlight_bg),
        _ => fallback_bg,
    }
}

pub fn prepare_metadata_layout(
    node: &mut H5FNode,
    outer_width: u16,
) -> Result<PreparedMetadataLayout, hdf5_metno::Error> {
    let attributes = node.read_attributes()?;
    Ok(PreparedMetadataLayout {
        initial_display_rows: build_rows_for_width(attributes, outer_width),
    })
}

pub fn render_info_attributes(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    state: &mut AppState,
    prepared_layout: Option<&PreparedMetadataLayout>,
) -> Result<(), hdf5_metno::Error> {
    let outer_area = *area;
    let bg = match (&state.focus, &state.mode) {
        (
            Focus::Attributes,
            Mode::Normal
            | Mode::AttributeCreateDialog
            | Mode::AttributeDeleteDialog
            | Mode::FixedStringOverflowDialog
            | Mode::FixedStringResizeDialog,
        ) => configure::themed_color(|colors| colors.surface.focus_bg),
        _ => configure::themed_color(|colors| colors.surface.bg),
    };

    let attr_header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(configure::themed_color(|colors| {
            colors.surface.panel_border
        })))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(configure::configured_symbol(|symbols| symbols.title.meta).to_string())
        .bg(bg)
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.surface.panel_title))
                .bold(),
        )
        .title_alignment(Alignment::Center);
    f.render_widget(attr_header_block, outer_area);

    let area_inner = outer_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let selected_row_index = node.normalize_attribute_selection()?.unwrap_or(0);
    let cursor = node.attributes_view_cursor.clone();
    let attributes = node.read_attributes()?;

    let owned_initial_display_rows;
    let initial_display_rows = if let Some(layout) = prepared_layout {
        layout.initial_display_rows.as_slice()
    } else {
        owned_initial_display_rows = build_rows_for_width(attributes, outer_area.width);
        owned_initial_display_rows.as_slice()
    };
    let scroll_size = if area_inner.height as usize >= initial_display_rows.len() {
        0
    } else {
        3
    };
    let [rows_area, scroll_area] = split_main_scroll(area_inner, scroll_size);
    let use_property_grid = should_use_property_grid(rows_area, attributes.longest_name_length);
    let display_rows = build_display_rows(attributes, use_property_grid);

    let clear_panel = Block::default().style(Style::default().bg(bg));
    f.render_widget(clear_panel.clone(), rows_area);
    if scroll_area.height > 0 && scroll_area.width > 0 {
        f.render_widget(clear_panel, scroll_area);
    }

    let selected_display_row = display_row_index(&display_rows, selected_row_index);
    let visible_rows = rows_area.height as usize;

    if scroll_area.height > 0 && scroll_area.width > 0 {
        let scrollbar = Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight)
            .end_symbol(Some("v"))
            .thumb_symbol("█")
            .begin_symbol(Some("^"));
        let mut scrollbar_state = ScrollbarState::new(display_rows.len())
            .viewport_content_length(visible_rows)
            .position(selected_display_row);
        f.render_stateful_widget(scrollbar, scroll_area, &mut scrollbar_state);
    }

    let new_display_offset = if selected_display_row
        > visible_rows
            .saturating_sub(1)
            .saturating_add(cursor.attribute_offset)
    {
        selected_display_row.saturating_sub(visible_rows.saturating_sub(1))
    } else if selected_display_row <= cursor.attribute_offset.saturating_add(1) {
        selected_display_row.saturating_sub(1)
    } else {
        cursor.attribute_offset
    };

    let highlighted_bg_color = selected_attribute_bg_color(&state.focus, state.copying, bg);

    let mut hitboxes = Vec::new();
    for (visible_index, display_row) in display_rows
        .iter()
        .skip(new_display_offset)
        .take(visible_rows)
        .enumerate()
    {
        let row_area = Rect {
            x: rows_area.x,
            y: rows_area.y.saturating_add(visible_index as u16),
            width: rows_area.width,
            height: 1,
        };
        match display_row {
            MetadataDisplayRow::SectionHeader(title) => render_section_header(f, row_area, title),
            MetadataDisplayRow::Cells(row_indices) if row_indices.len() == 2 => {
                render_property_grid_row(
                    f,
                    row_area,
                    attributes.longest_name_length.max(5),
                    row_indices,
                    attributes,
                    selected_row_index,
                    cursor.attribute_view_selection,
                    highlighted_bg_color,
                    &mut hitboxes,
                );
            }
            MetadataDisplayRow::Cells(row_indices) => {
                let Some(row_index) = row_indices.first().copied() else {
                    continue;
                };
                let Some(row) = attributes.row(row_index) else {
                    continue;
                };
                let [name_area, value_area] =
                    split_name_value(row_area, attributes.longest_name_length.max(5));
                hitboxes.push(MetadataCellHitbox {
                    row_index,
                    name_area,
                    value_area,
                });
                render_metadata_cell(
                    f,
                    row,
                    name_area,
                    value_area,
                    cursor.attribute_view_selection,
                    row_index == selected_row_index,
                    highlighted_bg_color,
                );
            }
        }
    }

    state.ui_layout.attributes = Some(AttributesHitbox {
        outer: outer_area,
        inner: area_inner,
        cells: hitboxes,
    });
    node.attributes_view_cursor.attribute_offset = new_display_offset;

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        build_display_rows, display_row_index, navigate_metadata_grid, selected_attribute_bg_color,
        MetadataDisplayRow,
    };
    use crate::h5f::RenderedAttributeRow;
    use crate::ui::{
        input::keymap::Direction,
        state::{AttributeViewSelection, Focus, LastFocused},
    };
    use ratatui::style::Color;
    use ratatui::text::Line;

    fn line(text: &str) -> Line<'static> {
        Line::from(text.to_string())
    }

    fn test_rows() -> Vec<RenderedAttributeRow> {
        vec![
            RenderedAttributeRow::section("Properties"),
            RenderedAttributeRow::property("path", (line("path"), line("/"), line(""))),
            RenderedAttributeRow::property("type", (line("type"), line("i32"), line(""))),
            RenderedAttributeRow::property("size", (line("size"), line("4 B"), line(""))),
            RenderedAttributeRow::section("Attributes"),
            RenderedAttributeRow::attribute("units", (line("units"), line("m"), line("(str)"))),
        ]
    }

    #[test]
    fn wide_layout_groups_properties_in_pairs() {
        let attributes = crate::h5f::ComputedAttributes {
            longest_name_length: 8,
            attributes: vec![],
            rendered_rows: test_rows(),
        };
        let display_rows = build_display_rows(&attributes, true);

        assert!(matches!(
            display_rows[0],
            MetadataDisplayRow::SectionHeader(_)
        ));
        assert!(
            matches!(display_rows[1], MetadataDisplayRow::Cells(ref cells) if cells == &vec![1, 2])
        );
        assert!(
            matches!(display_rows[2], MetadataDisplayRow::Cells(ref cells) if cells == &vec![3])
        );
        assert!(matches!(
            display_rows[3],
            MetadataDisplayRow::SectionHeader(_)
        ));
    }

    #[test]
    fn display_row_index_tracks_paired_property_rows() {
        let attributes = crate::h5f::ComputedAttributes {
            longest_name_length: 8,
            attributes: vec![],
            rendered_rows: test_rows(),
        };
        let display_rows = build_display_rows(&attributes, true);

        assert_eq!(display_row_index(&display_rows, 1), 1);
        assert_eq!(display_row_index(&display_rows, 2), 1);
        assert_eq!(display_row_index(&display_rows, 3), 2);
        assert_eq!(display_row_index(&display_rows, 5), 4);
    }

    #[test]
    fn right_moves_across_paired_properties_before_next_row() {
        let attributes = crate::h5f::ComputedAttributes {
            longest_name_length: 8,
            attributes: vec![],
            rendered_rows: test_rows(),
        };

        assert!(matches!(
            navigate_metadata_grid(
                &attributes,
                80,
                1,
                AttributeViewSelection::Name,
                Direction::Right
            ),
            Some((1, AttributeViewSelection::Value))
        ));
        assert!(matches!(
            navigate_metadata_grid(
                &attributes,
                80,
                1,
                AttributeViewSelection::Value,
                Direction::Right
            ),
            Some((2, AttributeViewSelection::Name))
        ));
    }

    #[test]
    fn down_keeps_visual_column_in_property_grid() {
        let attributes = crate::h5f::ComputedAttributes {
            longest_name_length: 8,
            attributes: vec![],
            rendered_rows: test_rows(),
        };

        assert!(matches!(
            navigate_metadata_grid(
                &attributes,
                80,
                2,
                AttributeViewSelection::Value,
                Direction::Down
            ),
            Some((3, AttributeViewSelection::Value))
        ));
    }

    #[test]
    fn selection_highlight_falls_back_to_panel_bg_when_unfocused() {
        let fallback_bg = Color::Blue;

        assert_eq!(
            selected_attribute_bg_color(&Focus::Attributes, false, fallback_bg),
            crate::configure::themed_color(|colors| colors.surface.highlight_bg)
        );
        assert_eq!(
            selected_attribute_bg_color(&Focus::Tree(LastFocused::Attributes), false, fallback_bg),
            fallback_bg
        );
        assert_eq!(
            selected_attribute_bg_color(&Focus::Content, true, fallback_bg),
            fallback_bg
        );
    }
}
