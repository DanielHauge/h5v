use std::{cell::RefCell, os::unix::raw::uid_t, rc::Rc};

use ratatui::{
    layout::{Alignment::Center, Constraint, Layout, Offset, Rect},
    style::Stylize,
    text::Line,
    Frame,
};

use crate::{
    color_consts,
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
        render_dim_selector(f, &areas_split[0], state, &attr.shape, true)?;
        areas_split[1].inner(ratatui::layout::Margin {
            horizontal: 0,
            vertical: 1,
        })
    } else {
        area_inner
    };
    let width = matrix_area.width;
    let heigh = matrix_area.height;
    let x_shape = attr
        .shape
        .get(state.selected_y_dim)
        .map(|x| *x as u16)
        .unwrap_or(0);
    let y_scale = attr
        .shape
        .get(state.selected_x_dim)
        .map(|x| *x as u16)
        .unwrap_or(0);
    let max_cols = (width / 24).min(x_shape);
    let rows = heigh.min(y_scale);
    state.matrix_view_state.rows_currently_available = rows as usize;
    state.matrix_view_state.cols_currently_available = max_cols as usize;
    let matrix_selection = MatrixSelection {
        cols: max_cols,
        rows,
    };
    let slice_selection = state.get_matrix_selection(matrix_selection, &attr.shape);

    let mut rows_area_constraints = Vec::with_capacity(rows as usize);
    (0..rows).for_each(|_| {
        rows_area_constraints.push(Constraint::Length(1));
    });

    let rows_areas = Layout::vertical(rows_area_constraints).split(matrix_area);

    if shape_len == 1 {
        let data = ds.matrix_values::<f64>(slice_selection)?;
        let mut i = state.matrix_view_state.row_offset.min(
            attr.shape[state.selected_x_dim] - state.matrix_view_state.rows_currently_available,
        );
        for (row_idx, d) in data.data.iter().enumerate() {
            let row_area = rows_areas[row_idx];
            let areas_split =
                Layout::horizontal(vec![Constraint::Max(15), Constraint::Min(16)]).split(row_area);
            let idx_area = areas_split[0];
            let value_area = areas_split[1];
            let val_bg_color = match (row_idx % 2) == 0 {
                true => match (state.matrix_view_state.row_offset % 2) == 0 {
                    true => color_consts::BG_VAL3_COLOR,
                    false => color_consts::BG_VAL4_COLOR,
                },
                false => match (state.matrix_view_state.row_offset % 2) == 0 {
                    true => color_consts::BG_VAL4_COLOR,
                    false => color_consts::BG_VAL3_COLOR,
                },
            };
            let idx_line = Line::from(format!("{i}")).left_aligned();
            let value_line = Line::from(format!("{d}"))
                .alignment(Center)
                .bg(val_bg_color);
            f.render_widget(idx_line, idx_area);
            f.render_widget(value_line, value_area);
            i += 1;
        }
    } else {
        let data = ds.matrix_table::<f64>(slice_selection)?;

        let mut col_constraint = Vec::with_capacity((max_cols + 1) as usize);
        col_constraint.push(Constraint::Length(15));
        (0..max_cols).for_each(|_| col_constraint.push(Constraint::Fill(1)));
        let col_header_areas = Layout::horizontal(col_constraint).split(rows_areas[0]);

        for col in 0..max_cols {
            let col_area = col_header_areas[(col + 1) as usize];
            let col_idx = state
                .matrix_view_state
                .col_offset
                .min(attr.shape[state.selected_y_dim] - max_cols as usize)
                + col as usize;
            f.render_widget(
                Line::from(format!("{col_idx}"))
                    // .bg(color_consts::NUMBER_COLOR)
                    .centered(),
                col_area.offset(Offset { x: 0, y: -1 }),
            );
        }

        for i in 0..rows {
            let mut col_constraint = Vec::with_capacity((max_cols + 1) as usize);
            col_constraint.push(Constraint::Length(15));

            (0..max_cols).for_each(|_| col_constraint.push(Constraint::Fill(1)));
            let row_area = rows_areas[i as usize];
            let col_areas = Layout::horizontal(col_constraint).split(row_area);
            let idx_area = col_areas[0];

            let idx = state.matrix_view_state.row_offset.min(
                attr.shape[state.selected_x_dim] - state.matrix_view_state.rows_currently_available,
            ) + i as usize;
            let idx_line = Line::from(format!("{idx}")).left_aligned();
            f.render_widget(idx_line, idx_area);
            for j in 0..max_cols {
                let val_area = col_areas[(j + 1) as usize];

                let val_bg_color = match (
                    (i as usize + state.matrix_view_state.row_offset) % 2 == 0,
                    (j as usize + state.matrix_view_state.col_offset) % 2 == 0,
                ) {
                    (true, true) => color_consts::BG_VAL3_COLOR,
                    (true, false) => color_consts::BG_VAL4_COLOR,
                    (false, true) => color_consts::BG_VAL1_COLOR,
                    (false, false) => color_consts::BG_VAL2_COLOR,
                };
                let idx = (i as usize, j as usize);
                let val = data.data.get(idx);

                match val {
                    Some(v) => f.render_widget(
                        Line::from(format!("{v}")).bg(val_bg_color).centered(),
                        val_area,
                    ),
                    None => f.render_widget("None", val_area),
                }
            }
        }
    }

    Ok(())
}
