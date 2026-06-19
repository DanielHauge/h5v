use std::fmt::Display;

use hdf5_metno::{
    types::{EnumMember, EnumType, IntSize},
    H5Type, Hyperslab, Selection, SliceOrIndex,
};
use ndarray::{Array1, Array2};
use ratatui::{
    layout::{Constraint, Layout, Offset, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{
    configure,
    data::{MatrixTable, MatrixValues},
    error::AppError,
    h5f::{
        read_opaque_values_1d, read_opaque_values_2d, read_projected_values_1d,
        read_projected_values_2d, read_selected_values_bytes, read_varlen_u8_matrix_table,
        read_varlen_u8_matrix_values, DatasetMeta, EnumRenderOverrides, H5FNode, ProjectionDecode,
        ResolvedOpenMode,
    },
    ui::{render::sprint_typedescriptor, state::Focus},
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

fn refresh_dataset_for_swmr(ds: &hdf5_metno::Dataset, state: &AppState) -> Result<(), AppError> {
    if state.resolved_open_mode == ResolvedOpenMode::ReadSwmr {
        ds.refresh()?;
    }
    Ok(())
}

fn normalize_matrix_axes(node: &mut H5FNode, shape: &[usize]) {
    node.sync_selection_rank(shape.len());
    let rank = shape.len();
    if rank == 0 {
        node.selected_dim = 0;
        node.selected_x = 0;
        node.selected_row = 0;
        node.selected_col = 0;
        return;
    }

    let max_index = rank.saturating_sub(1);
    node.selected_dim = node.selected_dim.min(max_index);
    node.selected_x = node.selected_x.min(max_index);
    node.selected_row = node.selected_row.min(max_index);
    node.selected_col = node.selected_col.min(max_index);

    if rank == 1 {
        node.selected_dim = 0;
        node.selected_x = 0;
        node.selected_row = 0;
        node.selected_col = 0;
        return;
    }

    let selectable_dims: Vec<usize> = shape
        .iter()
        .enumerate()
        .filter(|(_, len)| **len > 1)
        .map(|(idx, _)| idx)
        .collect();
    let fallback_row = selectable_dims.first().copied().unwrap_or(0);
    if !selectable_dims.contains(&node.selected_row) {
        node.selected_row = fallback_row;
    }
    if !selectable_dims.contains(&node.selected_col) || node.selected_col == node.selected_row {
        node.selected_col = selectable_dims
            .iter()
            .copied()
            .find(|dim| *dim != node.selected_row)
            .or_else(|| (0..rank).find(|dim| *dim != node.selected_row))
            .unwrap_or(node.selected_row);
    }
    if node.selected_dim >= rank
        || node.selected_dim == node.selected_row
        || node.selected_dim == node.selected_col
    {
        node.selected_dim = selectable_dims
            .iter()
            .copied()
            .find(|dim| *dim != node.selected_row && *dim != node.selected_col)
            .or_else(|| {
                (0..rank).find(|dim| *dim != node.selected_row && *dim != node.selected_col)
            })
            .unwrap_or(0);
    }
}

fn has_visible_matrix_cells(shape_len: usize, selection: MatrixSelection) -> bool {
    selection.rows > 0 && (shape_len <= 1 || selection.cols > 0)
}

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
    refresh_dataset_for_swmr(ds, state)?;
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
    refresh_dataset_for_swmr(ds, state)?;
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
    refresh_dataset_for_swmr(ds, state)?;
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
    refresh_dataset_for_swmr(ds, state)?;
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

pub fn render_compound_root_matrix(
    f: &mut Frame,
    area: &Rect,
    ds: &hdf5_metno::Dataset,
    attr: &DatasetMeta,
    node: &mut H5FNode,
    state: &mut AppState,
) -> Result<(), AppError> {
    refresh_dataset_for_swmr(ds, state)?;
    let Some(compound) = attr.current_compound_type() else {
        render_not_yet_implemented(f, area, "Compound root matrix metadata is unavailable");
        return Ok(());
    };
    let Some((row_dim, row_count)) = compound_root_matrix_axis(node, attr) else {
        render_not_yet_implemented(
            f,
            area,
            "Compound root matrix requires at least one non-singleton record axis",
        );
        return Ok(());
    };

    node.sync_selection_rank(attr.shape.len());
    node.selected_row = row_dim;
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 2,
    });
    let field_count = compound.fields.len();
    let matrix_area = if attr.shape.len() > 1 {
        let areas_split =
            Layout::vertical(vec![Constraint::Length(4), Constraint::Min(1)]).split(area_inner);
        let provisional_selection = visible_matrix_capacity(areas_split[1], row_count, field_count);
        let field_window =
            compound_root_field_window(state, field_count, provisional_selection.cols);
        render_compound_root_matrix_selector(
            f,
            &areas_split[0],
            node,
            attr,
            row_dim,
            field_window,
        )?;
        areas_split[1].inner(ratatui::layout::Margin {
            horizontal: 0,
            vertical: 1,
        })
    } else {
        area_inner
    };
    let matrix_selection = visible_matrix_capacity(matrix_area, row_count, field_count);
    let max_cols = matrix_selection.cols;
    let max_rows = matrix_selection.rows;
    state.matrix_view_state.rows_currently_available = max_rows;
    state.matrix_view_state.cols_currently_available = max_cols;

    if max_rows == 0 || max_cols == 0 || field_count == 0 {
        return Ok(());
    }

    let row_start = state
        .matrix_view_state
        .row_offset
        .min(row_count.saturating_sub(max_rows));
    let col_start = compound_root_field_window(state, field_count, max_cols).start;
    let selection = compound_root_matrix_selection(node, attr, row_dim, row_start, max_rows);
    let (bytes, _) = read_selected_values_bytes(ds, selection)?;
    let record_size = compound.size;
    if bytes.len() != max_rows * record_size {
        return Err(AppError::DrawingError(format!(
            "Compound root matrix byte size mismatch: expected {} bytes, got {}",
            max_rows * record_size,
            bytes.len()
        )));
    }
    let records = bytes.chunks_exact(record_size).collect::<Vec<_>>();

    let mut rows_area_constraints = Vec::with_capacity(max_rows + 1);
    (0..max_rows).for_each(|_| rows_area_constraints.push(Constraint::Length(1)));
    let rows_areas = Layout::vertical(rows_area_constraints).split(matrix_area);

    let mut col_constraint = Vec::with_capacity(max_cols + 1);
    col_constraint.push(Constraint::Length(15));
    (0..max_cols).for_each(|_| col_constraint.push(Constraint::Fill(1)));
    let col_header_areas = Layout::horizontal(col_constraint.clone()).split(rows_areas[0]);

    for col in 0..max_cols {
        let field = &compound.fields[col_start + col];
        let col_area = col_header_areas[col + 1];
        let mut header = Line::from(Span::styled(
            field.name.clone(),
            Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)),
        ))
        .centered();
        if configure::prefers_strong_text() {
            header = header.bold();
        }
        f.render_widget(header, col_area.offset(Offset { x: 0, y: -1 }));
    }

    for i in 0..max_rows {
        let row_area = rows_areas[i];
        let col_areas = Layout::horizontal(col_constraint.clone()).split(row_area);
        let idx_area = col_areas[0];
        state.ui_layout.matrix_rows.push(MatrixRowHitbox {
            area: idx_area,
            row: i,
        });
        let row_index = row_start + i;
        let mut idx_line = Line::from(format!("{row_index}"))
            .style(Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)))
            .left_aligned();
        if configure::prefers_strong_text() {
            idx_line = idx_line.bold();
        }
        f.render_widget(idx_line, idx_area);

        let record = records[i];
        for j in 0..max_cols {
            let val_area = col_areas[j + 1];
            state.ui_layout.matrix_cells.push(MatrixCellHitbox {
                area: val_area,
                row: i,
                col: j,
            });
            let field = &compound.fields[col_start + j];
            let val_bg_color = match (
                (i + state.matrix_view_state.row_offset).is_multiple_of(2),
                (j + state.matrix_view_state.col_offset).is_multiple_of(2),
            ) {
                (true, true) => configure::themed_color(|colors| colors.surface.bg_val3),
                (true, false) => configure::themed_color(|colors| colors.surface.bg_val4),
                (false, true) => configure::themed_color(|colors| colors.surface.bg_val1),
                (false, false) => configure::themed_color(|colors| colors.surface.bg_val2),
            };
            let val_bg_color = if i == state.matrix_view_state.cursor_row
                && j == state.matrix_view_state.cursor_col
            {
                selected_matrix_bg_color(&state.focus, state.copying, val_bg_color, true)
            } else {
                val_bg_color
            };
            let value = compound_root_matrix_field_text_from_record(record, field)?;
            render_centered_matrix_cell(
                f,
                val_area,
                DefaultMatrixResultRenderIntercept.render_as_line(&value),
                val_bg_color,
            );
        }
    }

    Ok(())
}

pub(crate) fn compound_root_matrix_cell_text(
    dataset: &hdf5_metno::Dataset,
    meta: &DatasetMeta,
    row_dim: usize,
    row_index: usize,
    field_index: usize,
    selected_indexes: &[usize],
) -> Result<String, AppError> {
    let Some((row_dim, row_count)) = compound_root_matrix_axis_for_row(meta, row_dim) else {
        return Err(AppError::DrawingError(
            "Compound root matrix copy requires at least one non-singleton record axis".to_string(),
        ));
    };
    let compound = meta.current_compound_type().ok_or_else(|| {
        AppError::DrawingError("Compound root matrix metadata is unavailable".to_string())
    })?;
    let field = compound.fields.get(field_index).ok_or_else(|| {
        AppError::DrawingError(format!(
            "Compound root field column {field_index} is out of bounds for {} fields",
            compound.fields.len()
        ))
    })?;
    if row_index >= row_count {
        return Err(AppError::DrawingError(format!(
            "Compound root row {row_index} is out of bounds for {row_count} rows"
        )));
    }

    let mut slice = Vec::with_capacity(meta.shape.len());
    for dim in 0..meta.shape.len() {
        if dim == row_dim {
            slice.push(SliceOrIndex::Index(row_index));
        } else {
            slice.push(SliceOrIndex::Index(
                selected_indexes.get(dim).copied().unwrap_or_default(),
            ));
        }
    }
    let (bytes, _) =
        read_selected_values_bytes(dataset, Selection::Hyperslab(Hyperslab::from(slice)))?;
    if bytes.len() != compound.size {
        return Err(AppError::DrawingError(format!(
            "Compound root cell read returned {} bytes, expected {}",
            bytes.len(),
            compound.size
        )));
    }
    compound_root_matrix_field_text_from_record(&bytes, field)
}

#[derive(Clone, Copy)]
struct CompoundFieldWindow {
    start: usize,
    end: usize,
}

fn compound_root_matrix_axis(node: &mut H5FNode, attr: &DatasetMeta) -> Option<(usize, usize)> {
    let selectable_dims = compound_root_selectable_dims(attr);
    let row_dim = if selectable_dims.contains(&node.selected_row) {
        node.selected_row
    } else {
        *selectable_dims.first()?
    };
    node.selected_row = row_dim;
    if node.selected_dim == row_dim {
        node.selected_dim = selectable_dims
            .iter()
            .copied()
            .find(|dim| *dim != row_dim)
            .unwrap_or(0);
    }
    compound_root_matrix_axis_for_row(attr, row_dim)
}

fn compound_root_matrix_axis_for_row(attr: &DatasetMeta, row_dim: usize) -> Option<(usize, usize)> {
    if !attr.supports_compound_root_matrix() {
        return None;
    }
    attr.shape
        .get(row_dim)
        .copied()
        .filter(|len| *len > 1)
        .map(|len| (row_dim, len))
}

fn compound_root_selectable_dims(attr: &DatasetMeta) -> Vec<usize> {
    attr.shape
        .iter()
        .enumerate()
        .filter(|(_, len)| **len > 1)
        .map(|(dim, _)| dim)
        .collect()
}

fn compound_root_field_window(
    state: &AppState,
    field_count: usize,
    visible_cols: usize,
) -> CompoundFieldWindow {
    let start = state
        .matrix_view_state
        .col_offset
        .min(field_count.saturating_sub(visible_cols));
    let end = (start + visible_cols).min(field_count);
    CompoundFieldWindow { start, end }
}

fn render_compound_root_matrix_selector(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    attr: &DatasetMeta,
    row_dim: usize,
    field_window: CompoundFieldWindow,
) -> Result<(), AppError> {
    let block = Block::default()
        .title("Compound matrix")
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.surface.panel_title))
                .bold(),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(configure::themed_color(|colors| {
            colors.surface.panel_border
        })));
    f.render_widget(block, *area);
    let inner = area.inner(ratatui::layout::Margin {
        horizontal: 1,
        vertical: 1,
    });

    let mut lines = Vec::with_capacity(2);
    let shape_summary = attr
        .shape
        .iter()
        .enumerate()
        .map(|(dim, len)| {
            if dim == row_dim {
                format!("[{len}]")
            } else {
                len.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" | ");
    lines.push(Line::from(vec![
        Span::styled(
            "shape ",
            Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)),
        ),
        Span::styled(
            shape_summary,
            Style::default().fg(configure::themed_color(|colors| colors.text.primary)),
        ),
        Span::raw("  "),
        Span::styled(
            "row dim ",
            Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)),
        ),
        Span::styled(
            row_dim.to_string(),
            Style::default()
                .fg(configure::themed_color(|colors| colors.accent.selected_dim))
                .bold(),
        ),
    ]));

    let fixed_dims = attr
        .shape
        .iter()
        .enumerate()
        .filter(|(dim, _)| *dim != row_dim)
        .map(|(dim, len)| {
            let value = node.selected_indexes.get(dim).copied().unwrap_or_default();
            format!("d{dim}={}/{}", value, len.saturating_sub(1))
        })
        .collect::<Vec<_>>()
        .join(" ");
    let field_count = attr.compound_root_matrix_column_count().unwrap_or_default();
    let field_range = if field_window.end > field_window.start {
        format!(
            "{}..{}/{}",
            field_window.start,
            field_window.end.saturating_sub(1),
            field_count
        )
    } else {
        format!("0/{}", field_count)
    };
    lines.push(Line::from(vec![
        Span::styled(
            "fields ",
            Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)),
        ),
        Span::styled(
            field_range,
            Style::default().fg(configure::themed_color(|colors| colors.text.primary)),
        ),
        Span::raw("  "),
        Span::styled(
            "fixed ",
            Style::default().fg(configure::themed_color(|colors| colors.text.type_desc)),
        ),
        Span::styled(
            if fixed_dims.is_empty() {
                "<none>".to_string()
            } else {
                fixed_dims
            },
            Style::default().fg(configure::themed_color(|colors| colors.text.primary)),
        ),
    ]));
    f.render_widget(Paragraph::new(lines), inner);
    Ok(())
}

fn compound_root_matrix_selection(
    node: &mut H5FNode,
    attr: &DatasetMeta,
    row_dim: usize,
    row_start: usize,
    rows: usize,
) -> Selection {
    let end = (row_start + rows).min(attr.shape[row_dim]);
    let mut slice = Vec::with_capacity(attr.shape.len());
    for dim in 0..attr.shape.len() {
        if dim == row_dim {
            slice.push(SliceOrIndex::SliceTo {
                start: row_start,
                step: 1,
                end,
                block: 1,
            });
        } else {
            slice.push(SliceOrIndex::Index(
                node.selected_indexes.get(dim).copied().unwrap_or_default(),
            ));
        }
    }
    Selection::Hyperslab(Hyperslab::from(slice))
}

fn compound_root_matrix_field_text_from_record(
    record: &[u8],
    field: &hdf5_metno::types::CompoundField,
) -> Result<String, AppError> {
    let end = field.offset + field.ty.size();
    let field_bytes = record.get(field.offset..end).ok_or_else(|| {
        AppError::DrawingError(format!(
            "Compound field '{}' exceeded record bounds",
            field.name
        ))
    })?;
    format_compound_matrix_value(&field.ty, field_bytes)
}

fn format_compound_matrix_value(
    type_desc: &hdf5_metno::types::TypeDescriptor,
    bytes: &[u8],
) -> Result<String, AppError> {
    match type_desc {
        hdf5_metno::types::TypeDescriptor::Compound(compound) => {
            let mut fields = Vec::with_capacity(compound.fields.len());
            for field in &compound.fields {
                let value = compound_root_matrix_field_text_from_record(bytes, field)?;
                fields.push(format!("{}: {value}", field.name));
            }
            Ok(format!("{{{}}}", fields.join(", ")))
        }
        hdf5_metno::types::TypeDescriptor::FixedArray(inner, size) => {
            let inner_size = inner.size();
            if bytes.len() != inner_size * size {
                return Err(AppError::DrawingError(format!(
                    "Fixed array value size mismatch: expected {}, got {}",
                    inner_size * size,
                    bytes.len()
                )));
            }
            let mut values = Vec::with_capacity(*size);
            for chunk in bytes.chunks_exact(inner_size).take(*size) {
                values.push(format_compound_matrix_value(inner.as_ref(), chunk)?);
            }
            Ok(format!("[{}]", values.join(", ")))
        }
        hdf5_metno::types::TypeDescriptor::VarLenArray(_)
        | hdf5_metno::types::TypeDescriptor::Reference(_) => Ok(sprint_typedescriptor(type_desc)),
        _ => {
            let mut owned = bytes.to_vec();
            <String as ProjectionDecode>::decode_scalar_buffer(type_desc, &mut owned)
        }
    }
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
    normalize_matrix_axes(node, &attr.shape);

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
        render_dim_selector(
            f,
            &areas_split[0],
            node,
            &attr.shape,
            super::dims::RenderDimSelectorOptions {
                row_columns: true,
                page_info: None,
                panel_title: "Slice selection",
                detail_lines: None,
            },
        )?;
        areas_split[1].inner(ratatui::layout::Margin {
            horizontal: 0,
            vertical: 1,
        })
    } else {
        area_inner
    };
    let col_ds_len = attr.shape.get(node.selected_col).copied().ok_or_else(|| {
        AppError::DrawingError(format!(
            "Matrix column axis {} is out of bounds for rank {}",
            node.selected_col,
            attr.shape.len()
        ))
    })?;
    let row_ds_len = attr.shape.get(node.selected_row).copied().ok_or_else(|| {
        AppError::DrawingError(format!(
            "Matrix row axis {} is out of bounds for rank {}",
            node.selected_row,
            attr.shape.len()
        ))
    })?;
    let matrix_selection = visible_matrix_capacity(matrix_area, row_ds_len, col_ds_len);
    let max_cols = matrix_selection.cols;
    let max_rows = matrix_selection.rows;
    state.matrix_view_state.rows_currently_available = max_rows;
    state.matrix_view_state.cols_currently_available = max_cols;
    if !has_visible_matrix_cells(shape_len, matrix_selection) {
        return Ok(());
    }
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
    use hdf5_metno::types::{
        CompoundField, CompoundType, EnumMember, EnumType, IntSize, TypeDescriptor,
    };
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
        let first_symbol = crate::configure::configured_symbol(|symbols| symbols.chart.r#enum[0]);
        let second_symbol = crate::configure::configured_symbol(|symbols| symbols.chart.r#enum[1]);
        assert_eq!(
            renderer.render_as_line(&1).to_string(),
            format!("{first_symbol} Red")
        );
        assert_eq!(
            renderer.render_as_span(&2).content,
            format!("{second_symbol} Green")
        );
    }

    #[test]
    fn enum_renderer_uses_member_overrides_when_present() {
        let overrides = EnumRenderOverrides {
            colors: vec![Some(Color::Green), None],
            symbols: vec![Some("✓".to_string()), None],
        };
        let renderer = EnumRenderer::with_overrides(sample_enum(), Some(&overrides));
        let second_symbol = crate::configure::configured_symbol(|symbols| symbols.chart.r#enum[1]);
        assert_eq!(renderer.render_as_line(&1).to_string(), "✓ Red");
        assert_eq!(
            renderer.render_as_span(&2).content,
            format!("{second_symbol} Green")
        );
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
    fn invisible_matrix_layout_is_skipped_for_zero_rows_or_cols() {
        assert!(!has_visible_matrix_cells(
            1,
            MatrixSelection { rows: 0, cols: 1 }
        ));
        assert!(!has_visible_matrix_cells(
            2,
            MatrixSelection { rows: 1, cols: 0 }
        ));
        assert!(!has_visible_matrix_cells(
            2,
            MatrixSelection { rows: 0, cols: 4 }
        ));
        assert!(has_visible_matrix_cells(
            2,
            MatrixSelection { rows: 1, cols: 1 }
        ));
    }

    #[test]
    fn normalize_matrix_axes_clamps_invalid_indices() {
        let mut node = H5FNode::new(crate::h5f::Node::Broken("broken".to_string()));
        node.selected_dim = 9;
        node.selected_x = 9;
        node.selected_row = 9;
        node.selected_col = 9;
        node.selected_indexes = vec![4, 5, 6];

        normalize_matrix_axes(&mut node, &[3, 1, 5]);

        assert_eq!(node.selected_row, 2);
        assert_eq!(node.selected_col, 0);
        assert_eq!(node.selected_dim, 1);
        assert_eq!(node.selected_x, 2);
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

    #[test]
    fn compound_matrix_value_formats_nested_fields() {
        let type_desc = TypeDescriptor::Compound(CompoundType {
            fields: vec![
                CompoundField::new("count", TypeDescriptor::Unsigned(IntSize::U2), 0, 0),
                CompoundField::new(
                    "window",
                    TypeDescriptor::FixedArray(Box::new(TypeDescriptor::Integer(IntSize::U2)), 2),
                    2,
                    1,
                ),
            ],
            size: 6,
        });
        let bytes = vec![7, 0, 1, 0, 2, 0];

        let rendered =
            format_compound_matrix_value(&type_desc, &bytes).expect("failed rendering compound");
        assert_eq!(rendered, "{count: 7, window: [1, 2]}");
    }
}
