use super::tests::{make_dataset_ref_test_file, make_state, source};
use super::*;
use std::fs;

#[test]
fn scalar_functions_support_series_and_scalar_references() {
    let (file, path) = make_dataset_ref_test_file();
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };

    state.add_chart_item(
        source("/group/a", selection),
        vec![(0.0, 1.0), (1.0, 3.0), (2.0, 5.0)],
    );
    state
        .create_expression_derived_with_file("load(/scalar)".to_string(), Some(&file))
        .unwrap();
    state
        .create_expression_derived_with_file("exp($1, $2)".to_string(), Some(&file))
        .unwrap();
    state
        .create_expression_derived("avg($1)".to_string())
        .unwrap();
    state
        .create_expression_derived("mean($1)".to_string())
        .unwrap();
    state
        .create_expression_derived("stddev($1)".to_string())
        .unwrap();
    state
        .create_expression_derived("len($1)".to_string())
        .unwrap();
    state
        .create_expression_derived("sqrt(abs($1 - 4))".to_string())
        .unwrap();
    state
        .create_expression_derived_with_file("round(load(/scalar))".to_string(), Some(&file))
        .unwrap();

    let exp_item = &state.chart_items()[2];
    let avg_item = &state.chart_items()[3];
    let mean_item = &state.chart_items()[4];
    let stddev_item = &state.chart_items()[5];
    let len_item = &state.chart_items()[6];
    let unary_series_item = &state.chart_items()[7];
    let unary_scalar_item = &state.chart_items()[8];
    assert_eq!(
        exp_item.series.points,
        vec![
            (0.0, 1.0_f64.powf(1.5)),
            (1.0, 3.0_f64.powf(1.5)),
            (2.0, 5.0_f64.powf(1.5))
        ]
    );
    assert_eq!(avg_item.scalar_value, Some(3.0));
    assert_eq!(avg_item.data_state_label(), "value 3");
    assert_eq!(mean_item.scalar_value, Some(3.0));
    assert_eq!(stddev_item.scalar_value, Some((8.0_f64 / 3.0).sqrt()));
    assert_eq!(len_item.scalar_value, Some(3.0));
    assert_eq!(
        unary_series_item.series.points,
        vec![
            (0.0, 3.0_f64.sqrt()),
            (1.0, 1.0_f64.sqrt()),
            (2.0, 1.0_f64.sqrt())
        ]
    );
    assert_eq!(unary_scalar_item.scalar_value, Some(2.0));

    drop(file);
    let _ = fs::remove_file(path);
}

#[test]
fn derived_series_functions_support_rolling_threshold_and_diff() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };

    state.add_chart_item(
        source("/group/a", selection),
        vec![(0.0, 1.0), (1.0, 3.0), (2.0, 5.0)],
    );
    state
        .create_expression_derived("rolling_mean($1, 2)".to_string())
        .unwrap();
    state
        .create_expression_derived("rolling_median($1, 2)".to_string())
        .unwrap();
    state
        .create_expression_derived("rolling_stddev($1, 2)".to_string())
        .unwrap();
    state
        .create_expression_derived("rolling_min($1, 2)".to_string())
        .unwrap();
    state
        .create_expression_derived("rolling_max($1, 2)".to_string())
        .unwrap();
    state
        .create_expression_derived("rolling_quantile($1, 2, 0.25)".to_string())
        .unwrap();
    state
        .create_expression_derived("threshold($1, 4)".to_string())
        .unwrap();
    state
        .create_expression_derived("diff($1)".to_string())
        .unwrap();

    assert_eq!(
        state.chart_items()[1].series.points,
        vec![(0.0, 1.0), (1.0, 2.0), (2.0, 4.0)]
    );
    assert_eq!(
        state.chart_items()[2].series.points,
        vec![(0.0, 1.0), (1.0, 2.0), (2.0, 4.0)]
    );
    assert_eq!(
        state.chart_items()[3].series.points,
        vec![(0.0, 0.0), (1.0, 1.0), (2.0, 1.0)]
    );
    assert_eq!(
        state.chart_items()[4].series.points,
        vec![(0.0, 1.0), (1.0, 1.0), (2.0, 3.0)]
    );
    assert_eq!(
        state.chart_items()[5].series.points,
        vec![(0.0, 1.0), (1.0, 3.0), (2.0, 5.0)]
    );
    assert_eq!(
        state.chart_items()[6].series.points,
        vec![(0.0, 1.0), (1.0, 1.5), (2.0, 3.5)]
    );
    assert_eq!(
        state.chart_items()[7].series.points,
        vec![(0.0, 0.0), (1.0, 0.0), (2.0, 1.0)]
    );
    assert_eq!(
        state.chart_items()[8].series.points,
        vec![(0.0, 0.0), (1.0, 2.0), (2.0, 2.0)]
    );
}

#[test]
fn interp_resamples_xy_series_to_next_sample_modulus() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };

    state.add_chart_item(
        source("/group/x", selection.clone()),
        vec![(0.0, 2.27), (1.0, 2.32), (2.0, 2.41)],
    );
    state.add_chart_item(
        source("/group/y", selection),
        vec![(0.0, 10.0), (1.0, 20.0), (2.0, 40.0)],
    );
    state
        .create_expression_derived("($1, $2)".to_string())
        .unwrap();
    state
        .create_expression_derived("interp($3, 0.05)".to_string())
        .unwrap();

    let interp_item = &state.chart_items()[3];
    assert!(matches!(
        interp_item.source,
        ChartSource::DerivedExpression {
            kind: DerivedExpressionKind::XySeries,
            ..
        }
    ));
    let expected = [
        (2.30, 16.0),
        (2.35, 26.666666666666668),
        (2.40, 37.77777777777778),
    ];
    assert_eq!(interp_item.series.points.len(), expected.len());
    for (actual, expected) in interp_item.series.points.iter().zip(expected.iter()) {
        assert!(
            (actual.0 - expected.0).abs() < 1e-9,
            "{actual:?} != {expected:?}"
        );
        assert!(
            (actual.1 - expected.1).abs() < 1e-9,
            "{actual:?} != {expected:?}"
        );
    }
}

#[test]
fn interp_requires_xy_series_input() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };

    state.add_chart_item(
        source("/group/a", selection),
        vec![(0.0, 1.0), (1.0, 3.0), (2.0, 5.0)],
    );
    state
        .create_expression_derived("interp($1, 0.05)".to_string())
        .expect("save invalid interp draft");

    let item = state.chart_items().last().unwrap();
    assert!(!item.visible);
    assert!(matches!(
        item.load_state,
        MultiChartLoadState::Error(ref message) if message.contains("x/y derived series")
    ));
}

#[test]
fn slice_filters_series_by_x_range() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };

    state.add_chart_item(
        source("/group/a", selection),
        vec![
            (10.0, 1.0),
            (25.5, 3.0),
            (100.0, 5.0),
            (250.5, 7.0),
            (300.0, 9.0),
        ],
    );
    state
        .create_expression_derived("slice($1, 25.5, 250.5)".to_string())
        .unwrap();

    let item = &state.chart_items()[1];
    assert_eq!(
        item.series.points,
        vec![(25.5, 3.0), (100.0, 5.0), (250.5, 7.0)]
    );
}

#[test]
fn slice_requires_nonempty_x_range() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };

    state.add_chart_item(
        source("/group/a", selection),
        vec![(10.0, 1.0), (20.0, 3.0), (30.0, 5.0)],
    );
    state
        .create_expression_derived("slice($1, 40.0, 50.0)".to_string())
        .expect("save invalid slice draft");

    let item = state.chart_items().last().unwrap();
    assert!(!item.visible);
    assert!(matches!(
        item.load_state,
        MultiChartLoadState::Error(ref message) if message.contains("produced no samples")
    ));
}

#[test]
fn scalar_only_functions_reject_series_arguments() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };

    state.add_chart_item(
        source("/group/a", selection),
        vec![(0.0, 1.0), (1.0, 3.0), (2.0, 5.0)],
    );
    state
        .create_expression_derived("max2($1, 2)".to_string())
        .expect("save invalid scalar draft");

    let item = state.chart_items().last().unwrap();
    assert!(!item.visible);
    assert!(matches!(
        item.load_state,
        MultiChartLoadState::Error(ref message) if message.contains("max2() requires scalar arguments")
    ));
}
