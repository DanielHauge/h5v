use std::{cell::RefCell, rc::Rc};

use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span, Text},
    Frame,
};

use crate::{
    data::{MatrixTable, MatrixValues},
    error::AppError,
    h5f::{H5FNode, Node::Dataset},
};

use super::{
    dims::{render_dim_selector, HasMatrixSelection, MatrixSelection},
    state::AppState,
};

pub fn render_matrix(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Rc<RefCell<H5FNode>>,
    state: &mut AppState,
) -> Result<(), AppError> {
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let node = &selected_node.borrow().node;
    let (ds, attr) = match node {
        Dataset(ds, attr) => (ds, attr),
        _ => {
            unreachable!("Should not render matrix for anything other than dataset")
        }
    };
    let shape_len = attr.shape.len();

    let matrix_area = if shape_len > 1 {
        let x_selectable_dims: Vec<usize> = attr
            .shape
            .iter()
            .enumerate()
            .filter(|(_, v)| **v > 1)
            .map(|(i, _)| i)
            .collect();

        let selected_indexe_length = state.selected_indexes.len();
        for i in 0..selected_indexe_length {
            if !x_selectable_dims.contains(&i) {
                state.selected_indexes[i] = 0;
            }
        }

        if !x_selectable_dims.contains(&state.selected_x_dim) {
            state.selected_x_dim = x_selectable_dims[0];
        }
        let areas_split =
            Layout::vertical(vec![Constraint::Length(4), Constraint::Min(1)]).split(area_inner);
        render_dim_selector(f, &areas_split[0], state, &attr.shape)?;
        areas_split[1].inner(ratatui::layout::Margin {
            horizontal: 0,
            vertical: 1,
        })
    } else {
        area_inner
    };
    let width = matrix_area.width;
    let heigh = matrix_area.height;
    let cols = width / 10;
    let rows = heigh;
    let matrix_selection = MatrixSelection { cols, rows };
    let slice_selection = state.get_matrix_selection(matrix_selection, &attr.shape);
    // panic!("{slice_selection}");

    if shape_len == 1 {
        let data = ds.matrix_values::<f64>(slice_selection)?;
        let mut lines = Vec::new();
        let mut i = state.matrix_view_state.row_offset;
        for d in data.data {
            let l = Line::from(format!("{i} - {d}"));
            i += 1;
            lines.push(l)
        }
        let p = Text::from(lines);
        f.render_widget(p, matrix_area);
    } else {
        let data = ds.matrix_table::<f64>(slice_selection)?;
        let mut lines = Vec::new();

        for i in 0..rows {
            let mut spans = Vec::new();
            spans.push(Span::raw(format!("{i}")));
            for j in 0..cols {
                let idx = (i as usize, j as usize);
                let val = data.data.get(idx);
                spans.push(Span::raw(" - "));
                match val {
                    Some(v) => spans.push(Span::from(format!("{v}"))),
                    None => spans.push(Span::raw("None")),
                }
            }
            let line = Line::from(spans);
            lines.push(line);
        }
        let text = Text::from(lines);
        f.render_widget(text, matrix_area);
    }

    Ok(())
}
