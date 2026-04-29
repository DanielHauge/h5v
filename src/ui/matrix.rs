use std::fmt::Display;

use hdf5_metno::{types::EnumType, H5Type, Selection};
use ndarray::{Array1, Array2};
use ratatui::{
    layout::{Constraint, Layout, Offset, Rect},
    style::Stylize,
    text::{Line, Span},
    Frame,
};

use crate::{
    color_consts,
    data::{MatrixTable, MatrixValues},
    error::AppError,
    h5f::{read_projected_values_1d, read_projected_values_2d, DatasetMeta, H5FNode},
    ui::state::Focus,
};

use super::{
    dims::{render_dim_selector, HasMatrixSelection, MatrixSelection},
    state::AppState,
};
pub fn render_not_yet_implemented(f: &mut Frame, area: &Rect, desc: &str) {
    let inner_area = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let unsupported_msg = "Not yet implemented:".to_string();
    f.render_widget(unsupported_msg, inner_area);
    let why = desc.to_string();
    f.render_widget(
        why,
        inner_area.inner(ratatui::layout::Margin {
            horizontal: 2,
            vertical: 1,
        }),
    );
}

pub trait RenderIntercept<T: Display> {
    fn render_as_line(&self, value: &T) -> Line<'static>;
    fn render_as_span(&self, value: &T) -> Span<'static>;
}

pub struct DefaultMatrixResultRenderIntercept;

impl<T: Display> RenderIntercept<T> for DefaultMatrixResultRenderIntercept {
    fn render_as_line(&self, value: &T) -> Line<'static> {
        Line::from(format!("{value}"))
    }

    fn render_as_span(&self, value: &T) -> Span<'static> {
        Span::from(format!("{value}"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumRenderer {
    pub enum_mapping: Vec<(u64, String)>,
}

impl EnumRenderer {
    pub fn new(enum_mapping: EnumType) -> Self {
        let enum_mapping = enum_mapping
            .members
            .into_iter()
            .map(|v| (v.value, v.name))
            .collect();
        Self { enum_mapping }
    }
}

impl RenderIntercept<u64> for EnumRenderer {
    fn render_as_line(&self, value: &u64) -> Line<'static> {
        let mapped = self
            .enum_mapping
            .iter()
            .find(|(v, _)| v == value)
            .map(|(_, s)| s.clone())
            .unwrap_or_else(|| format!("Unknown enum value: {value}"));
        Line::from(mapped).fg(color_consts::NUMBER_COLOR)
    }
    fn render_as_span(&self, value: &u64) -> Span<'static> {
        let mapped = self
            .enum_mapping
            .iter()
            .find(|(v, _)| v == value)
            .map(|(_, s)| s.clone())
            .unwrap_or_else(|| format!("Unknown enum value: {value}"));
        Span::from(mapped).fg(color_consts::NUMBER_COLOR)
    }
}

pub fn render_matrix<T: H5Type + Display>(
    f: &mut Frame,
    area: &Rect,
    ds: &hdf5_metno::Dataset,
    attr: &DatasetMeta,
    node: &mut H5FNode,
    state: &mut AppState,
    result_render: impl RenderIntercept<T>,
) -> Result<(), AppError> {
    render_matrix_with_reader(
        f,
        area,
        attr,
        node,
        state,
        |selection| Ok(ds.matrix_values::<T>(selection)?.data),
        |selection| Ok(ds.matrix_table::<T>(selection)?.data),
        result_render,
    )
}

pub fn render_projected_matrix<T: Display + crate::h5f::ProjectionDecode>(
    f: &mut Frame,
    area: &Rect,
    ds: &hdf5_metno::Dataset,
    attr: &DatasetMeta,
    node: &mut H5FNode,
    state: &mut AppState,
    result_render: impl RenderIntercept<T>,
) -> Result<(), AppError> {
    render_matrix_with_reader(
        f,
        area,
        attr,
        node,
        state,
        |selection| read_projected_values_1d::<T>(ds, attr, selection),
        |selection| read_projected_values_2d::<T>(ds, attr, selection),
        result_render,
    )
}

fn render_matrix_with_reader<T: Display>(
    f: &mut Frame,
    area: &Rect,
    attr: &DatasetMeta,
    node: &mut H5FNode,
    state: &mut AppState,
    read_values: impl Fn(Selection) -> Result<Array1<T>, AppError>,
    read_table: impl Fn(Selection) -> Result<Array2<T>, AppError>,
    result_render: impl RenderIntercept<T>,
) -> Result<(), AppError> {
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let shape_len = attr.shape.len();
    node.sync_selection_rank(shape_len);

    let matrix_area = if shape_len > 1 {
        let x_selectable_dims: Vec<usize> = attr
            .shape
            .iter()
            .enumerate()
            .filter(|(_, v)| **v > 1)
            .map(|(i, _)| i)
            .collect();

        for (i, selected_index) in node.selected_indexes.iter_mut().enumerate() {
            if !x_selectable_dims.contains(&i) {
                *selected_index = 0;
            }
        }

        // if !x_selectable_dims.contains(&node.selected_row) {
        //     node.selected_row = x_selectable_dims[0];
        // }
        if node.selected_dim == node.selected_row || node.selected_dim == node.selected_col {
            node.selected_dim = x_selectable_dims
                .iter()
                .find(|&&x| x != node.selected_row && x != node.selected_col)
                .cloned()
                .unwrap_or(0);
        }
        let areas_split =
            Layout::vertical(vec![Constraint::Length(4), Constraint::Min(1)]).split(area_inner);
        render_dim_selector(f, &areas_split[0], node, &attr.shape, true)?;
        areas_split[1].inner(ratatui::layout::Margin {
            horizontal: 0,
            vertical: 1,
        })
    } else {
        area_inner
    };
    let width = matrix_area.width;
    let heigh = matrix_area.height;

    let col_ds_len = attr
        .shape
        .get(node.selected_col)
        .map(|x| *x as u16)
        .unwrap_or(0);
    let row_ds_len = attr
        .shape
        .get(node.selected_row)
        .map(|x| *x as u16)
        .unwrap_or(0);

    let max_cols = (width / 24).min(col_ds_len);
    let max_rows = heigh.min(row_ds_len);
    state.matrix_view_state.rows_currently_available = max_rows as usize;
    state.matrix_view_state.cols_currently_available = max_cols as usize;
    let matrix_selection = MatrixSelection {
        cols: max_cols,
        rows: max_rows,
    };
    let slice_selection = state.get_matrix_selection(node, matrix_selection, &attr.shape);

    let mut rows_area_constraints = Vec::with_capacity(max_rows as usize);
    (0..max_rows).for_each(|_| {
        rows_area_constraints.push(Constraint::Length(1));
    });

    let rows_areas = Layout::vertical(rows_area_constraints).split(matrix_area);

    if shape_len == 1 {
        let data = read_values(slice_selection)?;
        let mut i = state.matrix_view_state.row_offset.min(
            attr.shape[node.selected_row]
                .saturating_sub(state.matrix_view_state.rows_currently_available),
        );
        for (row_idx, d) in data.iter().enumerate() {
            let row_area = rows_areas[row_idx];
            let areas_split =
                Layout::horizontal(vec![Constraint::Max(15), Constraint::Min(16)]).split(row_area);
            let idx_area = areas_split[0];
            let value_area = areas_split[1];
            let val_bg_color = match (row_idx % 2) == 0 {
                true => match state.matrix_view_state.row_offset.is_multiple_of(2) {
                    true => color_consts::BG_VAL3_COLOR,
                    false => color_consts::BG_VAL4_COLOR,
                },
                false => match state.matrix_view_state.row_offset.is_multiple_of(2) {
                    true => color_consts::BG_VAL4_COLOR,
                    false => color_consts::BG_VAL3_COLOR,
                },
            };
            let val_bg_color = if row_idx == state.matrix_view_state.cursor_row {
                let copying = state.copying;
                if let (true, Focus::Content) = (copying, &state.focus) {
                    state.clipboard.set_text(format!("{d}")).map_err(|_| {
                        AppError::ClipboardError("Could not copy data as text".to_string())
                    })?;
                    color_consts::HIGHLIGHT_BG_COLOR_COPY
                } else {
                    color_consts::HIGHLIGHT_BG_COLOR
                }
            } else {
                val_bg_color
            };
            let idx_line = Line::from(format!("{i}")).left_aligned();
            let value_line = result_render
                .render_as_line(d)
                .alignment(ratatui::layout::Alignment::Center)
                .bg(val_bg_color);
            f.render_widget(idx_line, idx_area);
            f.render_widget(value_line, value_area);
            i += 1;
        }
    } else {
        let data = read_table(slice_selection)?;

        let mut col_constraint = Vec::with_capacity((max_cols + 1) as usize);
        col_constraint.push(Constraint::Length(15));
        (0..max_cols).for_each(|_| col_constraint.push(Constraint::Fill(1)));
        let col_header_areas = Layout::horizontal(col_constraint).split(rows_areas[0]);

        for col in 0..max_cols {
            let col_area = col_header_areas[(col + 1) as usize];
            let col_idx = state
                .matrix_view_state
                .col_offset
                .min(attr.shape[node.selected_col].saturating_sub(max_cols as usize))
                + col as usize;
            f.render_widget(
                Line::from(format!("{col_idx}"))
                    // .bg(color_consts::NUMBER_COLOR)
                    .centered(),
                col_area.offset(Offset { x: 0, y: -1 }),
            );
        }

        for i in 0..max_rows {
            let mut col_constraint = Vec::with_capacity((max_cols + 1) as usize);
            col_constraint.push(Constraint::Length(15));

            (0..max_cols).for_each(|_| col_constraint.push(Constraint::Fill(1)));
            let row_area = rows_areas[i as usize];
            let col_areas = Layout::horizontal(col_constraint).split(row_area);
            let idx_area = col_areas[0];

            let idx = state.matrix_view_state.row_offset.min(
                attr.shape[node.selected_row]
                    .saturating_sub(state.matrix_view_state.rows_currently_available),
            ) + i as usize;
            let idx_line = Line::from(format!("{idx}")).left_aligned();
            f.render_widget(idx_line, idx_area);
            for j in 0..max_cols {
                let val_area = col_areas[(j + 1) as usize];

                let val_bg_color = match (
                    (i as usize + state.matrix_view_state.row_offset).is_multiple_of(2),
                    (j as usize + state.matrix_view_state.col_offset).is_multiple_of(2),
                ) {
                    (true, true) => color_consts::BG_VAL3_COLOR,
                    (true, false) => color_consts::BG_VAL4_COLOR,
                    (false, true) => color_consts::BG_VAL1_COLOR,
                    (false, false) => color_consts::BG_VAL2_COLOR,
                };
                let idx = if node.selected_row > node.selected_col {
                    (j as usize, i as usize)
                } else {
                    (i as usize, j as usize)
                };

                let val = data.get(idx);
                let val_bg_color = if idx.1 == state.matrix_view_state.cursor_col
                    && idx.0 == state.matrix_view_state.cursor_row
                {
                    let copying = state.copying;
                    if let (true, Focus::Content) = (copying, &state.focus) {
                        if let Some(v) = val {
                            state.clipboard.set_text(format!("{v}")).map_err(|_| {
                                AppError::ClipboardError("Could not copy data as text".to_string())
                            })?;
                            color_consts::HIGHLIGHT_BG_COLOR_COPY
                        } else {
                            color_consts::ERROR_COLOR
                        }
                    } else {
                        color_consts::HIGHLIGHT_BG_COLOR
                    }
                } else {
                    val_bg_color
                };

                match val {
                    Some(v) => f.render_widget(
                        result_render.render_as_line(v).bg(val_bg_color).centered(),
                        val_area,
                    ),
                    None => f.render_widget("None", val_area),
                }
            }
        }
    }

    Ok(())
}
