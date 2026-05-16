use super::load::coalesce_load_requests;
use super::tests::{assert_viewport, make_dataset_ref_test_file, make_state, source};
use super::*;
use std::fs;

#[test]
fn zoom_in_anchor_ratio_biases_toward_hovered_side() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection),
        (0..100).map(|i| (i as f64, i as f64)).collect(),
    );

    assert!(state.zoom_in_x(10.0));
    assert_viewport(&state, Some((9.9, 89.1, 0.0, 99.0)));

    state.clear_zoom();
    assert!(state.zoom_with_anchor(10.0, 0.0, 0.0, true, ChartZoomMode::XOnly));
    assert_viewport(&state, Some((0.0, 79.2, 0.0, 99.0)));

    state.clear_zoom();
    assert!(state.zoom_with_anchor(10.0, 1.0, 0.0, true, ChartZoomMode::XOnly));
    assert_viewport(&state, Some((19.8, 99.0, 0.0, 99.0)));
}

#[test]
fn chart_panel_title_includes_viewport_when_zoomed() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection),
        (0..10).map(|i| (i as f64, (i * 2) as f64)).collect(),
    );

    assert_eq!(state.chart_panel_title(), " 📈 Chart ");

    state.viewport = Some(ChartViewport {
        x_min: 2.0,
        x_max: 6.0,
        y_min: 4.0,
        y_max: 12.0,
    });
    assert_eq!(state.chart_panel_title(), " 📈 Chart ");
}

#[test]
fn chart_mode_subheader_tracks_active_view_mode() {
    let mut state = make_state();
    assert_eq!(
        state.chart_mode_subheader(),
        "[x values] - parametric curves and sampled series"
    );

    state.cycle_view_mode();
    assert_eq!(
        state.chart_mode_subheader(),
        "[visible sample values] - overlaid distributions"
    );
}

#[test]
fn histogram_mode_disables_zoom_changes() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection),
        (0..10).map(|i| (i as f64, i as f64)).collect(),
    );
    state.cycle_view_mode();

    assert!(!state.zoom_in(10.0));
    assert_viewport(&state, None);
}

#[test]
fn comparison_scatter_mode_disables_zoom_changes() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection.clone()),
        (0..10).map(|i| (i as f64, i as f64)).collect(),
    );
    state.add_chart_item(
        source("/group/b", selection),
        (0..10).map(|i| (i as f64, (i * 2) as f64)).collect(),
    );
    state.cycle_view_mode();
    state.cycle_view_mode();

    assert!(!state.zoom_in(10.0));
    assert_viewport(&state, None);
}

#[test]
fn coalesce_load_requests_keeps_latest_request_per_item_and_kind() {
    let (file, path) = make_dataset_ref_test_file();
    let dataset = file.dataset("/series").expect("series dataset");
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };

    let requests = coalesce_load_requests(vec![
        MultiChartLoadRequest {
            item_id: ChartItemId(1),
            kind: MultiChartLoadKind::Detail {
                generation: 1,
                window: ChartLodWindow {
                    start: 0,
                    end: 10,
                    sample_cap: 10,
                },
            },
            source: MultiChartLoadSource::Dataset {
                dataset: dataset.clone(),
                selection: selection.clone(),
            },
        },
        MultiChartLoadRequest {
            item_id: ChartItemId(1),
            kind: MultiChartLoadKind::Detail {
                generation: 2,
                window: ChartLodWindow {
                    start: 5,
                    end: 15,
                    sample_cap: 10,
                },
            },
            source: MultiChartLoadSource::Dataset {
                dataset: dataset.clone(),
                selection: selection.clone(),
            },
        },
        MultiChartLoadRequest {
            item_id: ChartItemId(1),
            kind: MultiChartLoadKind::Overview { generation: 0 },
            source: MultiChartLoadSource::Dataset { dataset, selection },
        },
    ]);

    assert_eq!(requests.len(), 2);
    assert!(matches!(
        requests[0].kind,
        MultiChartLoadKind::Detail { generation: 2, .. }
    ));
    assert!(matches!(
        requests[1].kind,
        MultiChartLoadKind::Overview { generation: 0 }
    ));

    drop(file);
    fs::remove_file(path).expect("failed removing temp hdf5 file");
}

#[test]
fn coalesce_load_requests_prioritizes_latest_detail_work() {
    let (file, path) = make_dataset_ref_test_file();
    let dataset = file.dataset("/series").expect("series dataset");
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };

    let requests = coalesce_load_requests(vec![
        MultiChartLoadRequest {
            item_id: ChartItemId(1),
            kind: MultiChartLoadKind::Overview { generation: 0 },
            source: MultiChartLoadSource::Dataset {
                dataset: dataset.clone(),
                selection: selection.clone(),
            },
        },
        MultiChartLoadRequest {
            item_id: ChartItemId(2),
            kind: MultiChartLoadKind::Detail {
                generation: 1,
                window: ChartLodWindow {
                    start: 0,
                    end: 10,
                    sample_cap: 10,
                },
            },
            source: MultiChartLoadSource::Dataset {
                dataset: dataset.clone(),
                selection: selection.clone(),
            },
        },
        MultiChartLoadRequest {
            item_id: ChartItemId(3),
            kind: MultiChartLoadKind::Detail {
                generation: 2,
                window: ChartLodWindow {
                    start: 5,
                    end: 15,
                    sample_cap: 10,
                },
            },
            source: MultiChartLoadSource::Dataset { dataset, selection },
        },
    ]);

    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].item_id, ChartItemId(3));
    assert_eq!(requests[1].item_id, ChartItemId(2));
    assert_eq!(requests[2].item_id, ChartItemId(1));
    assert!(matches!(
        requests[0].kind,
        MultiChartLoadKind::Detail { .. }
    ));
    assert!(matches!(
        requests[1].kind,
        MultiChartLoadKind::Detail { .. }
    ));
    assert!(matches!(
        requests[2].kind,
        MultiChartLoadKind::Overview { generation: 0 }
    ));

    drop(file);
    fs::remove_file(path).expect("failed removing temp hdf5 file");
}

#[test]
fn zoom_at_position_only_applies_inside_chart_area() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection),
        (0..100).map(|i| (i as f64, i as f64)).collect(),
    );
    state.last_chart_area = Some(Rect::new(10, 5, 20, 8));

    assert!(!state.zoom_in_at_position(5, 6, 10.0, ChartZoomMode::XOnly));
    assert_viewport(&state, None);

    assert!(state.zoom_in_at_position(10, 6, 10.0, ChartZoomMode::XOnly));
    assert_viewport(&state, Some((0.0, 79.2, 0.0, 99.0)));
}

#[test]
fn chart_plot_area_conversion_respects_padding() {
    let plot_area =
        chart_plot_area_in_rect(Rect::new(10, 5, 20, 8), 200, 80, 40..180, 10..70).unwrap();
    assert_eq!(plot_area, Rect::new(14, 6, 14, 6));
}

#[test]
fn zoom_at_position_ignores_chart_padding() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection),
        (0..100).map(|i| (i as f64, i as f64)).collect(),
    );
    state.last_chart_area =
        chart_plot_area_in_rect(Rect::new(10, 5, 20, 8), 200, 80, 40..180, 10..70);

    assert!(!state.zoom_in_at_position(11, 6, 10.0, ChartZoomMode::XOnly));
    assert_viewport(&state, None);

    assert!(state.zoom_in_at_position(14, 6, 10.0, ChartZoomMode::XOnly));
    assert_viewport(&state, Some((0.0, 79.2, 0.0, 99.0)));
}

#[test]
fn drag_pan_applies_snapshot_on_release_in_both_axes() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection),
        (0..100).map(|i| (i as f64, i as f64)).collect(),
    );
    state.viewport = Some(ChartViewport {
        x_min: 20.0,
        x_max: 80.0,
        y_min: 20.0,
        y_max: 80.0,
    });
    state.last_chart_area = Some(Rect::new(10, 5, 20, 8));

    assert!(state.start_drag_at_position(20, 6));
    assert!(!state.drag_to_position(15, 4));
    assert_viewport(&state, Some((20.0, 80.0, 20.0, 80.0)));

    assert!(state.finish_drag_at_position(15, 4));
    assert_viewport(
        &state,
        Some((
            35.78947368421053,
            95.78947368421052,
            2.8571428571428577,
            62.85714285714286,
        )),
    );

    state.end_drag();
    assert!(!state.drag_to_position(25, 4));
}

#[test]
fn drag_pan_only_starts_inside_chart_area() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection),
        (0..100).map(|i| (i as f64, i as f64)).collect(),
    );
    state.viewport = Some(ChartViewport {
        x_min: 20.0,
        x_max: 80.0,
        y_min: 20.0,
        y_max: 80.0,
    });
    state.last_chart_area = Some(Rect::new(10, 5, 20, 8));

    assert!(!state.start_drag_at_position(5, 6));
    assert!(!state.drag_to_position(15, 5));
    assert_viewport(&state, Some((20.0, 80.0, 20.0, 80.0)));
}

#[test]
fn fit_selected_uses_selected_series_bounds() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection),
        (0..100).map(|i| (i as f64, i as f64)).collect(),
    );
    state.add_chart_item(
        source(
            "/group/b",
            PreviewSelection {
                index: vec![0],
                x: 0,
                slice: SliceSelection::All,
            },
        ),
        vec![(-10.0, -5.0), (5.0, 10.0)],
    );
    state.idx = 1;

    assert!(state.fit_selected());
    assert_viewport(&state, Some((-10.0, 5.0, -5.0, 10.0)));
}

#[test]
fn fit_all_clears_explicit_viewport() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection),
        (0..100).map(|i| (i as f64, i as f64)).collect(),
    );
    state.viewport = Some(ChartViewport {
        x_min: 10.0,
        x_max: 20.0,
        y_min: 10.0,
        y_max: 20.0,
    });

    assert!(state.fit_all());
    assert_viewport(&state, None);
}

#[test]
fn chart_series_filters_non_finite_points() {
    let series = ChartSeries::from_points(vec![
        (0.0, 1.0),
        (1.0, f64::NAN),
        (f64::INFINITY, 2.0),
        (2.0, 3.0),
    ])
    .expect("finite points should remain");
    assert_eq!(series.points, vec![(0.0, 1.0), (2.0, 3.0)]);
    assert_eq!(series.y_min, 1.0);
    assert_eq!(series.y_max, 3.0);
}

#[test]
fn dataset_plot_preview_filters_non_finite_points() {
    let preview = dataset_ploting_data_from_points(vec![
        (0.0, f64::NAN),
        (1.0, 4.0),
        (2.0, f64::INFINITY),
        (3.0, 6.0),
    ])
    .expect("finite preview points");
    assert_eq!(preview.data, vec![(1.0, 4.0), (3.0, 6.0)]);
    assert_eq!(preview.min, 4.0);
    assert_eq!(preview.max, 6.0);
}

#[test]
fn prepared_chart_data_filters_legacy_non_finite_points() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection),
        vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
    );
    state.items[0].series.points[1] = (1.0, f64::NAN);

    let prepared = state.prepared_chart_data().expect("prepared chart data");
    let PreparedChartData::Line(prepared) = prepared else {
        panic!("expected line chart data");
    };
    assert_eq!(prepared.series.len(), 1);
    assert_eq!(prepared.series[0].points, vec![(0.0, 1.0), (2.0, 3.0)]);
    assert_eq!(prepared.y_min, 1.0);
    assert_eq!(prepared.y_max, 3.0);
}

#[test]
fn prepared_chart_data_respects_visibility_and_viewport() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection.clone()),
        (0..6).map(|i| (i as f64, i as f64)).collect(),
    );
    state.add_chart_item(
        source("/group/b", selection),
        (0..6).map(|i| (i as f64, (i * 10) as f64)).collect(),
    );
    state.items[1].visible = false;
    state.viewport = Some(ChartViewport {
        x_min: 1.0,
        x_max: 3.0,
        y_min: 100.0,
        y_max: 200.0,
    });

    let prepared = state.prepared_chart_data().expect("prepared chart data");
    let PreparedChartData::Line(prepared) = prepared else {
        panic!("expected line chart data");
    };
    assert_eq!(prepared.series.len(), 1);
    assert_eq!(
        prepared.series[0].points,
        vec![(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)]
    );
    assert_eq!(prepared.plot_x_min, 1.0);
    assert_eq!(prepared.plot_x_max, 3.0);
    assert_eq!(prepared.y_min, 100.0);
    assert_eq!(prepared.y_max, 200.0);
}

#[test]
fn cycle_view_mode_rotates_through_first_pass_modes() {
    let mut state = make_state();
    assert_eq!(state.view_mode(), MultiChartViewMode::Line);
    assert_eq!(state.cycle_view_mode(), MultiChartViewMode::Histogram);
    assert_eq!(state.cycle_view_mode(), MultiChartViewMode::BoxPlot);
    assert_eq!(
        state.cycle_view_mode(),
        MultiChartViewMode::ComparisonScatter
    );
    assert_eq!(state.cycle_view_mode(), MultiChartViewMode::Line);
}

#[test]
fn box_plot_mode_prepares_visible_window_distribution_summaries() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0), (3.0, 100.0)],
    );
    state.add_chart_item(
        source("/group/b", selection),
        vec![(0.0, 5.0), (1.0, 6.0), (2.0, 7.0), (3.0, 8.0)],
    );
    state.viewport = Some(ChartViewport {
        x_min: 0.0,
        x_max: 3.0,
        y_min: -10.0,
        y_max: 200.0,
    });
    state.cycle_view_mode();
    state.cycle_view_mode();

    let prepared = state.prepared_chart_data().expect("prepared chart data");
    let PreparedChartData::BoxPlot(prepared) = prepared else {
        panic!("expected box plot chart data");
    };
    assert_eq!(prepared.series.len(), 2);
    assert_eq!(prepared.series[0].label, "a[..,0]");
    assert_eq!(prepared.series[0].outliers, vec![100.0]);
    assert!(prepared.series[0].median > prepared.series[0].q1);
    assert!(prepared.series[0].q3 >= prepared.series[0].median);
    assert!(prepared.value_min < 1.0);
    assert!(prepared.value_max > 100.0);
}

#[test]
fn histogram_mode_prepares_overlayed_bins_from_visible_window() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(0.0, 1.0), (1.0, 1.5), (2.0, 2.0), (3.0, 3.5)],
    );
    state.add_chart_item(
        source("/group/b", selection),
        vec![(0.0, 2.0), (1.0, 2.5), (2.0, 3.0), (3.0, 4.0)],
    );
    state.viewport = Some(ChartViewport {
        x_min: 1.0,
        x_max: 2.0,
        y_min: -10.0,
        y_max: 10.0,
    });
    state.cycle_view_mode();

    let prepared = state.prepared_chart_data().expect("prepared chart data");
    let PreparedChartData::Histogram(prepared) = prepared else {
        panic!("expected histogram chart data");
    };
    assert_eq!(prepared.series.len(), 2);
    assert_eq!(prepared.bin_count, 2);
    assert_eq!(
        prepared.series[0]
            .bins
            .iter()
            .map(|bin| bin.count as usize)
            .sum::<usize>(),
        2
    );
    assert_eq!(
        prepared.series[1]
            .bins
            .iter()
            .map(|bin| bin.count as usize)
            .sum::<usize>(),
        2
    );
}

#[test]
fn comparison_scatter_mode_uses_selected_series_and_next_visible_series() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(0.0, 10.0), (1.0, 20.0), (2.0, 30.0)],
    );
    state.add_chart_item(
        source("/group/b", selection.clone()),
        vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
    );
    state.add_chart_item(
        source("/group/c", selection),
        vec![(0.0, 7.0), (1.0, 8.0), (2.0, 9.0)],
    );
    state.idx = 1;
    state.cycle_view_mode();
    state.cycle_view_mode();
    state.cycle_view_mode();

    let prepared = state.prepared_chart_data().expect("prepared chart data");
    let PreparedChartData::ComparisonScatter(prepared) = prepared else {
        panic!("expected comparison scatter data");
    };
    assert_eq!(prepared.x_label, "b[..,0]");
    assert_eq!(prepared.y_label, "c[..,0]");
    assert_eq!(prepared.points, vec![(1.0, 7.0), (2.0, 8.0), (3.0, 9.0)]);
}

#[test]
fn comparison_scatter_mode_updates_when_selection_changes() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(0.0, 10.0), (1.0, 20.0), (2.0, 30.0)],
    );
    state.add_chart_item(
        source("/group/b", selection.clone()),
        vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
    );
    state.add_chart_item(
        source("/group/c", selection),
        vec![(0.0, 7.0), (1.0, 8.0), (2.0, 9.0)],
    );
    state.cycle_view_mode();
    state.cycle_view_mode();
    state.cycle_view_mode();

    let first = state.prepared_chart_data().expect("prepared chart data");
    let PreparedChartData::ComparisonScatter(first) = first else {
        panic!("expected comparison scatter data");
    };
    assert_eq!(first.x_label, "c[..,0]");
    assert_eq!(first.y_label, "a[..,0]");

    state.modified = false;
    state.move_up();
    assert!(state.modified);

    let second = state.prepared_chart_data().expect("prepared chart data");
    let PreparedChartData::ComparisonScatter(second) = second else {
        panic!("expected comparison scatter data");
    };
    assert_eq!(second.x_label, "b[..,0]");
    assert_eq!(second.y_label, "c[..,0]");
}

#[test]
fn comparison_scatter_mode_truncates_longer_visible_series_with_warning() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(0.0, 10.0), (1.0, 20.0), (2.0, 30.0)],
    );
    state.add_chart_item(source("/group/b", selection), vec![(0.0, 1.0), (1.0, 3.0)]);
    state.cycle_view_mode();
    state.cycle_view_mode();
    state.cycle_view_mode();

    let prepared = state.prepared_chart_data().expect("prepared chart data");
    let PreparedChartData::ComparisonScatter(prepared) = prepared else {
        panic!("expected comparison scatter data");
    };
    assert_eq!(prepared.points, vec![(1.0, 10.0), (3.0, 20.0)]);
    assert_eq!(
        state.chart_mode_subheader(),
        "[sample aligned, truncated] - b[..,0] vs a[..,0] (using first 2 aligned samples; a[..,0] truncated by 1 trailing sample from x=2.0000)"
    );
}

#[test]
fn comparison_scatter_mode_rejects_misaligned_visible_windows() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(0.0, 10.0), (1.0, 20.0), (2.0, 30.0)],
    );
    state.add_chart_item(
        source("/group/b", selection),
        vec![(0.0, 1.0), (1.5, 3.0), (2.0, 4.0)],
    );
    state.cycle_view_mode();
    state.cycle_view_mode();
    state.cycle_view_mode();

    assert!(state.prepared_chart_data().is_none());
    assert_eq!(
        state.unavailable_chart_message(),
        "Comparison scatter requires matching visible sample positions in both series."
    );
}

#[test]
fn histogram_render_request_succeeds() {
    let request = MultiChartRenderRequest {
        generation: 1,
        chart_area: Rect::new(0, 0, 40, 12),
        width: 400,
        height: 240,
        prepared: PreparedChartData::Histogram(PreparedHistogramData {
            value_min: 0.0,
            value_max: 4.0,
            count_max: 3.0,
            bin_count: 2,
            series: vec![PreparedHistogramSeries {
                label: "series".to_string(),
                color_slot: 0,
                bins: vec![
                    PreparedHistogramBin {
                        start: 0.0,
                        end: 2.0,
                        count: 3.0,
                    },
                    PreparedHistogramBin {
                        start: 2.0,
                        end: 4.0,
                        count: 1.0,
                    },
                ],
                is_selected: true,
            }],
        }),
    };

    assert!(matches!(
        render::render_prepared_chart_request(request),
        MultiChartRenderResult::Success { .. }
    ));
}

#[test]
fn box_plot_render_request_succeeds() {
    let request = MultiChartRenderRequest {
        generation: 1,
        chart_area: Rect::new(0, 0, 40, 12),
        width: 400,
        height: 240,
        prepared: PreparedChartData::BoxPlot(PreparedBoxPlotData {
            value_min: 0.0,
            value_max: 10.0,
            series: vec![PreparedBoxPlotSeries {
                label: "series".to_string(),
                color_slot: 0,
                x_index: 0,
                q1: 2.0,
                median: 4.0,
                q3: 6.0,
                whisker_low: 1.0,
                whisker_high: 8.0,
                outliers: vec![9.5],
                is_selected: true,
            }],
        }),
    };

    assert!(matches!(
        render::render_prepared_chart_request(request),
        MultiChartRenderResult::Success { .. }
    ));
}

#[test]
fn detail_window_targets_viewport_slice_for_large_dataset_series() {
    let source = DatasetChartSource {
        dataset_path: "/big".to_string(),
        display_path: "/big".to_string(),
        selection: PreviewSelection {
            index: vec![0],
            x: 0,
            slice: SliceSelection::All,
        },
        shape: vec![100_000],
        kind: DatasetChartKind::Dataset,
    };
    let window = MultiChartState::detail_window_for_viewport(
        &source,
        ChartViewport {
            x_min: 1_000.0,
            x_max: 1_500.0,
            y_min: -1.0,
            y_max: 1.0,
        },
        2_048,
    )
    .expect("detail window");
    assert!(window.start <= 1_000);
    assert!(window.end >= 1_500);
    assert_eq!(window.sample_cap, 2_048);
}

#[test]
fn detail_load_replaces_active_series_without_losing_overview() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    let item_id = state
        .add_chart_item(
            source("/group/a", selection),
            (0..8).map(|i| (i as f64, i as f64)).collect(),
        )
        .expect("item");
    state.items[0].source_len = 10_000;
    state.items[0].sampled = true;
    state.items[0].detail_generation = 1;

    state
        .apply_loaded_item(
            item_id,
            MultiChartLoadKind::Detail {
                generation: 1,
                window: ChartLodWindow {
                    start: 100,
                    end: 140,
                    sample_cap: 512,
                },
            },
            vec![(100.0, 1.0), (110.0, 2.0), (120.0, 3.0)],
            0,
        )
        .expect("detail load");

    let item = state.item_by_id(item_id).expect("item");
    assert_eq!(item.series.len(), 8);
    assert_eq!(
        item.active_series().points,
        vec![(100.0, 1.0), (110.0, 2.0), (120.0, 3.0)]
    );
    assert_eq!(
        item.detail_window,
        Some(ChartLodWindow {
            start: 100,
            end: 140,
            sample_cap: 512,
        })
    );
}

#[test]
fn stale_detail_started_event_is_ignored() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    let item_id = state
        .add_chart_item(
            source("/group/a", selection),
            (0..8).map(|i| (i as f64, i as f64)).collect(),
        )
        .expect("item");
    state.items[0].load_state = MultiChartLoadState::Ready;
    state.items[0].detail_generation = 3;
    state.items[0].pending_detail_window = Some(ChartLodWindow {
        start: 20,
        end: 40,
        sample_cap: 256,
    });

    state.apply_load_started(
        item_id,
        MultiChartLoadKind::Detail {
            generation: 2,
            window: ChartLodWindow {
                start: 10,
                end: 20,
                sample_cap: 256,
            },
        },
    );

    assert_eq!(state.items[0].load_state, MultiChartLoadState::Ready);
    assert_eq!(
        state.items[0].pending_detail_window,
        Some(ChartLodWindow {
            start: 20,
            end: 40,
            sample_cap: 256,
        })
    );
}

#[test]
fn clearing_zoom_discards_detail_series() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state
        .add_chart_item(
            ChartSource::DatasetSelection(DatasetChartSource {
                dataset_path: "/missing".to_string(),
                display_path: "/group/a".to_string(),
                selection,
                shape: vec![10_000],
                kind: DatasetChartKind::Dataset,
            }),
            (0..8).map(|i| (i as f64, i as f64)).collect(),
        )
        .expect("item");
    state.items[0].detail_series = ChartSeries::from_points(vec![(10.0, 2.0), (11.0, 3.0)]);
    state.items[0].detail_window = Some(ChartLodWindow {
        start: 10,
        end: 12,
        sample_cap: 512,
    });
    state.viewport = Some(ChartViewport {
        x_min: 10.0,
        x_max: 12.0,
        y_min: 2.0,
        y_max: 3.0,
    });

    assert!(state.clear_zoom());
    assert!(state.items[0].detail_series.is_none());
    assert!(state.items[0].detail_window.is_none());
}

#[test]
fn clearing_zoom_invalidates_inflight_detail_results() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    let item_id = state
        .add_chart_item(
            source("/group/a", selection),
            (0..8).map(|i| (i as f64, i as f64)).collect(),
        )
        .expect("item");
    state.items[0].detail_generation = 4;
    state.items[0].pending_detail_window = Some(ChartLodWindow {
        start: 10,
        end: 20,
        sample_cap: 512,
    });
    state.viewport = Some(ChartViewport {
        x_min: 10.0,
        x_max: 20.0,
        y_min: 0.0,
        y_max: 20.0,
    });
    assert!(state.clear_zoom());

    state
        .apply_loaded_item(
            item_id,
            MultiChartLoadKind::Detail {
                generation: 4,
                window: ChartLodWindow {
                    start: 10,
                    end: 20,
                    sample_cap: 512,
                },
            },
            vec![(10.0, 10.0), (11.0, 11.0)],
            0,
        )
        .expect("stale detail load ignored");

    assert!(state.items[0].detail_series.is_none());
    assert!(state.items[0].detail_window.is_none());
}

#[test]
fn derived_series_builds_detail_from_matching_input_windows() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state
        .add_chart_item(
            source("/group/a", selection.clone()),
            vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
        )
        .expect("source a");
    state
        .add_chart_item(
            source("/group/b", selection),
            vec![(0.0, 10.0), (1.0, 20.0), (2.0, 30.0)],
        )
        .expect("source b");
    state
        .create_expression_derived("$1 + $2".to_string())
        .expect("derived");

    let window = ChartLodWindow {
        start: 100,
        end: 120,
        sample_cap: 256,
    };
    state.items[0].detail_series =
        ChartSeries::from_points(vec![(100.0, 1.0), (110.0, 2.0), (120.0, 3.0)]);
    state.items[0].detail_window = Some(window);
    state.items[1].detail_series =
        ChartSeries::from_points(vec![(100.0, 10.0), (110.0, 20.0), (120.0, 30.0)]);
    state.items[1].detail_window = Some(window);

    state
        .refresh_expression_detail_series(None)
        .expect("refresh detail");

    let derived = state.item_by_id(ChartItemId(3)).expect("derived");
    assert_eq!(derived.detail_window, Some(window));
    assert_eq!(
        derived.active_series().points,
        vec![(100.0, 11.0), (110.0, 22.0), (120.0, 33.0)]
    );
}

#[test]
fn derived_series_detail_clears_when_inputs_do_not_share_window() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state
        .add_chart_item(
            source("/group/a", selection.clone()),
            vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
        )
        .expect("source a");
    state
        .add_chart_item(
            source("/group/b", selection),
            vec![(0.0, 10.0), (1.0, 20.0), (2.0, 30.0)],
        )
        .expect("source b");
    state
        .create_expression_derived("$1 + $2".to_string())
        .expect("derived");

    state.items[0].detail_series =
        ChartSeries::from_points(vec![(100.0, 1.0), (110.0, 2.0), (120.0, 3.0)]);
    state.items[0].detail_window = Some(ChartLodWindow {
        start: 100,
        end: 120,
        sample_cap: 256,
    });
    state.items[1].detail_series =
        ChartSeries::from_points(vec![(200.0, 10.0), (210.0, 20.0), (220.0, 30.0)]);
    state.items[1].detail_window = Some(ChartLodWindow {
        start: 200,
        end: 220,
        sample_cap: 256,
    });

    state
        .refresh_expression_detail_series(None)
        .expect("refresh detail");

    let derived = state.item_by_id(ChartItemId(3)).expect("derived");
    assert!(derived.detail_series.is_none());
    assert!(derived.detail_window.is_none());
}

#[test]
fn stale_background_detail_refresh_results_are_ignored() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state
        .add_chart_item(source("/group/a", selection), vec![(0.0, 1.0), (1.0, 2.0)])
        .expect("source");
    let derived_id = state
        .create_expression_derived("$1 + 1".to_string())
        .expect("derived");
    let current_revision = state.expression_revision();
    let detail_series = ChartSeries::from_points(vec![(10.0, 11.0), (11.0, 12.0)]).expect("detail");
    let detail_window = ChartLodWindow {
        start: 10,
        end: 12,
        sample_cap: 128,
    };

    state
        .apply_expression_refresh_result(MultiChartExpressionRefreshResult::Success {
            revision: current_revision.wrapping_add(1),
            updates: vec![MultiChartDerivedDetailUpdate {
                item_id: derived_id,
                detail_series: Some(detail_series.clone()),
                detail_window: Some(detail_window),
            }],
        })
        .expect("stale result ignored");

    let derived = state.item_by_id(derived_id).expect("derived item");
    assert!(derived.detail_series.is_none());
    assert!(derived.detail_window.is_none());

    state
        .apply_expression_refresh_result(MultiChartExpressionRefreshResult::Success {
            revision: current_revision,
            updates: vec![MultiChartDerivedDetailUpdate {
                item_id: derived_id,
                detail_series: Some(detail_series),
                detail_window: Some(detail_window),
            }],
        })
        .expect("fresh result applied");

    let derived = state.item_by_id(derived_id).expect("derived item");
    assert_eq!(derived.detail_window, Some(detail_window));
    assert_eq!(
        derived.active_series().points,
        vec![(10.0, 11.0), (11.0, 12.0)]
    );
}
