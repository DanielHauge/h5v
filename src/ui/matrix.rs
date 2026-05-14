use std::fmt::Display;

use hdf5_metno::{
    types::{EnumMember, EnumType, IntSize},
    H5Type, Selection,
};
use ndarray::{Array1, Array2};
use ratatui::{
    layout::{Constraint, Layout, Offset, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::{
    configure,
    data::{MatrixTable, MatrixValues},
    error::AppError,
    h5f::{
        read_opaque_values_1d, read_opaque_values_2d, read_projected_values_1d,
        read_projected_values_2d, read_varlen_u8_matrix_table, read_varlen_u8_matrix_values,
        DatasetMeta, EnumRenderOverrides, H5FNode,
    },
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
    let mut text_style = Style::default().fg(configure::themed_color(|colors| colors.text.primary));
    if configure::prefers_strong_text() {
        text_style = text_style.bold();
    }
    f.render_widget(
        Paragraph::new(unsupported_msg).style(text_style),
        inner_area,
    );
    let why = desc.to_string();
    f.render_widget(
        Paragraph::new(why).style(text_style),
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
        let mut span = Span::styled(
            format!("{value}"),
            Style::default().fg(configure::themed_color(|colors| colors.text.primary)),
        );
        if configure::prefers_strong_text() {
            span = span.bold();
        }
        Line::from(span)
    }

    fn render_as_span(&self, value: &T) -> Span<'static> {
        let mut span = Span::styled(
            format!("{value}"),
            Style::default().fg(configure::themed_color(|colors| colors.text.primary)),
        );
        if configure::prefers_strong_text() {
            span = span.bold();
        }
        span
    }
}

pub struct OpaqueHexRenderIntercept;

impl RenderIntercept<String> for OpaqueHexRenderIntercept {
    fn render_as_line(&self, value: &String) -> Line<'static> {
        Line::from(Span::styled(
            value.clone(),
            Style::default().fg(configure::themed_color(|colors| colors.text.opaque)),
        ))
    }

    fn render_as_span(&self, value: &String) -> Span<'static> {
        Span::styled(
            value.clone(),
            Style::default().fg(configure::themed_color(|colors| colors.text.opaque)),
        )
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
    pub symbol: String,
}

impl EnumRenderer {
    pub fn new(enum_mapping: EnumType) -> Self {
        Self::with_overrides(enum_mapping, None)
    }

    pub fn with_overrides(enum_mapping: EnumType, overrides: Option<&EnumRenderOverrides>) -> Self {
        let EnumType {
            size,
            signed,
            mut members,
        } = enum_mapping;
        members.sort_by_key(|member| enum_member_sort_key(signed, size, member));

        let enum_mapping = members
            .into_iter()
            .enumerate()
            .map(|(idx, member)| EnumRenderMember {
                value: member.value,
                name: member.name,
                color: overrides
                    .and_then(|overrides| overrides.colors.get(idx).copied().flatten())
                    .unwrap_or(crate::configure::themed_color(|colors| {
                        colors.chart.enums[idx % colors.chart.enums.len()]
                    })),
                symbol: overrides
                    .and_then(|overrides| overrides.symbols.get(idx))
                    .and_then(|symbol| symbol.clone())
                    .unwrap_or_else(|| {
                        configure::configured_symbol(|symbols| {
                            symbols.chart.r#enum[idx % symbols.chart.r#enum.len()]
                        })
                        .to_string()
                    }),
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

fn enum_member_sort_key(signed: bool, size: IntSize, member: &EnumMember) -> i128 {
    if signed {
        match size {
            IntSize::U1 => (member.value as u8 as i8) as i128,
            IntSize::U2 => (member.value as u16 as i16) as i128,
            IntSize::U4 => (member.value as u32 as i32) as i128,
            IntSize::U8 => (member.value as i64) as i128,
        }
    } else {
        member.value as i128
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
            None => Line::from(format!("Unknown enum value: {value}"))
                .fg(configure::themed_color(|colors| colors.text.error)),
        }
    }
    fn render_as_span(&self, value: &u64) -> Span<'static> {
        match self.member(value) {
            Some(member) => Span::styled(
                format!("{} {}", member.symbol, member.name),
                Style::default().fg(member.color).bold(),
            ),
            None => Span::from(format!("Unknown enum value: {value}"))
                .fg(configure::themed_color(|colors| colors.text.error)),
        }
    }
}

fn render_centered_matrix_cell(f: &mut Frame, area: Rect, line: Line<'static>, bg_color: Color) {
    let mut style = Style::default()
        .bg(bg_color)
        .fg(configure::themed_color(|colors| colors.text.primary));
    if configure::prefers_strong_text() {
        style = style.bold();
    }
    f.render_widget(
        Paragraph::new(line)
            .alignment(ratatui::layout::Alignment::Center)
            .style(style),
        area,
    );
}

fn selected_matrix_bg_color(
    focus: &Focus,
    copying: bool,
    fallback_bg: Color,
    has_value: bool,
) -> Color {
    match (focus, copying) {
        (Focus::Content, true) if has_value => {
            configure::themed_color(|colors| colors.surface.highlight_bg_copy)
        }
        (Focus::Content, true) => configure::themed_color(|colors| colors.text.error),
        (Focus::Content, false) => configure::themed_color(|colors| colors.surface.highlight_bg),
        _ => fallback_bg,
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

pub fn render_opaque_matrix(
    f: &mut Frame,
    area: &Rect,
    ds: &hdf5_metno::Dataset,
    attr: &DatasetMeta,
    node: &mut H5FNode,
    state: &mut AppState,
) -> Result<(), AppError> {
    render_matrix_with_reader(
        f,
        area,
        attr,
        node,
        state,
        |selection| read_opaque_values_1d(ds, selection),
        |selection| read_opaque_values_2d(ds, selection),
        OpaqueHexRenderIntercept,
    )
}

pub fn render_varlen_u8_matrix(
    f: &mut Frame,
    area: &Rect,
    ds: &hdf5_metno::Dataset,
    attr: &DatasetMeta,
    node: &mut H5FNode,
    state: &mut AppState,
) -> Result<(), AppError> {
    render_matrix_with_reader(
        f,
        area,
        attr,
        node,
        state,
        |selection| read_varlen_u8_matrix_values(ds, selection),
        |selection| read_varlen_u8_matrix_table(ds, selection),
        DefaultMatrixResultRenderIntercept,
    )
}

fn visible_matrix_capacity(matrix_area: Rect, row_len: usize, col_len: usize) -> MatrixSelection {
    MatrixSelection {
        cols: usize::from(matrix_area.width / 24).min(col_len),
        rows: usize::from(matrix_area.height).min(row_len),
    }
}

#[allow(clippy::too_many_arguments)]
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
        render_dim_selector(f, &areas_split[0], node, &attr.shape, true, None)?;
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
                    true => configure::themed_color(|colors| colors.surface.bg_val3),
                    false => configure::themed_color(|colors| colors.surface.bg_val4),
                },
                false => match state.matrix_view_state.row_offset.is_multiple_of(2) {
                    true => configure::themed_color(|colors| colors.surface.bg_val4),
                    false => configure::themed_color(|colors| colors.surface.bg_val3),
                },
            };
            let val_bg_color = if row_idx == state.matrix_view_state.cursor_row {
                selected_matrix_bg_color(&state.focus, state.copying, val_bg_color, true)
            } else {
                val_bg_color
            };
            let mut idx_line = Line::from(format!("{i}"))
                .style(Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)))
                .left_aligned();
            if configure::prefers_strong_text() {
                idx_line = idx_line.bold();
            }
            let value_line = result_render.render_as_line(d);
            f.render_widget(idx_line, idx_area);
            render_centered_matrix_cell(f, value_area, value_line, val_bg_color);
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
                Line::from(Span::styled(format!("{col_idx}"), {
                    let mut style = Style::default()
                        .fg(configure::themed_color(|colors| colors.text.type_desc));
                    if configure::prefers_strong_text() {
                        style = style.bold();
                    }
                    style
                }))
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
            let mut idx_line = Line::from(format!("{idx}"))
                .style(Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)))
                .left_aligned();
            if configure::prefers_strong_text() {
                idx_line = idx_line.bold();
            }
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
                    (true, true) => configure::themed_color(|colors| colors.surface.bg_val3),
                    (true, false) => configure::themed_color(|colors| colors.surface.bg_val4),
                    (false, true) => configure::themed_color(|colors| colors.surface.bg_val1),
                    (false, false) => configure::themed_color(|colors| colors.surface.bg_val2),
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
                    selected_matrix_bg_color(
                        &state.focus,
                        state.copying,
                        val_bg_color,
                        val.is_some(),
                    )
                } else {
                    val_bg_color
                };

                match val {
                    Some(v) => render_centered_matrix_cell(
                        f,
                        val_area,
                        result_render.render_as_line(v),
                        val_bg_color,
                    ),
                    None => render_centered_matrix_cell(
                        f,
                        val_area,
                        Line::from("None").fg(configure::themed_color(|colors| colors.text.error)),
                        val_bg_color,
                    ),
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::ui::state::{Focus, LastFocused};
    use hdf5_metno::types::{EnumMember, EnumType, IntSize};
    use ratatui::style::Color;

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
    fn enum_renderer_uses_member_overrides_when_present() {
        let overrides = EnumRenderOverrides {
            colors: vec![Some(Color::Green), None],
            symbols: vec![Some("✓".to_string()), None],
        };
        let renderer = EnumRenderer::with_overrides(sample_enum(), Some(&overrides));
        assert_eq!(renderer.render_as_line(&1).to_string(), "✓ Red");
        assert_eq!(renderer.render_as_span(&2).content, "■ Green");
    }

    #[test]
    fn enum_renderer_applies_overrides_by_numeric_value_order() {
        let overrides = EnumRenderOverrides {
            colors: vec![Some(Color::Green), Some(Color::Yellow), Some(Color::Red)],
            symbols: vec![
                Some("✓".to_string()),
                Some("⚠".to_string()),
                Some("✗".to_string()),
            ],
        };
        let renderer = EnumRenderer::with_overrides(
            EnumType {
                size: IntSize::U1,
                signed: false,
                members: vec![
                    EnumMember {
                        name: "AMBER".to_string(),
                        value: 1,
                    },
                    EnumMember {
                        name: "GREEN".to_string(),
                        value: 0,
                    },
                    EnumMember {
                        name: "RED".to_string(),
                        value: 2,
                    },
                ],
            },
            Some(&overrides),
        );
        assert_eq!(renderer.render_as_line(&0).to_string(), "✓ GREEN");
        assert_eq!(renderer.render_as_line(&1).to_string(), "⚠ AMBER");
        assert_eq!(renderer.render_as_line(&2).to_string(), "✗ RED");
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

    #[test]
    fn selected_matrix_bg_respects_content_focus() {
        let fallback_bg = Color::Blue;

        assert_eq!(
            selected_matrix_bg_color(&Focus::Content, false, fallback_bg, true),
            crate::configure::themed_color(|colors| colors.surface.highlight_bg)
        );
        assert_eq!(
            selected_matrix_bg_color(&Focus::Content, true, fallback_bg, true),
            crate::configure::themed_color(|colors| colors.surface.highlight_bg_copy)
        );
        assert_eq!(
            selected_matrix_bg_color(&Focus::Content, true, fallback_bg, false),
            crate::configure::themed_color(|colors| colors.text.error)
        );
        assert_eq!(
            selected_matrix_bg_color(&Focus::Tree(LastFocused::Content), false, fallback_bg, true),
            fallback_bg
        );
        assert_eq!(
            selected_matrix_bg_color(&Focus::Attributes, true, fallback_bg, true),
            fallback_bg
        );
    }
}
