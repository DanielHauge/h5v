use hdf5_metno::{Dataset, Selection};
use ndarray::Array2;

use crate::{
    data::MatrixTable,
    error::AppError,
    h5f::{read_projected_values_2d, DatasetMeta},
    sprint_typedesc::MatrixRenderType,
    ui::state::{
        HeatmapLoadedPage, HeatmapRenderKey, HeatmapSegmentAxis, HeatmapSliceSummary,
        HeatmapViewport,
    },
};

use super::{
    build_heatmap_selection,
    render::{
        compute_heatmap_color_scale, compute_heatmap_histogram, compute_heatmap_stats,
        compute_region_selection, render_heatmap_image,
    },
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
    let ((row_start, row_end), (col_start, col_end)) = match key.segment_axis {
        Some(HeatmapSegmentAxis::Rows) => (
            (
                base_viewport.row_start + key.segment_start,
                (base_viewport.row_start + key.segment_start + key.segment_len)
                    .min(base_viewport.row_start + base_viewport.row_len),
            ),
            (
                base_viewport.col_start,
                base_viewport.col_start + base_viewport.col_len,
            ),
        ),
        Some(HeatmapSegmentAxis::Cols) => (
            (
                base_viewport.row_start,
                base_viewport.row_start + base_viewport.row_len,
            ),
            (
                base_viewport.col_start + key.segment_start,
                (base_viewport.col_start + key.segment_start + key.segment_len)
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
    let data = read_heatmap_table(ds, attr, selection)?;
    let transpose = key.selected_row > key.selected_col;
    let visible_rows = row_end.saturating_sub(row_start).max(1);
    let visible_cols = col_end.saturating_sub(col_start).max(1);
    let viewport_rows = usize::from(key.height).min(visible_rows);
    let viewport_cols = usize::from(key.width).min(visible_cols);
    let stats = compute_heatmap_stats(&data, transpose, visible_rows, visible_cols);
    let color_scale = compute_heatmap_color_scale(
        &data,
        transpose,
        visible_rows,
        visible_cols,
        key.settings.range,
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
        viewport_rows,
        viewport_cols,
        None,
        row_start,
        col_start,
        key.settings.invert_y,
        key.settings.invert_x,
    );
    let region = key.selected_cells.map(|selected_cells| {
        compute_region_selection(
            &data,
            transpose,
            visible_rows,
            visible_cols,
            viewport_rows,
            viewport_cols,
            Some(selected_cells),
            row_start,
            col_start,
            key.settings.invert_y,
            key.settings.invert_x,
        )
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
        viewport_rows,
        viewport_cols,
        key.selected_cells,
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
    })
}

fn read_heatmap_table(
    ds: &Dataset,
    attr: &DatasetMeta,
    selection: Selection,
) -> Result<Array2<f64>, AppError> {
    match attr.matrixable {
        Some(MatrixRenderType::Float64) => {
            if attr.is_compound_leaf() {
                Ok(read_projected_values_2d::<f64>(ds, attr, selection)?)
            } else {
                Ok(ds.matrix_table::<f64>(selection)?.data)
            }
        }
        Some(MatrixRenderType::Uint64) => {
            if attr.is_compound_leaf() {
                Ok(read_projected_values_2d::<u64>(ds, attr, selection)?.mapv(|v| v as f64))
            } else {
                Ok(ds.matrix_table::<u64>(selection)?.data.mapv(|v| v as f64))
            }
        }
        Some(MatrixRenderType::Int64) => {
            if attr.is_compound_leaf() {
                Ok(read_projected_values_2d::<i64>(ds, attr, selection)?.mapv(|v| v as f64))
            } else {
                Ok(ds.matrix_table::<i64>(selection)?.data.mapv(|v| v as f64))
            }
        }
        _ => Err(AppError::DrawingError(
            "Heatmap mode currently supports numeric datasets and numeric compound leaves"
                .to_string(),
        )),
    }
}
