use hdf5_metno::{Error, Hyperslab, Selection, SliceOrIndex};
use ratatui::{
    layout::{Constraint, Margin, Offset, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{color_consts, h5f::H5FNode};

use super::state::AppState;

pub fn render_dim_selector(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    state: &mut AppState,
    shape: &[usize],
    row_columns: bool,
) -> Result<(), Error> {
    let x_selection = node.selected_x;
    let row_selection = node.selected_row;
    let col_selection = node.selected_col;
    let selected_dim = node.selected_dim;
    let index_selection = state.selected_indexes;
    let block = Block::default()
        .title("Slice selection")
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN));
    f.render_widget(block, *area);

    let inner_area = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    let (labels_area, dims_area) = {
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Length(8), Constraint::Min(0)].as_ref())
            .split(inner_area);
        (chunks[0], chunks[1])
    };
    // Print Shape: and View: on each line
    let shape_line = Line::from("Shape: ").alignment(ratatui::layout::Alignment::Right);
    let view_line = if !row_columns {
        Line::from(" y = ").alignment(ratatui::layout::Alignment::Right)
    } else {
        Line::from(" view = ").alignment(ratatui::layout::Alignment::Right)
    };
    f.render_widget(shape_line, labels_area);
    f.render_widget(view_line, labels_area.offset(Offset { x: 0, y: 1 }));

    let shape_strings = shape.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let bounds: Vec<u16> = shape_strings.iter().map(|s| s.len() as u16).collect();
    let (segments, spacers) = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
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
            // .style(Style::default().bg(color_consts::BG_COLOR))
            .block(Block::default().borders(Borders::NONE));
        f.render_widget(&spacer, spacer_area.offset(Offset { x: 0, y: 1 }));
        f.render_widget(spacer, *spacer_area);
    }

    for (i, dim) in shape_strings.iter().enumerate() {
        let dim_line = Line::from(dim.as_str()).alignment(ratatui::layout::Alignment::Left);
        f.render_widget(dim_line, segments[i]);
        if i == col_selection && row_columns {
            let y_span =
                Span::from("Col").style(Style::default().bold().fg(color_consts::SELECTED_DIM));
            let y_line = Line::from(y_span).alignment(ratatui::layout::Alignment::Center);
            f.render_widget(y_line, segments[i].offset(Offset { x: 0, y: 1 }));
        } else if i == row_selection && row_columns {
            let x_text = "Row";
            let x_span =
                Span::from(x_text).style(Style::default().bold().fg(color_consts::SELECTED_DIM));
            let x_line = Line::from(x_span).alignment(ratatui::layout::Alignment::Center);
            f.render_widget(x_line, segments[i].offset(Offset { x: 0, y: 1 }));
        } else if i == x_selection && !row_columns {
            let x_span =
                Span::from("X").style(Style::default().bold().fg(color_consts::SELECTED_DIM));
            let x_line = Line::from(x_span).alignment(ratatui::layout::Alignment::Center);
            f.render_widget(x_line, segments[i].offset(Offset { x: 0, y: 1 }));
        } else if i == selected_dim {
            let selected_index = index_selection[i];
            let span = Span::from(format!("{}", selected_index)).style(
                Style::default()
                    .bold()
                    .underlined()
                    .underline_color(color_consts::SELECTED_INDEX),
            );
            let selected_line = Line::from(span).alignment(ratatui::layout::Alignment::Center);
            f.render_widget(selected_line, segments[i].offset(Offset { x: 0, y: 1 }));
        } else {
            let selected_index = index_selection[i];
            let selected_line = Line::from(format!("{}", selected_index))
                .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(selected_line, segments[i].offset(Offset { x: 0, y: 1 }));
        }
    }

    Ok(())
}

pub struct MatrixSelection {
    pub cols: u16,
    pub rows: u16,
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
        let mut slice: Vec<SliceOrIndex> = Vec::new();
        let total_dims = shape.len();
        if total_dims == 1 {
            slice.push(SliceOrIndex::SliceTo {
                start: self
                    .matrix_view_state
                    .row_offset
                    .min(shape[0] - self.matrix_view_state.rows_currently_available),
                step: 1,
                end: (self.matrix_view_state.row_offset + matrix_view.rows as usize).min(shape[0]),
                block: 1,
            });
        } else {
            let selections = self.selected_indexes;
            (0..total_dims).for_each(|dim| {
                if node.selected_col == dim {
                    slice.push(SliceOrIndex::SliceTo {
                        start: self.matrix_view_state.col_offset.min(shape[dim] - 1),
                        step: 1,
                        end: (self.matrix_view_state.col_offset + matrix_view.cols as usize)
                            .min(shape[dim]),
                        block: 1,
                    });
                } else if node.selected_row == dim {
                    slice.push(SliceOrIndex::SliceTo {
                        start: self
                            .matrix_view_state
                            .row_offset
                            .min(shape[dim] - self.matrix_view_state.rows_currently_available),
                        step: 1,
                        end: (self.matrix_view_state.row_offset + matrix_view.rows as usize)
                            .min(shape[dim]),
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
// const MAX_PAGE_SIZE: usize = 250000;
// pub trait HasSelection {
//     fn get_selection(&self) -> Selection;
// }
//
// impl HasSelection for AppState<'_> {
//     fn get_selection(&self) -> Selection {
//         let x = self.selected_x_dim;
//         let sels = self.selected_indexes;
//         let page = self.page;
//         generate_selector_slice(x, &sels, page)
//     }
// }
// fn generate_selector_slice(x: usize, selections: &[usize], page: usize) -> Selection {
//     let mut slice: Vec<SliceOrIndex> = Vec::new();
//     let total_dims = selections.len();
//     let start = page * MAX_PAGE_SIZE;
//     let end = start + MAX_PAGE_SIZE;
//     (0..total_dims).for_each(|dim| {
//         if x == dim {
//             slice.push(SliceOrIndex::SliceTo {
//                 start,
//                 end,
//                 step: 1,
//                 block: 1,
//             });
//         } else {
//             slice.push(SliceOrIndex::Index(selections[dim]));
//         }
//     });
//
//     Selection::Hyperslab(Hyperslab::from(slice))
// }
