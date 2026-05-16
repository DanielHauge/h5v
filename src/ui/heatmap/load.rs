use hdf5_metno::Dataset;
use ndarray::Array2;

use crate::{
    data::MatrixTable,
    error::AppError,
    h5f::{read_projected_values_2d, DatasetMeta},
    ui::render::MatrixRenderType,
    ui::state::{
        HeatmapLineProfile, HeatmapLineSelection, HeatmapLoadedPage, HeatmapPageAxis,
        HeatmapProfileSample, HeatmapRenderKey, HeatmapSliceSummary, HeatmapViewport,
    },
};

use super::{
    build_heatmap_selection,
    render::{
        compute_heatmap_histogram, compute_heatmap_metrics, compute_region_selection,
        render_heatmap_image,
    },
    HeatmapNumber,
};

pub(super) fn build_heatmap_page(
    ds: &Dataset,
    attr: &DatasetMeta,
    key: &HeatmapRenderKey,
) -> Result<HeatmapLoadedPage, AppError> {
    let source_rows = attr.shape[key.selected_row];
    let source_cols = attr.shape[key.selected_col];
    let base_viewport = key.viewport.unwrap_or(HeatmapViewport {
        row_start: 0,
        row_len: source_rows.max(1),
        col_start: 0,
        col_len: source_cols.max(1),
    });
    let ((row_start, row_end), (col_start, col_end)) = match key.page_axis {
        Some(HeatmapPageAxis::Rows) => (
            (
                base_viewport.row_start + key.page_start,
                (base_viewport.row_start + key.page_start + key.page_len)
                    .min(base_viewport.row_start + base_viewport.row_len),
            ),
            (
                base_viewport.col_start,
                base_viewport.col_start + base_viewport.col_len,
            ),
        ),
        Some(HeatmapPageAxis::Cols) => (
            (
                base_viewport.row_start,
                base_viewport.row_start + base_viewport.row_len,
            ),
            (
                base_viewport.col_start + key.page_start,
                (base_viewport.col_start + key.page_start + key.page_len)
                    .min(base_viewport.col_start + base_viewport.col_len),
            ),
        ),
        None => (
            (
                base_viewport.row_start,
                base_viewport.row_start + base_viewport.row_len,
            ),
            (
                base_viewport.col_start,
                base_viewport.col_start + base_viewport.col_len,
            ),
        ),
    };
    let selection = build_heatmap_selection(
        key.selected_row,
        key.selected_col,
        &key.selected_indexes,
        &attr.shape,
        (row_start, row_end),
        (col_start, col_end),
    );
    let transpose = key.selected_row > key.selected_col;
    match attr.matrixable {
        Some(MatrixRenderType::Float64) => {
            if attr.is_compound_leaf() {
                build_heatmap_page_from_data(
                    read_projected_values_2d::<f64>(ds, attr, selection)?,
                    attr,
                    key,
                    row_start,
                    row_end,
                    col_start,
                    col_end,
                    transpose,
                )
            } else {
                build_heatmap_page_from_data(
                    ds.matrix_table::<f64>(selection)?.data,
                    attr,
                    key,
                    row_start,
                    row_end,
                    col_start,
                    col_end,
                    transpose,
                )
            }
        }
        Some(MatrixRenderType::Uint64) => {
            if attr.is_compound_leaf() {
                build_heatmap_page_from_data(
                    read_projected_values_2d::<u64>(ds, attr, selection)?,
                    attr,
                    key,
                    row_start,
                    row_end,
                    col_start,
                    col_end,
                    transpose,
                )
            } else {
                build_heatmap_page_from_data(
                    ds.matrix_table::<u64>(selection)?.data,
                    attr,
                    key,
                    row_start,
                    row_end,
                    col_start,
                    col_end,
                    transpose,
                )
            }
        }
        Some(MatrixRenderType::Int64) => {
            if attr.is_compound_leaf() {
                build_heatmap_page_from_data(
                    read_projected_values_2d::<i64>(ds, attr, selection)?,
                    attr,
                    key,
                    row_start,
                    row_end,
                    col_start,
                    col_end,
                    transpose,
                )
            } else {
                build_heatmap_page_from_data(
                    ds.matrix_table::<i64>(selection)?.data,
                    attr,
                    key,
                    row_start,
                    row_end,
                    col_start,
                    col_end,
                    transpose,
                )
            }
        }
        _ => Err(AppError::DrawingError(
            "Heatmap mode currently supports numeric datasets and numeric compound leaves"
                .to_string(),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_heatmap_page_from_data<T: HeatmapNumber>(
    data: Array2<T>,
    attr: &DatasetMeta,
    key: &HeatmapRenderKey,
    row_start: usize,
    row_end: usize,
    col_start: usize,
    col_end: usize,
    transpose: bool,
) -> Result<HeatmapLoadedPage, AppError> {
    let visible_rows = row_end.saturating_sub(row_start).max(1);
    let visible_cols = col_end.saturating_sub(col_start).max(1);
    let (stats, color_scale) = compute_heatmap_metrics(
        &data,
        attr,
        transpose,
        visible_rows,
        visible_cols,
        &key.settings.range,
    );
    let histogram = compute_heatmap_histogram(
        &data,
        transpose,
        visible_rows,
        visible_cols,
        color_scale,
        24,
    );
    let viewport_region = compute_region_selection(
        &data,
        transpose,
        visible_rows,
        visible_cols,
        None,
        row_start,
        col_start,
    );
    let region = key.selected_cells.map(|selected_cells| {
        compute_region_selection(
            &data,
            transpose,
            visible_rows,
            visible_cols,
            Some(selected_cells),
            row_start,
            col_start,
        )
    });
    let line_profile = key.line_selection.map(|line_selection| {
        compute_line_profile(&data, transpose, line_selection, row_start, col_start)
    });
    let pixel_width = u32::from(key.width.max(1)) * u32::from(key.cell_width.max(1));
    let pixel_height = u32::from(key.height.max(1)) * u32::from(key.cell_height.max(1));
    let mut buffer = vec![0; (pixel_width * pixel_height * 3) as usize];
    render_heatmap_image(
        &mut buffer,
        pixel_width,
        pixel_height,
        &data,
        transpose,
        row_start,
        col_start,
        visible_rows,
        visible_cols,
        key.selected_cells,
        key.line_selection,
        color_scale,
        &key.settings,
    )?;
    Ok(HeatmapLoadedPage {
        key: key.clone(),
        pixel_width,
        pixel_height,
        rgb_bytes: buffer,
        slice_summary: HeatmapSliceSummary {
            min: stats.min,
            max: stats.max,
            has_finite: stats.has_finite,
        },
        legend_summary: crate::ui::state::HeatmapLegendSummary {
            min: color_scale.min,
            max: color_scale.max,
            has_finite: color_scale.has_finite,
            histogram,
        },
        viewport_selection: viewport_region,
        selection: region,
        line_profile,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_line_profile<T: HeatmapNumber>(
    data: &Array2<T>,
    transpose: bool,
    line_selection: HeatmapLineSelection,
    y_offset: usize,
    x_offset: usize,
) -> HeatmapLineProfile {
    let data_rows = if transpose {
        data.shape()[1]
    } else {
        data.shape()[0]
    };
    let data_cols = if transpose {
        data.shape()[0]
    } else {
        data.shape()[1]
    };
    let max_row = data_rows.saturating_sub(1);
    let max_col = data_cols.saturating_sub(1);
    let start_row = line_selection
        .start_row
        .saturating_sub(y_offset)
        .min(max_row);
    let start_col = line_selection
        .start_col
        .saturating_sub(x_offset)
        .min(max_col);
    let end_row = line_selection.end_row.saturating_sub(y_offset).min(max_row);
    let end_col = line_selection.end_col.saturating_sub(x_offset).min(max_col);
    let points = rasterize_line(start_row, start_col, end_row, end_col);

    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    let mut finite_count = 0usize;
    let mut samples = Vec::with_capacity(points.len());
    let mut distance = 0.0;
    let mut previous = None;

    for (row, col) in points.iter().copied() {
        if let Some((prev_row, prev_col)) = previous {
            let delta_row = row as f64 - prev_row as f64;
            let delta_col = col as f64 - prev_col as f64;
            distance += (delta_row * delta_row + delta_col * delta_col).sqrt();
        }
        previous = Some((row, col));

        let value = super::render::heatmap_value(data, transpose, row, col);
        samples.push(HeatmapProfileSample { distance, value });
        if value.is_finite() {
            min = min.min(value);
            max = max.max(value);
            sum += value;
            sum_sq += value * value;
            finite_count += 1;
        }
    }

    let mean = if finite_count == 0 {
        f64::NAN
    } else {
        sum / finite_count as f64
    };
    let stddev = if finite_count == 0 {
        f64::NAN
    } else {
        let variance = (sum_sq / finite_count as f64) - mean * mean;
        variance.max(0.0).sqrt()
    };
    let (min, max) = if finite_count == 0 {
        (f64::NAN, f64::NAN)
    } else {
        (min, max)
    };

    HeatmapLineProfile {
        start_x: line_selection.start_col,
        start_y: line_selection.start_row,
        end_x: line_selection.end_col,
        end_y: line_selection.end_row,
        sample_count: samples.len(),
        finite_count,
        min,
        max,
        mean,
        stddev,
        samples,
    }
}

fn rasterize_line(
    start_row: usize,
    start_col: usize,
    end_row: usize,
    end_col: usize,
) -> Vec<(usize, usize)> {
    let mut points = Vec::new();
    let mut x0 = start_col as isize;
    let mut y0 = start_row as isize;
    let x1 = end_col as isize;
    let y1 = end_row as isize;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;

    loop {
        points.push((y0.max(0) as usize, x0.max(0) as usize));
        if x0 == x1 && y0 == y1 {
            break;
        }
        let twice_error = 2 * error;
        if twice_error >= dy {
            error += dy;
            x0 += sx;
        }
        if twice_error <= dx {
            error += dx;
            y0 += sy;
        }
    }

    points
}
