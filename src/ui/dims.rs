use hdf5_metno::{Error, Hyperslab, Selection, SliceOrIndex};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Offset, Rect},
    prelude::Stylize,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::{color_consts, h5f::H5FNode};

use super::state::AppState;

pub fn render_dim_selector(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    shape: &[usize],
    row_columns: bool,
) -> Result<(), Error> {
    node.sync_selection_rank(shape.len());
    let x_selection = node.selected_x;
    let row_selection = node.selected_row;
    let col_selection = node.selected_col;
    let selected_dim = node.selected_dim;
    let index_selection = &node.selected_indexes;
    let block = Block::default()
        .title("Slice selection")
        .title_style(
            Style::default()
                .fg(color_consts::panel_title_color())
                .bold(),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color_consts::panel_border_color()));
    f.render_widget(block, *area);

    let inner_area = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    let (labels_area, dims_area) = {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(8), Constraint::Min(0)])
            .split(inner_area);
        (chunks[0], chunks[1])
    };
    // Print Shape: and View: on each line
    let mut label_style = Style::default().fg(color_consts::type_desc_color());
    if color_consts::prefers_strong_text() {
        label_style = label_style.bold();
    }
    let shape_line = Line::from(Span::styled("Shape: ", label_style)).alignment(Alignment::Right);
    let view_line = if !row_columns {
        Line::from(Span::styled(" y = ", label_style)).alignment(Alignment::Right)
    } else {
        Line::from(Span::styled(" view = ", label_style)).alignment(Alignment::Right)
    };
    f.render_widget(shape_line, labels_area);
    f.render_widget(view_line, labels_area.offset(Offset { x: 0, y: 1 }));

    let shape_strings = shape.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let bounds: Vec<u16> = shape_strings.iter().map(|s| s.len() as u16).collect();
    let (segments, spacers) = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(3)
        .constraints(
            bounds
                .iter()
                .map(|&s| Constraint::Length(s.max(3)))
                .collect::<Vec<_>>(),
        )
        .split_with_spacers(dims_area);
    let spacers_len = spacers.len();

    for (i, spacer_area) in spacers.iter().enumerate() {
        if i == spacers_len - 1 {
            break;
        }
        let spacer = Paragraph::new(" | ")
            .style(Style::default().fg(color_consts::break_color()))
            .block(Block::default().borders(Borders::NONE));
        f.render_widget(&spacer, spacer_area.offset(Offset { x: 0, y: 1 }));
        f.render_widget(spacer, *spacer_area);
    }

    for (i, dim) in shape_strings.iter().enumerate() {
        let mut dim_span = Span::styled(
            dim.clone(),
            Style::default().fg(color_consts::primary_text_color()),
        );
        if color_consts::prefers_strong_text() {
            dim_span = dim_span.bold();
        }
        let dim_line = Line::from(dim_span).alignment(Alignment::Left);
        f.render_widget(dim_line, segments[i]);
        if i == col_selection && row_columns {
            let y_span = Span::from("Col").style(
                Style::default()
                    .bold()
                    .fg(color_consts::selected_dim_color()),
            );
            let y_line = Line::from(y_span).alignment(Alignment::Center);
            f.render_widget(y_line, segments[i].offset(Offset { x: 0, y: 1 }));
        } else if i == row_selection && row_columns {
            let x_text = "Row";
            let x_span = Span::from(x_text).style(
                Style::default()
                    .bold()
                    .fg(color_consts::selected_dim_color()),
            );
            let x_line = Line::from(x_span).alignment(Alignment::Center);
            f.render_widget(x_line, segments[i].offset(Offset { x: 0, y: 1 }));
        } else if i == x_selection && !row_columns {
            let x_span = Span::from("X").style(
                Style::default()
                    .bold()
                    .fg(color_consts::selected_dim_color()),
            );
            let x_line = Line::from(x_span).alignment(Alignment::Center);
            f.render_widget(x_line, segments[i].offset(Offset { x: 0, y: 1 }));
        } else if i == selected_dim {
            let selected_index = index_selection.get(i).copied().unwrap_or_default();
            let span = Span::from(format!("{}", selected_index)).style(
                Style::default()
                    .fg(color_consts::primary_text_color())
                    .bold()
                    .underlined()
                    .underline_color(color_consts::selected_index_color()),
            );
            let selected_line = Line::from(span).alignment(Alignment::Center);
            f.render_widget(selected_line, segments[i].offset(Offset { x: 0, y: 1 }));
        } else {
            let selected_index = index_selection.get(i).copied().unwrap_or_default();
            let mut span = Span::styled(
                format!("{}", selected_index),
                Style::default().fg(color_consts::primary_text_color()),
            );
            if color_consts::prefers_strong_text() {
                span = span.bold();
            }
            let selected_line = Line::from(span).alignment(Alignment::Center);
            f.render_widget(selected_line, segments[i].offset(Offset { x: 0, y: 1 }));
        }
    }

    Ok(())
}

pub struct MatrixSelection {
    pub cols: usize,
    pub rows: usize,
}

pub trait HasMatrixSelection {
    fn get_matrix_selection(
        &self,
        node: &mut H5FNode,
        select: MatrixSelection,
        total_dims: &[usize],
    ) -> Selection;
}

impl HasMatrixSelection for AppState<'_> {
    fn get_matrix_selection(
        &self,
        node: &mut H5FNode,
        matrix_view: MatrixSelection,
        shape: &[usize],
    ) -> Selection {
        node.sync_selection_rank(shape.len());
        let mut slice: Vec<SliceOrIndex> = Vec::new();
        let total_dims = shape.len();
        if total_dims == 1 {
            slice.push(SliceOrIndex::SliceTo {
                start: self
                    .matrix_view_state
                    .row_offset
                    .min(shape[0].saturating_sub(self.matrix_view_state.rows_currently_available)),
                step: 1,
                end: (self.matrix_view_state.row_offset + matrix_view.rows).min(shape[0]),
                block: 1,
            });
        } else {
            let selections = &node.selected_indexes;
            (0..total_dims).for_each(|dim| {
                if node.selected_col == dim {
                    slice.push(SliceOrIndex::SliceTo {
                        start: self.matrix_view_state.col_offset.min(
                            shape[dim]
                                .saturating_sub(self.matrix_view_state.cols_currently_available),
                        ),
                        step: 1,
                        end: (self.matrix_view_state.col_offset + matrix_view.cols).min(shape[dim]),
                        block: 1,
                    });
                } else if node.selected_row == dim {
                    slice.push(SliceOrIndex::SliceTo {
                        start: self.matrix_view_state.row_offset.min(
                            shape[dim]
                                .saturating_sub(self.matrix_view_state.rows_currently_available),
                        ),
                        step: 1,
                        end: (self.matrix_view_state.row_offset + matrix_view.rows).min(shape[dim]),
                        block: 1,
                    });
                } else {
                    slice.push(SliceOrIndex::Index(selections[dim]));
                }
            });
        }
        Selection::Hyperslab(Hyperslab::from(slice))
    }
}
