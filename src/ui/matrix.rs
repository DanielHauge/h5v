use std::fmt::Display;

use hdf5_metno::{types::EnumType, H5Type, Selection};
use ndarray::{Array1, Array2};
use ratatui::{
    layout::{Constraint, Layout, Offset, Rect},
    style::{Color, Style, Stylize},
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
    state::{AppState, MatrixCellHitbox, MatrixRowHitbox},
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
    pub enum_mapping: Vec<EnumRenderMember>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumRenderMember {
    pub value: u64,
    pub name: String,
    pub color: Color,
    pub symbol: &'static str,
}

impl EnumRenderer {
    pub fn new(enum_mapping: EnumType) -> Self {
        const ENUM_COLORS: [Color; 8] = [
            Color::Rgb(255, 204, 0),
            Color::Rgb(38, 166, 154),
            Color::Rgb(66, 165, 245),
            Color::Rgb(200, 140, 255),
            Color::Rgb(255, 112, 67),
            Color::Rgb(181, 206, 168),
            Color::Rgb(240, 98, 146),
            Color::Rgb(129, 199, 132),
        ];
        const ENUM_SYMBOLS: [&str; 8] = ["●", "■", "▲", "◆", "✦", "✚", "⬢", "◉"];
        let enum_mapping = enum_mapping
            .members
            .into_iter()
            .enumerate()
            .map(|(idx, member)| EnumRenderMember {
                value: member.value,
                name: member.name,
                color: ENUM_COLORS[idx % ENUM_COLORS.len()],
                symbol: ENUM_SYMBOLS[idx % ENUM_SYMBOLS.len()],
            })
            .collect();
        Self { enum_mapping }
    }

    fn member(&self, value: &u64) -> Option<&EnumRenderMember> {
        self.enum_mapping
            .iter()
            .find(|member| &member.value == value)
    }
}

impl RenderIntercept<u64> for EnumRenderer {
    fn render_as_line(&self, value: &u64) -> Line<'static> {
        match self.member(value) {
            Some(member) => Line::from(vec![
                Span::styled(
                    format!("{} ", member.symbol),
                    Style::default().fg(member.color).bold(),
                ),
                Span::styled(member.name.clone(), Style::default().fg(member.color)),
            ]),
            None => {
                Line::from(format!("Unknown enum value: {value}")).fg(color_consts::ERROR_COLOR)
            }
        }
    }
    fn render_as_span(&self, value: &u64) -> Span<'static> {
        match self.member(value) {
            Some(member) => Span::styled(
                format!("{} {}", member.symbol, member.name),
                Style::default().fg(member.color).bold(),
            ),
            None => {
                Span::from(format!("Unknown enum value: {value}")).fg(color_consts::ERROR_COLOR)
            }
        }
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

fn visible_matrix_capacity(matrix_area: Rect, row_len: usize, col_len: usize) -> MatrixSelection {
    MatrixSelection {
        cols: usize::from(matrix_area.width / 24).min(col_len),
        rows: usize::from(matrix_area.height).min(row_len),
    }
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
    let col_ds_len = attr.shape.get(node.selected_col).copied().unwrap_or(0);
    let row_ds_len = attr.shape.get(node.selected_row).copied().unwrap_or(0);
    let matrix_selection = visible_matrix_capacity(matrix_area, row_ds_len, col_ds_len);
    let max_cols = matrix_selection.cols;
    let max_rows = matrix_selection.rows;
    state.matrix_view_state.rows_currently_available = max_rows;
    state.matrix_view_state.cols_currently_available = max_cols;
    let slice_selection = state.get_matrix_selection(node, matrix_selection, &attr.shape);

    let mut rows_area_constraints = Vec::with_capacity(max_rows);
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
            state.ui_layout.matrix_rows.push(MatrixRowHitbox {
                area: idx_area,
                row: row_idx,
            });
            state.ui_layout.matrix_cells.push(MatrixCellHitbox {
                area: value_area,
                row: row_idx,
                col: 0,
            });
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

        let mut col_constraint = Vec::with_capacity(max_cols + 1);
        col_constraint.push(Constraint::Length(15));
        (0..max_cols).for_each(|_| col_constraint.push(Constraint::Fill(1)));
        let col_header_areas = Layout::horizontal(col_constraint).split(rows_areas[0]);

        for col in 0..max_cols {
            let col_area = col_header_areas[col + 1];
            let col_idx = state
                .matrix_view_state
                .col_offset
                .min(attr.shape[node.selected_col].saturating_sub(max_cols))
                + col;
            f.render_widget(
                Line::from(format!("{col_idx}"))
                    // .bg(color_consts::NUMBER_COLOR)
                    .centered(),
                col_area.offset(Offset { x: 0, y: -1 }),
            );
        }

        for i in 0..max_rows {
            let mut col_constraint = Vec::with_capacity(max_cols + 1);
            col_constraint.push(Constraint::Length(15));

            (0..max_cols).for_each(|_| col_constraint.push(Constraint::Fill(1)));
            let row_area = rows_areas[i];
            let col_areas = Layout::horizontal(col_constraint).split(row_area);
            let idx_area = col_areas[0];
            state.ui_layout.matrix_rows.push(MatrixRowHitbox {
                area: idx_area,
                row: i,
            });

            let idx = state.matrix_view_state.row_offset.min(
                attr.shape[node.selected_row]
                    .saturating_sub(state.matrix_view_state.rows_currently_available),
            ) + i;
            let idx_line = Line::from(format!("{idx}")).left_aligned();
            f.render_widget(idx_line, idx_area);
            for j in 0..max_cols {
                let val_area = col_areas[j + 1];
                state.ui_layout.matrix_cells.push(MatrixCellHitbox {
                    area: val_area,
                    row: i,
                    col: j,
                });

                let val_bg_color = match (
                    (i + state.matrix_view_state.row_offset).is_multiple_of(2),
                    (j + state.matrix_view_state.col_offset).is_multiple_of(2),
                ) {
                    (true, true) => color_consts::BG_VAL3_COLOR,
                    (true, false) => color_consts::BG_VAL4_COLOR,
                    (false, true) => color_consts::BG_VAL1_COLOR,
                    (false, false) => color_consts::BG_VAL2_COLOR,
                };
                let idx = if node.selected_row > node.selected_col {
                    (j, i)
                } else {
                    (i, j)
                };

                let val = data.get(idx);
                let val_bg_color = if idx.1 == state.matrix_view_state.cursor_col
                    && idx.0 == state.matrix_view_state.cursor_row
                {
                    let copying = state.copying;
                    if let (true, Focus::Content) = (copying, &state.focus) {
                        if val.is_some() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use hdf5_metno::types::{EnumMember, EnumType, IntSize};

    fn sample_enum() -> EnumType {
        EnumType {
            size: IntSize::U1,
            signed: false,
            members: vec![
                EnumMember {
                    name: "Red".to_string(),
                    value: 1,
                },
                EnumMember {
                    name: "Green".to_string(),
                    value: 2,
                },
            ],
        }
    }

    #[test]
    fn enum_renderer_includes_symbol_and_name() {
        let renderer = EnumRenderer::new(sample_enum());
        assert_eq!(renderer.render_as_line(&1).to_string(), "● Red");
        assert_eq!(renderer.render_as_span(&2).content, "■ Green");
    }

    #[test]
    fn enum_renderer_falls_back_for_unknown_values() {
        let renderer = EnumRenderer::new(EnumType {
            size: IntSize::U1,
            signed: false,
            members: vec![EnumMember {
                name: "Blue".to_string(),
                value: 7,
            }],
        });
        assert_eq!(
            renderer.render_as_line(&99).to_string(),
            "Unknown enum value: 99"
        );
    }

    #[test]
    fn visible_matrix_capacity_handles_large_dimensions_without_wrapping() {
        let selection = visible_matrix_capacity(
            Rect {
                x: 0,
                y: 0,
                width: 120,
                height: 20,
            },
            65_536,
            65_537,
        );
        assert_eq!(selection.rows, 20);
        assert_eq!(selection.cols, 5);
    }
}
