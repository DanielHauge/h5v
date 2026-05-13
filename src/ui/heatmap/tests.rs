use ndarray::arr2;

use super::{
    render::{compute_heatmap_color_scale, compute_region_selection, viewport_partition},
    HeatmapPageWindow, HeatmapSegmentAxis,
};
use crate::{
    h5f::{DatasetMeta, Encoding},
    sprint_typedesc::MatrixRenderType,
    ui::state::{HeatmapRangeMode, HeatmapSelectedCells},
};
use hdf5_metno::types::{IntSize, TypeDescriptor};

#[test]
fn viewport_partition_covers_whole_source_extent() {
    assert_eq!(viewport_partition(1564, 80, 0), (0, 19));
    assert_eq!(viewport_partition(1564, 80, 79), (1544, 1564));
}

#[test]
fn viewport_partition_keeps_single_source_pixel_visible() {
    assert_eq!(viewport_partition(3, 3, 0), (0, 1));
    assert_eq!(viewport_partition(3, 3, 2), (2, 3));
}

#[test]
fn heatmap_page_window_last_page_clamps_to_tail() {
    let window = HeatmapPageWindow {
        ds_path: "/x".to_string(),
        axis: HeatmapSegmentAxis::Cols,
        len: 100,
        total: 250,
        page: 3,
        page_count: 4,
    };
    assert_eq!(window.range_for_page(0), (0, 100));
    assert_eq!(window.range_for_page(1), (50, 150));
    assert_eq!(window.current_range(), (150, 250));
}

#[test]
fn heatmap_region_defaults_to_visible_viewport_when_nothing_selected() {
    let data = arr2(&[[1.0, 2.0], [3.0, 4.0]]);
    let region = compute_region_selection(&data, false, 2, 2, 2, 2, None, 10, 20, false, false);
    assert_eq!(region.x, 20);
    assert_eq!(region.y, 10);
    assert_eq!(region.width, 2);
    assert_eq!(region.height, 2);
    assert_eq!(region.min, 1.0);
    assert_eq!(region.max, 4.0);
    assert_eq!(region.mean, 2.5);
}

#[test]
fn heatmap_region_expands_between_two_selected_cells() {
    let data = arr2(&[
        [1.0, 2.0, 3.0, 4.0],
        [5.0, 6.0, 7.0, 8.0],
        [9.0, 10.0, 11.0, 12.0],
        [13.0, 14.0, 15.0, 16.0],
    ]);
    let region = compute_region_selection(
        &data,
        false,
        4,
        4,
        4,
        4,
        Some(HeatmapSelectedCells::normalized(1, 1, 2, 3)),
        0,
        0,
        false,
        false,
    );
    assert_eq!(region.x, 1);
    assert_eq!(region.y, 1);
    assert_eq!(region.width, 3);
    assert_eq!(region.height, 2);
    assert_eq!(region.min, 6.0);
    assert_eq!(region.max, 12.0);
}

#[test]
fn heatmap_minmax_uses_numeric_type_bounds() {
    let data = arr2(&[[1.0, 2.0], [3.0, 4.0]]);
    let attr = DatasetMeta {
        link_name: None,
        display_name: "x".to_string(),
        shape: vec![2, 2],
        data_type: "u8".to_string(),
        unsupported_reason: None,
        type_descriptor: TypeDescriptor::Unsigned(IntSize::U1),
        data_bytesize: 1,
        storage_required: 0,
        total_bytes: 4,
        total_elems: 4,
        chunk_shape: None,
        hl: None,
        matrixable: Some(MatrixRenderType::Float64),
        encoding: Encoding::Unknown,
        image: None,
        enum_render_overrides: None,
        is_link: false,
        filename: String::new(),
        compound_projection: None,
    };
    let scale = compute_heatmap_color_scale(&data, &attr, false, 2, 2, &HeatmapRangeMode::MinMax);
    assert_eq!(scale.min, 0.0);
    assert_eq!(scale.max, 255.0);
}

#[test]
fn heatmap_ignores_non_finite_values_in_stats_and_scale() {
    let data = arr2(&[[1.0, f64::NAN], [f64::INFINITY, 4.0]]);
    let attr = DatasetMeta {
        link_name: None,
        display_name: "x".to_string(),
        shape: vec![2, 2],
        data_type: "f64".to_string(),
        unsupported_reason: None,
        type_descriptor: TypeDescriptor::Float(hdf5_metno::types::FloatSize::U8),
        data_bytesize: 8,
        storage_required: 0,
        total_bytes: 32,
        total_elems: 4,
        chunk_shape: None,
        hl: None,
        matrixable: Some(MatrixRenderType::Float64),
        encoding: Encoding::Unknown,
        image: None,
        enum_render_overrides: None,
        is_link: false,
        filename: String::new(),
        compound_projection: None,
    };
    let scale = compute_heatmap_color_scale(&data, &attr, false, 2, 2, &HeatmapRangeMode::Auto);
    let region = compute_region_selection(&data, false, 2, 2, 2, 2, None, 0, 0, false, false);
    assert!(scale.has_finite);
    assert_eq!(scale.min, 1.0);
    assert_eq!(scale.max, 4.0);
    assert_eq!(region.min, 1.0);
    assert_eq!(region.max, 4.0);
    assert_eq!(region.mean, 2.5);
}
