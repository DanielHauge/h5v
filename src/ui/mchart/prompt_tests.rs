use super::expression::collect_expression_input_ids;
use super::tests::{make_attribute_test_file, make_dataset_ref_test_file, make_state, source};
use super::*;
use std::fs;

#[test]
fn click_expression_editor_opens_prompt_and_moves_name_cursor() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0, 0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(source("/group/a", selection), vec![(0.0, 1.0), (1.0, 2.0)]);
    state
        .create_expression_derived("$1 + 1".to_string())
        .expect("create derived");
    state
        .set_selected_item_name("temp", Some(ChartItemId(2)))
        .expect("rename derived item");
    state.editor_hitbox = Some(MultiChartEditorHitbox {
        area: ratatui::layout::Rect::new(0, 0, 40, 1),
        name_area: ratatui::layout::Rect::new(3, 0, 5, 1),
        expression_area: ratatui::layout::Rect::new(11, 0, 12, 1),
    });

    assert!(state.click_expression_editor(6, 0).expect("click editor"));
    let prompt = state.expression_prompt.as_ref().expect("prompt");
    assert_eq!(prompt.focus, ExpressionPromptFocus::Name);
    assert_eq!(prompt.name_cursor, 2);
}

#[test]
fn prompt_word_movement_uses_word_boundaries() {
    let mut state = make_state();
    state.open_expression_prompt();
    if let Some(prompt) = state.expression_prompt.as_mut() {
        prompt.buffer = "alpha beta gamma".to_string();
        prompt.cursor = prompt.buffer.len();
    }

    state.expression_move_word_left();
    assert_eq!(state.expression_prompt.as_ref().unwrap().cursor, 11);
    state.expression_move_word_left();
    assert_eq!(state.expression_prompt.as_ref().unwrap().cursor, 6);
    state.expression_move_word_right();
    assert_eq!(state.expression_prompt.as_ref().unwrap().cursor, 10);
}

#[test]
fn invalid_expression_submit_is_saved_hidden_and_editable() {
    let mut state = make_state();
    state
        .create_expression_derived("load(/broken".to_string())
        .expect("save invalid expression");

    let item = state.chart_items().last().expect("saved item");
    assert_eq!(item.list_label(), "load(/broken");
    assert!(!item.visible);
    assert!(matches!(item.load_state, MultiChartLoadState::Error(_)));

    state
        .open_selected_item_for_edit()
        .expect("open invalid item");
    let prompt = state.expression_prompt.as_ref().expect("prompt");
    assert_eq!(prompt.buffer, "load(/broken");
}

#[test]
fn updating_expression_to_invalid_keeps_item_as_hidden_error_draft() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(source("/group/a", selection), vec![(0.0, 1.0), (1.0, 2.0)]);
    let id = state
        .create_expression_derived("$1 + 1".to_string())
        .expect("create valid expression");

    state
        .update_expression_item_with_file(id, "$1 +".to_string(), None)
        .expect("save invalid draft");

    let item = state.item_by_id(id).expect("updated item");
    assert_eq!(item.list_label(), "$1 +");
    assert!(!item.visible);
    assert!(matches!(item.load_state, MultiChartLoadState::Error(_)));
}

#[test]
fn fixing_invalid_expression_clears_error_state() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(source("/group/a", selection), vec![(0.0, 1.0), (1.0, 2.0)]);
    let id = state
        .create_expression_derived("$1 + 1".to_string())
        .expect("create valid expression");

    state
        .update_expression_item_with_file(id, "$1 +".to_string(), None)
        .expect("save invalid draft");
    state
        .update_expression_item_with_file(id, "$1 + 2".to_string(), None)
        .expect("repair expression");

    let item = state.item_by_id(id).expect("updated item");
    assert_eq!(item.list_label(), "$1 + 2");
    assert!(item.visible);
    assert_eq!(item.load_state, MultiChartLoadState::Ready);
    assert_eq!(item.series.points, vec![(0.0, 3.0), (1.0, 4.0)]);
}

#[test]
fn updating_series_recomputes_dependent_expressions_recursively() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(0.0, 1.0), (1.0, 2.0)],
    );
    state.add_chart_item(
        source("/group/b", selection),
        vec![(0.0, 10.0), (1.0, 20.0)],
    );
    state
        .create_expression_derived("$1 + 1".to_string())
        .expect("create $3");
    state
        .create_expression_derived("$3 + 1".to_string())
        .expect("create $4");

    state
        .update_expression_item_with_file(ChartItemId(1), "$2 + 5".to_string(), None)
        .expect("update $1");

    assert_eq!(
        state.item_by_id(ChartItemId(1)).unwrap().series.points,
        vec![(0.0, 15.0), (1.0, 25.0)]
    );
    assert_eq!(
        state.item_by_id(ChartItemId(3)).unwrap().series.points,
        vec![(0.0, 16.0), (1.0, 26.0)]
    );
    assert_eq!(
        state.item_by_id(ChartItemId(4)).unwrap().series.points,
        vec![(0.0, 17.0), (1.0, 27.0)]
    );
}

#[test]
fn updating_series_rejects_dependency_cycles_and_keeps_original_series() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(source("/group/a", selection), vec![(0.0, 1.0), (1.0, 2.0)]);
    state
        .create_expression_derived("$1 + 1".to_string())
        .expect("create $2");

    let err = state
        .update_expression_item_with_file(ChartItemId(1), "$2 + 1".to_string(), None)
        .expect_err("cycle should fail");
    assert!(err.contains("dependency cycle"));
    assert_eq!(
        state.item_by_id(ChartItemId(1)).unwrap().series.points,
        vec![(0.0, 1.0), (1.0, 2.0)]
    );
    assert_eq!(
        state.item_by_id(ChartItemId(2)).unwrap().series.points,
        vec![(0.0, 2.0), (1.0, 3.0)]
    );
}

#[test]
fn updating_series_recomputes_xy_dependents_when_used_on_x_axis() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(0.0, 1.0), (1.0, 2.0)],
    );
    state.add_chart_item(
        source("/group/b", selection),
        vec![(0.0, 10.0), (1.0, 20.0)],
    );
    state
        .create_expression_derived("($1 * 10, $2 + 1)".to_string())
        .expect("create xy series");

    state
        .update_expression_item_with_file(ChartItemId(1), "$2 + 5".to_string(), None)
        .expect("update $1");

    assert_eq!(
        state.item_by_id(ChartItemId(3)).unwrap().series.points,
        vec![(150.0, 11.0), (250.0, 21.0)]
    );
}

#[test]
fn updating_xy_series_recomputes_downstream_y_dependents() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(0.0, 1.0), (1.0, 2.0)],
    );
    state.add_chart_item(
        source("/group/b", selection),
        vec![(0.0, 10.0), (1.0, 20.0)],
    );
    state
        .create_expression_derived("($1 * 10, $2 + 1)".to_string())
        .expect("create xy series");
    state
        .create_expression_derived("$3 + 1".to_string())
        .expect("create downstream series");

    state
        .update_expression_item_with_file(ChartItemId(1), "$2 + 5".to_string(), None)
        .expect("update $1");

    assert_eq!(
        state.item_by_id(ChartItemId(3)).unwrap().series.points,
        vec![(150.0, 11.0), (250.0, 21.0)]
    );
    assert_eq!(
        state.item_by_id(ChartItemId(4)).unwrap().series.points,
        vec![(150.0, 12.0), (250.0, 22.0)]
    );
}

#[test]
fn collect_expression_input_ids_includes_item_refs() {
    let tokens = tokenize_expression("($1, $2 + 1)").expect("tokenize");
    let parsed = parse_derived_expression(&tokens).expect("parse");
    let mut refs = ExpressionRefs::default();
    collect_parsed_expression_refs(&parsed, &mut refs);

    assert_eq!(
        collect_expression_input_ids(&refs),
        vec![ChartItemId(1), ChartItemId(2)]
    );
}

#[test]
fn rejects_item_refs_inside_load_calls() {
    let err = tokenize_expression("load($1:TRACE)").unwrap_err();
    assert!(err.contains("load(/group/dataset) or load(/group/dataset:ATTR)"));
}

#[test]
fn clear_selected_blocks_deleting_series_with_dependents() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(source("/group/a", selection), vec![(0.0, 1.0), (1.0, 2.0)]);
    state
        .create_expression_derived("$1 + 1".to_string())
        .expect("create dependent");
    state.idx = 0;

    let err = state
        .clear_selected()
        .expect_err("delete should be blocked");
    assert!(err.contains("Cannot delete $1"));
    assert!(err.contains("$2"));
    assert_eq!(state.chart_items().len(), 2);
}

#[test]
fn expression_derived_supports_dataset_path_series_inputs() {
    let (file, path) = make_dataset_ref_test_file();
    let mut state = make_state();

    state
        .create_expression_derived_with_file(
            "load(/series) + load(/matrix)[..,1]".to_string(),
            Some(&file),
        )
        .unwrap();

    let derived = state.chart_items().last().unwrap();
    assert_eq!(
        derived.series.points,
        vec![(0.0, 3.0), (1.0, 6.0), (2.0, 9.0)]
    );

    drop(file);
    fs::remove_file(path).expect("failed removing temp hdf5 file");
}

#[test]
fn expression_derived_supports_scalar_dataset_inputs() {
    let (file, path) = make_dataset_ref_test_file();
    let mut state = make_state();

    state
        .create_expression_derived_with_file(
            "load(/series) + load(/scalar)".to_string(),
            Some(&file),
        )
        .unwrap();

    let derived = state.chart_items().last().unwrap();
    assert_eq!(
        derived.series.points,
        vec![(0.0, 3.5), (1.0, 5.5), (2.0, 7.5)]
    );

    drop(file);
    fs::remove_file(path).expect("failed removing temp hdf5 file");
}

#[test]
fn expression_derived_dataset_path_refs_validate_series_lengths() {
    let (file, path) = make_dataset_ref_test_file();
    let mut state = make_state();

    state
        .create_expression_derived_with_file(
            "load(/series) + load(/matrix)[1,..]".to_string(),
            Some(&file),
        )
        .expect("save invalid draft");
    let item = state.chart_items().last().unwrap();
    assert_eq!(item.list_label(), "load(/series) + load(/matrix)[1,..]");
    assert!(!item.visible);
    assert!(
        matches!(item.load_state, MultiChartLoadState::Error(ref message) if message.contains("lengths must match"))
    );

    drop(file);
    fs::remove_file(path).expect("failed removing temp hdf5 file");
}

#[test]
fn expression_derived_xy_tuple_creates_xy_series() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0, 0],
        x: 1,
        slice: SliceSelection::All,
    };

    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(10.0, 2.0), (20.0, 4.0)],
    );
    state.add_chart_item(
        source("/group/b", selection),
        vec![(100.0, 3.0), (200.0, 5.0)],
    );

    state
        .create_expression_derived("($1 * 10, $2 + 1)".to_string())
        .unwrap();

    let derived = state.chart_items().last().unwrap();
    assert_eq!(derived.series.points, vec![(20.0, 4.0), (40.0, 6.0)]);
    match &derived.source {
        ChartSource::DerivedExpression {
            input_ids,
            len,
            kind,
            ..
        } => {
            assert_eq!(input_ids, &vec![ChartItemId(1), ChartItemId(2)]);
            assert_eq!(*len, 2);
            assert_eq!(*kind, DerivedExpressionKind::XySeries);
        }
        other => panic!("expected expression-derived source, got {other:?}"),
    }
}

#[test]
fn expression_derived_xy_tuple_requires_matching_lengths() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0, 0],
        x: 1,
        slice: SliceSelection::All,
    };

    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(0.0, 2.0), (1.0, 4.0), (2.0, 6.0)],
    );
    state.add_chart_item(source("/group/b", selection), vec![(0.0, 3.0), (1.0, 5.0)]);

    state
        .create_expression_derived("($1, $2 + 1)".to_string())
        .expect("save invalid draft");
    let item = state.chart_items().last().unwrap();
    assert_eq!(item.list_label(), "($1, $2 + 1)");
    assert!(!item.visible);
    assert!(
        matches!(item.load_state, MultiChartLoadState::Error(ref message) if message.contains("lengths must match"))
    );
}

#[test]
fn scalar_expression_items_store_values_without_plot_series() {
    let (file, path) = make_dataset_ref_test_file();
    let mut state = make_state();

    state
        .create_expression_derived_with_file("load(/scalar)".to_string(), Some(&file))
        .unwrap();

    let item = state.chart_items().last().unwrap();
    assert_eq!(item.scalar_value, Some(1.5));
    assert!(!item.has_loaded_series());
    assert_eq!(item.data_state_label(), "value 1.5");
    match &item.source {
        ChartSource::DerivedExpression { kind, .. } => {
            assert_eq!(*kind, DerivedExpressionKind::Scalar);
        }
        other => panic!("expected scalar derived source, got {other:?}"),
    }

    drop(file);
    let _ = fs::remove_file(path);
}

#[test]
#[ignore = "real HDF5 attribute reads are unstable in the default parallel test environment"]
fn expression_derived_supports_scalar_attributes_from_dataset_and_paths() {
    let (file, path) = make_attribute_test_file();
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };

    state.add_chart_item(
        ChartSource::DatasetSelection(DatasetChartSource {
            dataset_path: "/parent/child/ds".to_string(),
            display_path: "/parent/child/ds".to_string(),
            selection,
            shape: vec![2],
            kind: DatasetChartKind::Dataset,
        }),
        vec![(0.0, 1.0), (1.0, 2.0)],
    );

    state
        .create_expression_derived_with_file(
            "$1 + load(/parent/child:CHILD_OFFSET) + load(/parent/otherds:BIAS) + load(/parent/scalar)"
                .to_string(),
            Some(&file),
        )
        .unwrap();

    let derived = state.chart_items().last().unwrap();
    assert_eq!(derived.series.points, vec![(0.0, 8.0), (1.0, 10.0)]);

    drop(file);
    fs::remove_file(path).expect("failed removing temp hdf5 file");
}

#[test]
fn expression_derived_rejects_item_refs_inside_load_calls() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(source("/group/a", selection), vec![(0.0, 1.0), (1.0, 2.0)]);

    state
        .create_expression_derived("$1 + load($1:FLAG)".to_string())
        .expect("invalid expressions are persisted as drafts");

    let item = state.chart_items().last().expect("draft item");
    assert!(!item.visible);
    assert!(matches!(
        item.load_state,
        MultiChartLoadState::Error(ref message)
            if message.contains("load(/group/dataset) or load(/group/dataset:ATTR)")
    ));
}
