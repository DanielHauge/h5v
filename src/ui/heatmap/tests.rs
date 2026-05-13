use ndarray::arr2;

use super::{
    render::{compute_region_selection, viewport_partition},
    HeatmapPageWindow, HeatmapSegmentAxis,
};
use crate::ui::state::HeatmapSelectedCells;

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
