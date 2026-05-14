use hdf5_metno::types::{FloatSize, IntSize, TypeDescriptor};
use ndarray::Array2;
use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea, Rectangle as PlotRectangle},
    style::{RGBColor, ShapeStyle},
};
use ratatui::layout::Rect;

use crate::{
    configure,
    error::AppError,
    h5f::DatasetMeta,
    ui::state::{
        AppState, HeatmapColormap, HeatmapNormalization, HeatmapRangeBound, HeatmapRangeMode,
        HeatmapRegionSelection, HeatmapSelectedCells, HeatmapSettings,
    },
};

use super::HeatmapNumber;
use crate::ui::state::{MatrixCellHitbox, MatrixRowHitbox};

#[derive(Clone, Copy)]
pub(super) struct HeatmapStats {
    pub(super) min: f64,
    pub(super) max: f64,
    pub(super) has_finite: bool,
}

#[derive(Clone, Copy)]
pub(super) struct HeatmapColorScale {
    pub(super) min: f64,
    pub(super) max: f64,
    pub(super) has_finite: bool,
}

pub(super) fn compute_heatmap_metrics<T: HeatmapNumber>(
    data: &Array2<T>,
    attr: &DatasetMeta,
    transpose: bool,
    rows: usize,
    cols: usize,
    range_mode: &HeatmapRangeMode,
) -> (HeatmapStats, HeatmapColorScale) {
    let scan = scan_heatmap_values(data, transpose, rows, cols);
    (
        HeatmapStats {
            min: scan.min,
            max: scan.max,
            has_finite: scan.has_finite,
        },
        compute_heatmap_color_scale_from_scan(data, attr, transpose, rows, cols, range_mode, scan),
    )
}

#[derive(Clone, Copy)]
struct HeatmapScan {
    min: f64,
    max: f64,
    has_finite: bool,
    count: usize,
    sum: f64,
    sum_sq: f64,
}

pub(super) fn populate_viewport_hitboxes(
    state: &mut AppState<'_>,
    heatmap_inner: Rect,
    viewport_rows: usize,
    viewport_cols: usize,
) {
    state.ui_layout.matrix_rows.clear();
    state.ui_layout.matrix_cells.clear();

    for row in 0..viewport_rows {
        let y0 =
            heatmap_inner.y + ((row * usize::from(heatmap_inner.height)) / viewport_rows) as u16;
        let y1 = heatmap_inner.y
            + (((row + 1) * usize::from(heatmap_inner.height)) / viewport_rows) as u16;
        state.ui_layout.matrix_rows.push(MatrixRowHitbox {
            area: Rect {
                x: heatmap_inner.x,
                y: y0,
                width: heatmap_inner.width,
                height: y1.saturating_sub(y0).max(1),
            },
            row,
        });

        for col in 0..viewport_cols {
            let x0 =
                heatmap_inner.x + ((col * usize::from(heatmap_inner.width)) / viewport_cols) as u16;
            let x1 = heatmap_inner.x
                + (((col + 1) * usize::from(heatmap_inner.width)) / viewport_cols) as u16;
            state.ui_layout.matrix_cells.push(MatrixCellHitbox {
                area: Rect {
                    x: x0,
                    y: y0,
                    width: x1.saturating_sub(x0).max(1),
                    height: y1.saturating_sub(y0).max(1),
                },
                row,
                col,
            });
        }
    }
}

#[cfg(test)]
pub(super) fn compute_heatmap_color_scale<T: HeatmapNumber>(
    data: &Array2<T>,
    attr: &DatasetMeta,
    transpose: bool,
    rows: usize,
    cols: usize,
    range_mode: &HeatmapRangeMode,
) -> HeatmapColorScale {
    let scan = scan_heatmap_values(data, transpose, rows, cols);
    compute_heatmap_color_scale_from_scan(data, attr, transpose, rows, cols, range_mode, scan)
}

fn compute_heatmap_color_scale_from_scan<T: HeatmapNumber>(
    data: &Array2<T>,
    attr: &DatasetMeta,
    transpose: bool,
    rows: usize,
    cols: usize,
    range_mode: &HeatmapRangeMode,
    scan: HeatmapScan,
) -> HeatmapColorScale {
    if !scan.has_finite {
        return HeatmapColorScale {
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            has_finite: false,
        };
    }
    let (min, max) = match range_mode {
        HeatmapRangeMode::Auto => (scan.min, scan.max),
        HeatmapRangeMode::MinMax => {
            type_descriptor_range(&attr.type_descriptor).unwrap_or((scan.min, scan.max))
        }
        HeatmapRangeMode::Percentile {
            lower_bps,
            upper_bps,
        }
        | HeatmapRangeMode::Winsorized {
            lower_bps,
            upper_bps,
        } => {
            let values = sorted_heatmap_finite_values(data, transpose, rows, cols);
            (
                percentile_from_sorted(&values, *lower_bps),
                percentile_from_sorted(&values, *upper_bps),
            )
        }
        HeatmapRangeMode::SigmaClip { sigma_milli } => {
            let mean = scan.sum / scan.count as f64;
            let variance = (scan.sum_sq / scan.count as f64) - mean * mean;
            let stddev = variance.max(0.0).sqrt();
            let sigma = f64::from(*sigma_milli) / 1000.0;
            (mean - sigma * stddev, mean + sigma * stddev)
        }
        HeatmapRangeMode::Custom(mode)
            if matches!(mode.lower, HeatmapRangeBound::Exact(_))
                && matches!(mode.upper, HeatmapRangeBound::Exact(_)) =>
        {
            (
                resolve_range_bound(&[], mode.lower),
                resolve_range_bound(&[], mode.upper),
            )
        }
        HeatmapRangeMode::Custom(mode) => {
            let values = sorted_heatmap_finite_values(data, transpose, rows, cols);
            (
                resolve_range_bound(&values, mode.lower),
                resolve_range_bound(&values, mode.upper),
            )
        }
    };
    let (min, max) = if min <= max { (min, max) } else { (max, min) };
    HeatmapColorScale {
        min,
        max,
        has_finite: true,
    }
}

fn type_descriptor_range(type_descriptor: &TypeDescriptor) -> Option<(f64, f64)> {
    match type_descriptor {
        TypeDescriptor::Integer(size) => Some(match size {
            IntSize::U1 => (i8::MIN as f64, i8::MAX as f64),
            IntSize::U2 => (i16::MIN as f64, i16::MAX as f64),
            IntSize::U4 => (i32::MIN as f64, i32::MAX as f64),
            IntSize::U8 => (i64::MIN as f64, i64::MAX as f64),
        }),
        TypeDescriptor::Unsigned(size) => Some(match size {
            IntSize::U1 => (u8::MIN as f64, u8::MAX as f64),
            IntSize::U2 => (u16::MIN as f64, u16::MAX as f64),
            IntSize::U4 => (u32::MIN as f64, u32::MAX as f64),
            IntSize::U8 => (0.0, u64::MAX as f64),
        }),
        TypeDescriptor::Float(size) => Some(match size {
            FloatSize::U4 => (f32::MIN as f64, f32::MAX as f64),
            FloatSize::U8 => (f64::MIN, f64::MAX),
        }),
        TypeDescriptor::Boolean => Some((0.0, 1.0)),
        _ => None,
    }
}

fn percentile_from_sorted(values: &[f64], basis_points: u16) -> f64 {
    let last = values.len().saturating_sub(1);
    let percentile = f64::from(basis_points) / 10_000.0;
    let index = ((last as f64) * percentile).round() as usize;
    values[index.min(last)]
}

fn resolve_range_bound(values: &[f64], bound: crate::ui::state::HeatmapRangeBound) -> f64 {
    match bound {
        crate::ui::state::HeatmapRangeBound::Exact(value) => value.to_f64(),
        crate::ui::state::HeatmapRangeBound::Percentile(basis_points) => {
            percentile_from_sorted(values, basis_points)
        }
    }
}

fn scan_heatmap_values<T: HeatmapNumber>(
    data: &Array2<T>,
    transpose: bool,
    rows: usize,
    cols: usize,
) -> HeatmapScan {
    let mut scan = HeatmapScan {
        min: f64::INFINITY,
        max: f64::NEG_INFINITY,
        has_finite: false,
        count: 0,
        sum: 0.0,
        sum_sq: 0.0,
    };
    for row in 0..rows {
        for col in 0..cols {
            let value = heatmap_value(data, transpose, row, col);
            if !value.is_finite() {
                continue;
            }
            scan.has_finite = true;
            scan.min = scan.min.min(value);
            scan.max = scan.max.max(value);
            scan.count += 1;
            scan.sum += value;
            scan.sum_sq += value * value;
        }
    }
    scan
}

fn sorted_heatmap_finite_values<T: HeatmapNumber>(
    data: &Array2<T>,
    transpose: bool,
    rows: usize,
    cols: usize,
) -> Vec<f64> {
    let mut values = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let value = heatmap_value(data, transpose, row, col);
            if value.is_finite() {
                values.push(value);
            }
        }
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    values
}

pub(super) fn compute_heatmap_histogram<T: HeatmapNumber>(
    data: &Array2<T>,
    transpose: bool,
    rows: usize,
    cols: usize,
    scale: HeatmapColorScale,
    bins: usize,
) -> Vec<usize> {
    if !scale.has_finite || bins == 0 {
        return vec![0; bins];
    }
    let mut histogram = vec![0usize; bins];
    let span = (scale.max - scale.min).max(f64::EPSILON);
    for row in 0..rows {
        for col in 0..cols {
            let value = heatmap_value(data, transpose, row, col);
            if !value.is_finite() {
                continue;
            }
            let normalized = ((value - scale.min) / span).clamp(0.0, 1.0);
            let idx = ((normalized * (bins.saturating_sub(1)) as f64).round() as usize)
                .min(bins.saturating_sub(1));
            histogram[idx] += 1;
        }
    }
    histogram
}

fn heatmap_value<T: HeatmapNumber>(
    data: &Array2<T>,
    transpose: bool,
    row: usize,
    col: usize,
) -> f64 {
    let idx = if transpose { (col, row) } else { (row, col) };
    data.get(idx)
        .map(|value| value.to_f64())
        .unwrap_or(f64::NAN)
}

pub(super) fn viewport_partition(total: usize, cells: usize, index: usize) -> (usize, usize) {
    let start = (index * total) / cells;
    let mut end = ((index + 1) * total) / cells;
    if end <= start {
        end = (start + 1).min(total);
    }
    (start, end)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_region_selection<T: HeatmapNumber>(
    data: &Array2<T>,
    transpose: bool,
    rows: usize,
    cols: usize,
    viewport_rows: usize,
    viewport_cols: usize,
    selected_cells: Option<HeatmapSelectedCells>,
    y_offset: usize,
    x_offset: usize,
    invert_y: bool,
    invert_x: bool,
) -> HeatmapRegionSelection {
    let (display_y0, display_y1, display_x0, display_x1) = if let Some(selected) = selected_cells {
        let (display_y0, _) = viewport_partition(rows, viewport_rows, selected.row_start);
        let (_, display_y1) = viewport_partition(rows, viewport_rows, selected.row_end);
        let (display_x0, _) = viewport_partition(cols, viewport_cols, selected.col_start);
        let (_, display_x1) = viewport_partition(cols, viewport_cols, selected.col_end);
        (display_y0, display_y1, display_x0, display_x1)
    } else {
        (0, rows, 0, cols)
    };
    let (y0, y1) = if invert_y {
        (
            rows.saturating_sub(display_y1),
            rows.saturating_sub(display_y0),
        )
    } else {
        (display_y0, display_y1)
    };
    let (x0, x1) = if invert_x {
        (
            cols.saturating_sub(display_x1),
            cols.saturating_sub(display_x0),
        )
    } else {
        (display_x0, display_x1)
    };

    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    let mut count = 0usize;
    for row in y0..y1 {
        for col in x0..x1 {
            let value = heatmap_value(data, transpose, row, col);
            if !value.is_finite() {
                continue;
            }
            min = min.min(value);
            max = max.max(value);
            sum += value;
            sum_sq += value * value;
            count += 1;
        }
    }
    let mean = if count == 0 {
        f64::NAN
    } else {
        sum / count as f64
    };
    let stddev = if count == 0 {
        f64::NAN
    } else {
        let variance = (sum_sq / count as f64) - mean * mean;
        variance.max(0.0).sqrt()
    };
    HeatmapRegionSelection {
        x: x_offset + x0,
        y: y_offset + y0,
        width: x1.saturating_sub(x0).max(1),
        height: y1.saturating_sub(y0).max(1),
        min,
        max,
        mean,
        stddev,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_heatmap_image<T: HeatmapNumber>(
    buffer: &mut [u8],
    width: u32,
    height: u32,
    data: &Array2<T>,
    transpose: bool,
    viewport_rows: usize,
    viewport_cols: usize,
    selected_cells: Option<HeatmapSelectedCells>,
    color_scale: HeatmapColorScale,
    settings: &HeatmapSettings,
) -> Result<(), AppError> {
    let (nan_r, nan_g, nan_b) =
        configure::rgb_channels(configure::themed_color(|colors| colors.surface.break_line));
    let (cursor_r, cursor_g, cursor_b) =
        configure::rgb_channels(configure::themed_color(|colors| {
            colors.surface.panel_border
        }));
    let rows = if transpose {
        data.shape()[1]
    } else {
        data.shape()[0]
    };
    let cols = if transpose {
        data.shape()[0]
    } else {
        data.shape()[1]
    };
    let draw_width = width as usize;
    let draw_height = height as usize;

    for y in 0..height as usize {
        for x in 0..width as usize {
            let display_row = (y * rows) / draw_height.max(1);
            let src_row = if settings.invert_y {
                rows.saturating_sub(display_row + 1)
            } else {
                display_row
            };
            let display_col = (x * cols) / draw_width.max(1);
            let src_col = if settings.invert_x {
                cols.saturating_sub(display_col + 1)
            } else {
                display_col
            };
            let value = heatmap_value(data, transpose, src_row, src_col);
            let rgb = if !value.is_finite() || !color_scale.has_finite {
                (nan_r, nan_g, nan_b)
            } else {
                let normalized = apply_invert_colors(
                    normalize_heatmap_value(
                        value,
                        color_scale.min,
                        color_scale.max,
                        settings.normalization,
                    ),
                    settings.invert_c,
                );
                heatmap_colormap_rgb(normalized, settings.colormap)
            };
            write_rgb(buffer, width as usize, x, y, rgb);
        }
    }

    let root = BitMapBackend::with_buffer(buffer, (width, height)).into_drawing_area();
    if let Some(selected) = selected_cells {
        let left = ((selected.col_start * draw_width) / viewport_cols.max(1)) as i32;
        let right = (((selected.col_end + 1) * draw_width) / viewport_cols.max(1)) as i32;
        let top = ((selected.row_start * draw_height) / viewport_rows.max(1)) as i32;
        let bottom = (((selected.row_end + 1) * draw_height) / viewport_rows.max(1)) as i32;
        root.draw(&PlotRectangle::new(
            [(left, top), (right.max(left + 1), bottom.max(top + 1))],
            ShapeStyle::from(&RGBColor(cursor_r, cursor_g, cursor_b)).stroke_width(2),
        ))
        .map_err(|e| AppError::DrawingError(format!("Error drawing heatmap selection: {e}")))?;
    }
    root.present()
        .map_err(|e| AppError::DrawingError(format!("Error presenting heatmap image: {e}")))?;
    Ok(())
}

fn write_rgb(buffer: &mut [u8], width: usize, x: usize, y: usize, rgb: (u8, u8, u8)) {
    let offset = (y * width + x) * 3;
    if offset + 2 >= buffer.len() {
        return;
    }
    buffer[offset] = rgb.0;
    buffer[offset + 1] = rgb.1;
    buffer[offset + 2] = rgb.2;
}

fn turbo_rgb(value: f64) -> (u8, u8, u8) {
    const STOPS: &[(f64, (u8, u8, u8))] = &[
        (0.0, (48, 18, 59)),
        (0.2, (50, 103, 184)),
        (0.4, (38, 188, 225)),
        (0.6, (113, 250, 70)),
        (0.8, (245, 190, 40)),
        (1.0, (180, 4, 38)),
    ];
    let value = value.clamp(0.0, 1.0);
    for window in STOPS.windows(2) {
        let (start_pos, start_rgb) = window[0];
        let (end_pos, end_rgb) = window[1];
        if (start_pos..=end_pos).contains(&value) {
            let t = if (end_pos - start_pos).abs() < f64::EPSILON {
                0.0
            } else {
                (value - start_pos) / (end_pos - start_pos)
            };
            let lerp = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).round() as u8;
            return (
                lerp(start_rgb.0, end_rgb.0),
                lerp(start_rgb.1, end_rgb.1),
                lerp(start_rgb.2, end_rgb.2),
            );
        }
    }
    STOPS[STOPS.len() - 1].1
}

fn normalize_heatmap_value(
    value: f64,
    min: f64,
    max: f64,
    normalization: HeatmapNormalization,
) -> f64 {
    let transformed = match normalization {
        HeatmapNormalization::Linear => Some((value, min, max)),
        HeatmapNormalization::Log => {
            if value <= 0.0 || min <= 0.0 || max <= 0.0 {
                None
            } else {
                Some((value.ln(), min.ln(), max.ln()))
            }
        }
        HeatmapNormalization::Sqrt => {
            if value < 0.0 || min < 0.0 || max < 0.0 {
                None
            } else {
                Some((value.sqrt(), min.sqrt(), max.sqrt()))
            }
        }
    };
    let Some((value, min, max)) = transformed else {
        return 0.0;
    };
    if (max - min).abs() < f64::EPSILON {
        0.5
    } else {
        ((value - min) / (max - min)).clamp(0.0, 1.0)
    }
}

pub(super) fn heatmap_colormap_rgb(value: f64, colormap: HeatmapColormap) -> (u8, u8, u8) {
    match colormap {
        HeatmapColormap::Turbo => turbo_rgb(value),
        HeatmapColormap::Grayscale => {
            let gray = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            (gray, gray, gray)
        }
        HeatmapColormap::Inferno => inferno_rgb(value),
    }
}

pub(super) fn apply_invert_colors(value: f64, invert_colors: bool) -> f64 {
    if invert_colors {
        1.0 - value.clamp(0.0, 1.0)
    } else {
        value
    }
}

fn inferno_rgb(value: f64) -> (u8, u8, u8) {
    const STOPS: &[(f64, (u8, u8, u8))] = &[
        (0.0, (0, 0, 4)),
        (0.25, (87, 15, 109)),
        (0.5, (187, 55, 84)),
        (0.75, (249, 142, 8)),
        (1.0, (252, 255, 164)),
    ];
    let value = value.clamp(0.0, 1.0);
    for window in STOPS.windows(2) {
        let (start_pos, start_rgb) = window[0];
        let (end_pos, end_rgb) = window[1];
        if (start_pos..=end_pos).contains(&value) {
            let t = if (end_pos - start_pos).abs() < f64::EPSILON {
                0.0
            } else {
                (value - start_pos) / (end_pos - start_pos)
            };
            let lerp = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).round() as u8;
            return (
                lerp(start_rgb.0, end_rgb.0),
                lerp(start_rgb.1, end_rgb.1),
                lerp(start_rgb.2, end_rgb.2),
            );
        }
    }
    STOPS[STOPS.len() - 1].1
}
