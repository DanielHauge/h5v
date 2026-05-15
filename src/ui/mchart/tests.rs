use super::eval::{dataset_ploting_data_from_points, normalize_absolute_object_path};
use super::expression::{
    collect_expression_input_ids, ExpressionDatasetSelector, ExpressionScalarRef,
    ExpressionSeriesRef,
};
use super::prompt::{
    consume_expression_reference_fragment, current_expression_fragment, expression_prompt_messages,
    expression_suggestion_score, ExpressionPromptFocus,
};
use super::*;
use std::{
    fs,
    sync::mpsc::channel,
    time::{SystemTime, UNIX_EPOCH},
};

use hdf5_metno::File;
use image::{DynamicImage, ImageBuffer, Rgb};
use ndarray::Array;

#[allow(deprecated)]
fn make_state() -> MultiChartState {
    let (tx_load, _rx_load) = channel();
    let (tx_render, _rx_render) = channel();
    MultiChartState::new(Picker::from_fontsize((7, 14)), tx_load, tx_render)
}

fn source(path: &str, selection: PreviewSelection) -> ChartSource {
    ChartSource::DatasetSelection(DatasetChartSource {
        dataset_path: "/raw/ds".to_string(),
        display_path: path.to_string(),
        selection,
        shape: vec![4, 8],
        kind: DatasetChartKind::Dataset,
    })
}

fn assert_viewport(state: &MultiChartState, expected: Option<(f64, f64, f64, f64)>) {
    match (state.viewport, expected) {
        (None, None) => {}
        (Some(actual), Some((x_min, x_max, y_min, y_max))) => {
            assert!((actual.x_min - x_min).abs() < 1e-9, "{actual:?}");
            assert!((actual.x_max - x_max).abs() < 1e-9, "{actual:?}");
            assert!((actual.y_min - y_min).abs() < 1e-9, "{actual:?}");
            assert!((actual.y_max - y_max).abs() < 1e-9, "{actual:?}");
        }
        other => panic!("unexpected viewport state: {other:?}"),
    }
}

#[test]
fn compact_selection_summary_uses_concise_array_notation() {
    let one_d = DatasetChartSource {
        dataset_path: "/raw/ds".to_string(),
        display_path: "/group/chunked_dataset".to_string(),
        selection: PreviewSelection {
            index: vec![0],
            x: 0,
            slice: SliceSelection::All,
        },
        shape: vec![64],
        kind: DatasetChartKind::Dataset,
    };
    assert_eq!(one_d.compact_selection_summary(), "chunked_dataset");

    let three_d = DatasetChartSource {
        dataset_path: "/raw/ds".to_string(),
        display_path: "/group/chunked_dataset".to_string(),
        selection: PreviewSelection {
            index: vec![0, 25, 1],
            x: 0,
            slice: SliceSelection::All,
        },
        shape: vec![64, 32, 8],
        kind: DatasetChartKind::Dataset,
    };
    assert_eq!(
        three_d.compact_selection_summary(),
        "chunked_dataset[..,25,1]"
    );

    let swapped = DatasetChartSource {
        selection: PreviewSelection {
            index: vec![5, 0, 0],
            x: 2,
            slice: SliceSelection::All,
        },
        ..three_d.clone()
    };
    assert_eq!(
        swapped.compact_selection_summary(),
        "chunked_dataset[5,0,..]"
    );

    let sliced = DatasetChartSource {
        selection: PreviewSelection {
            index: vec![0],
            x: 0,
            slice: SliceSelection::FromTo(5, 12),
        },
        shape: vec![64],
        ..one_d
    };
    assert_eq!(sliced.compact_selection_summary(), "chunked_dataset[5..12]");
}

#[test]
fn chart_item_statistics_compute_mean_median_and_stddev() {
    let item = ChartItem {
        id: ChartItemId(1),
        color_slot: 0,
        label: "series".to_string(),
        name: None,
        source: ChartSource::DerivedExpression {
            expression: "series".to_string(),
            input_ids: vec![],
            len: 4,
            kind: DerivedExpressionKind::YSeries,
        },
        series: ChartSeries::from_points(vec![(1.0, 1.0), (2.0, 3.0), (3.0, 5.0), (4.0, 7.0)])
            .expect("series"),
        scalar_value: None,
        detail_series: None,
        detail_window: None,
        pending_detail_window: None,
        detail_generation: 0,
        source_len: 4,
        sampled: false,
        load_state: MultiChartLoadState::Ready,
        visible: true,
    };

    let stats = item.statistics();
    assert_eq!(stats.samples, 4);
    assert_eq!(stats.x_min, 1.0);
    assert_eq!(stats.x_max, 4.0);
    assert_eq!(stats.y_min, 1.0);
    assert_eq!(stats.y_max, 7.0);
    assert_eq!(stats.mean, 4.0);
    assert_eq!(stats.median, 4.0);
    assert!((stats.stddev - (5.0_f64).sqrt()).abs() < 1e-9);
}

fn temp_hdf5_path(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("h5v-{name}-{unique}.h5"))
}

fn make_attribute_test_file() -> (File, std::path::PathBuf) {
    let path = temp_hdf5_path("mchart-attr");
    let file = File::create(&path).expect("failed creating temp hdf5 file");
    let parent = file
        .create_group("parent")
        .expect("failed creating parent group");
    let offset_attr = parent
        .new_attr_builder()
        .empty::<f64>()
        .create("OFFSET")
        .expect("failed creating parent attr");
    offset_attr
        .write_scalar(&3.0_f64)
        .expect("failed writing parent attr");
    let child = parent
        .create_group("child")
        .expect("failed creating child group");
    let child_offset_attr = child
        .new_attr_builder()
        .empty::<f64>()
        .create("CHILD_OFFSET")
        .expect("failed creating child attr");
    child_offset_attr
        .write_scalar(&3.0_f64)
        .expect("failed writing child attr");
    let dataset = child
        .new_dataset_builder()
        .with_data(&[1.0_f64, 2.0_f64])
        .create("ds")
        .expect("failed creating dataset");
    let scale_attr = dataset
        .new_attr_builder()
        .empty::<f64>()
        .create("SCALE")
        .expect("failed creating dataset attr");
    scale_attr
        .write_scalar(&2.0_f64)
        .expect("failed writing dataset attr");
    let flag_attr = dataset
        .new_attr_builder()
        .empty::<bool>()
        .create("FLAG")
        .expect("failed creating non numeric attr");
    flag_attr
        .write_scalar(&true)
        .expect("failed writing non numeric attr");
    dataset
        .new_attr_builder()
        .with_data(&[4.0_f64, 8.0_f64])
        .create("TRACE")
        .expect("failed creating series attr");
    let other = parent
        .new_dataset_builder()
        .with_data(&[0.0_f64])
        .create("otherds")
        .expect("failed creating other dataset");
    let bias_attr = other
        .new_attr_builder()
        .empty::<f64>()
        .create("BIAS")
        .expect("failed creating other dataset attr");
    bias_attr
        .write_scalar(&5.0_f64)
        .expect("failed writing other dataset attr");
    let scalar = parent
        .new_dataset_builder()
        .empty::<f64>()
        .create("scalar")
        .expect("failed creating scalar dataset");
    scalar
        .write_scalar(&7.0_f64)
        .expect("failed writing scalar dataset");
    file.flush().expect("failed flushing temp hdf5 file");
    (file, path)
}

fn make_dataset_ref_test_file() -> (File, std::path::PathBuf) {
    let path = temp_hdf5_path("mchart-dataset-ref");
    let file = File::create(&path).expect("failed creating temp hdf5 file");
    file.new_dataset_builder()
        .with_data(&[2.0_f64, 4.0_f64, 6.0_f64])
        .create("series")
        .expect("failed creating 1d dataset");
    let matrix = Array::from_shape_vec((3, 2), vec![10.0_f64, 1.0, 20.0, 2.0, 30.0, 3.0])
        .expect("failed creating matrix test array");
    file.new_dataset_builder()
        .with_data(matrix.view())
        .create("matrix")
        .expect("failed creating 2d dataset");
    let scalar = file
        .new_dataset_builder()
        .empty::<f64>()
        .create("scalar")
        .expect("failed creating scalar dataset");
    scalar
        .write_scalar(&1.5_f64)
        .expect("failed writing scalar dataset");
    file.flush().expect("failed flushing temp hdf5 file");
    (file, path)
}

#[test]
fn reuses_exact_source_and_adds_distinct_selection_variants() {
    let mut state = make_state();
    let first_selection = PreviewSelection {
        index: vec![0, 0],
        x: 1,
        slice: SliceSelection::All,
    };
    let second_selection = PreviewSelection {
        index: vec![1, 0],
        x: 1,
        slice: SliceSelection::All,
    };

    state.add_chart_item(
        source("/group/ds", first_selection.clone()),
        vec![(0.0, 1.0), (1.0, 2.0)],
    );
    state.add_chart_item(
        source("/group/ds", first_selection),
        vec![(0.0, 3.0), (1.0, 4.0)],
    );
    state.add_chart_item(
        source("/group/ds", second_selection),
        vec![(0.0, 5.0), (1.0, 6.0)],
    );

    assert_eq!(state.chart_items().len(), 2);
    assert_eq!(state.source_item_count("/group/ds"), 2);
    assert_eq!(state.chart_items()[0].series.len(), 2);
    assert_eq!(state.chart_items()[0].series.y_max, 4.0);
}

#[test]
fn expression_derived_supports_item_refs_literals_and_precedence() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0, 0],
        x: 1,
        slice: SliceSelection::All,
    };

    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(0.0, 2.0), (1.0, 4.0)],
    );
    state.add_chart_item(source("/group/b", selection), vec![(0.0, 3.0), (1.0, 5.0)]);

    state
        .create_expression_derived("$1 + $2 * 2".to_string())
        .unwrap();

    let derived = state.chart_items().last().unwrap();
    assert_eq!(derived.series.points, vec![(0.0, 8.0), (1.0, 14.0)]);
    match &derived.source {
        ChartSource::DerivedExpression { input_ids, len, .. } => {
            assert_eq!(input_ids, &vec![ChartItemId(1), ChartItemId(2)]);
            assert_eq!(*len, 2);
        }
        other => panic!("expected expression-derived source, got {other:?}"),
    }
}

#[test]
fn expression_derived_rejects_mismatched_x_values() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0, 0],
        x: 1,
        slice: SliceSelection::All,
    };

    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(0.0, 2.0), (1.0, 4.0)],
    );
    state.add_chart_item(
        source("/group/b", selection),
        vec![(10.0, 3.0), (20.0, 5.0)],
    );

    state
        .create_expression_derived("$1 + $2".to_string())
        .expect("save invalid draft");
    let item = state.chart_items().last().unwrap();
    assert_eq!(item.list_label(), "$1 + $2");
    assert!(!item.visible);
    assert!(
        matches!(item.load_state, MultiChartLoadState::Error(ref message) if message.contains("x-values must match"))
    );
}

#[test]
fn tokenizes_explicit_scalar_references() {
    let tokens =
        tokenize_expression("$1 * load(/parent/scalar) + load(/parent/otherds:BIAS)").unwrap();
    assert!(tokens.iter().any(|token| matches!(
        token,
        ExpressionToken::LoadRef(ExpressionScalarRef {
            target: ExpressionObjectTarget::AbsolutePath(path),
            attr_name: None,
            selectors: None,
        }) if path == "/parent/scalar"
    )));
    assert!(tokens.iter().any(|token| matches!(
        token,
        ExpressionToken::LoadRef(ExpressionScalarRef {
            target: ExpressionObjectTarget::AbsolutePath(path),
            attr_name: Some(attr_name),
            selectors: None,
        }) if path == "/parent/otherds" && attr_name == "BIAS"
    )));
}

#[test]
fn tokenizes_explicit_series_references() {
    let tokens = tokenize_expression("load(/series) + load(/matrix)[.., 1]").unwrap();
    assert!(tokens.iter().any(|token| matches!(
        token,
        ExpressionToken::LoadRef(ExpressionSeriesRef {
            target: ExpressionObjectTarget::AbsolutePath(path),
            attr_name: None,
            selectors: None,
        }) if path == "/series"
    )));
    assert!(tokens.iter().any(|token| matches!(
        token,
        ExpressionToken::LoadRef(ExpressionSeriesRef {
            target: ExpressionObjectTarget::AbsolutePath(path),
            attr_name: None,
            selectors: Some(selectors),
        }) if path == "/matrix"
                && selectors
                    == &vec![
                        ExpressionDatasetSelector::All,
                        ExpressionDatasetSelector::Index(1),
                    ]
    )));
}

#[test]
fn parses_dataset_slices_with_explicit_ranges() {
    let tokens = tokenize_expression("load(/matrix)[2,..10,0] + load(/matrix)[5,5..15,0]").unwrap();
    assert!(tokens.iter().any(|token| matches!(
        token,
        ExpressionToken::LoadRef(ExpressionSeriesRef {
            target: ExpressionObjectTarget::AbsolutePath(path),
            attr_name: None,
            selectors: Some(selectors),
        }) if path == "/matrix"
                && selectors
                    == &vec![
                        ExpressionDatasetSelector::Index(2),
                        ExpressionDatasetSelector::Slice { start: None, end: Some(10) },
                        ExpressionDatasetSelector::Index(0),
                    ]
    )));
    assert!(tokens.iter().any(|token| matches!(
        token,
        ExpressionToken::LoadRef(ExpressionSeriesRef {
            target: ExpressionObjectTarget::AbsolutePath(path),
            attr_name: None,
            selectors: Some(selectors),
        }) if path == "/matrix"
                && selectors
                    == &vec![
                        ExpressionDatasetSelector::Index(5),
                        ExpressionDatasetSelector::Slice { start: Some(5), end: Some(15) },
                        ExpressionDatasetSelector::Index(0),
                    ]
    )));
}

#[test]
fn dataset_path_reference_builds_preview_selection() {
    let dataset_ref = ExpressionSeriesRef {
        target: ExpressionObjectTarget::AbsolutePath("/matrix".to_string()),
        attr_name: None,
        selectors: Some(vec![
            ExpressionDatasetSelector::Index(1),
            ExpressionDatasetSelector::All,
            ExpressionDatasetSelector::Index(2),
            ExpressionDatasetSelector::Index(3),
        ]),
    };
    let selection = dataset_ref
        .to_series_preview_selection(&[4, 5, 6, 7])
        .unwrap();
    assert_eq!(selection.x, 1);
    assert_eq!(selection.index, vec![1, 0, 2, 3]);
}

#[test]
fn dataset_path_reference_builds_preview_selection_from_range_slice() {
    let dataset_ref = ExpressionSeriesRef {
        target: ExpressionObjectTarget::AbsolutePath("/matrix".to_string()),
        attr_name: None,
        selectors: Some(vec![
            ExpressionDatasetSelector::Index(5),
            ExpressionDatasetSelector::Slice {
                start: Some(5),
                end: Some(15),
            },
            ExpressionDatasetSelector::Index(0),
        ]),
    };
    let selection = dataset_ref
        .to_series_preview_selection(&[10, 20, 3])
        .unwrap();
    assert_eq!(selection.x, 1);
    assert_eq!(selection.index, vec![5, 0, 0]);
    assert_eq!(selection.slice, SliceSelection::FromTo(5, 15));
}

#[test]
fn dataset_path_reference_requires_exactly_one_axis_selector() {
    let dataset_ref = ExpressionSeriesRef {
        target: ExpressionObjectTarget::AbsolutePath("/matrix".to_string()),
        attr_name: None,
        selectors: Some(vec![
            ExpressionDatasetSelector::Index(0),
            ExpressionDatasetSelector::Index(1),
        ]),
    };
    let err = dataset_ref
        .to_series_preview_selection(&[3, 4])
        .unwrap_err();
    assert!(err.contains("exactly one slice axis selector"));
}

#[test]
fn current_expression_fragment_keeps_commas_inside_dataset_selectors() {
    let buffer = "load(/matrix)[..,2,0] + $1";
    let cursor = buffer.find(",2").unwrap() + 1;
    let (_, _, fragment) = current_expression_fragment(buffer, cursor).unwrap();
    assert_eq!(fragment, "load(/matrix)[..,2,0]");
}

#[test]
fn consume_expression_reference_fragment_keeps_commas_inside_dataset_selectors() {
    let buffer = "load(/matrix)[5,5..15,0] + $1";
    let chars: Vec<(usize, char)> = buffer.char_indices().collect();
    let end = consume_expression_reference_fragment(buffer, &chars, 0);
    assert_eq!(&buffer[..end], "load(/matrix)[5,5..15,0]");
}

#[test]
fn parses_top_level_xy_expression_tuple() {
    let tokens = tokenize_expression("($1 * 2, $2 + load(/calibration/offset))").unwrap();
    let parsed = parse_derived_expression(&tokens).unwrap();
    match parsed {
        ParsedExpression::XySeries(_, _) => {}
        other => panic!("expected xy parsed expression, got {other:?}"),
    }
}

#[test]
fn normalizes_absolute_expression_paths() {
    assert_eq!(
        normalize_absolute_object_path("/parent/otherds").unwrap(),
        "/parent/otherds"
    );
    assert!(normalize_absolute_object_path("/../../../../").is_err());
}

#[test]
fn rejects_implicit_context_scalar_attributes() {
    let err = tokenize_expression("$1 + load(SCALE)").unwrap_err();
    assert!(err.contains("Data references must use load("));
}

#[test]
fn expression_prompt_edits_do_not_invalidate_chart_render() {
    let mut state = make_state();
    state.modified = false;
    state.open_expression_prompt();
    assert!(state.modified);

    state.modified = false;
    state.expression_insert_char('x');
    assert!(!state.modified);

    state.expression_move_left();
    assert!(!state.modified);

    state.expression_backspace();
    assert!(!state.modified);
}

#[test]
fn expression_prompt_can_defer_cached_image_protocol_frames() {
    let mut state = make_state();
    state.last_chart_area = Some(ratatui::layout::Rect::new(5, 5, 20, 10));
    state.last_chart_panel_area = Some(ratatui::layout::Rect::new(5, 5, 20, 10));
    state.stateful_protocol = Some(state.picker.new_resize_protocol(DynamicImage::ImageRgb8(
        ImageBuffer::<Rgb<u8>, _>::from_pixel(1, 1, Rgb([0, 0, 0])),
    )));
    state.expression_prompt = Some(ExpressionPromptState::new(
        ChartItemId(1),
        String::new(),
        "load(/series)".to_string(),
        "load(/series)".len(),
        ExpressionPromptMode::New,
    ));
    state.modified = false;

    assert!(state.should_defer_image_protocol_frame(ratatui::layout::Rect::new(5, 5, 20, 10)));
    assert!(!state.should_defer_image_protocol_frame(ratatui::layout::Rect::new(5, 5, 21, 10)));
}

#[test]
fn raw_dataset_reference_prompt_message_is_background_loading_hint() {
    let state = make_state();
    let messages = expression_prompt_messages(&state, None, "load(/big_dataset)[266505050]");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].kind, ExpressionPromptMessageKind::Valid);
    assert_eq!(
        messages[0].text,
        "Dataset reference will load in the background when submitted"
    );
}

#[test]
fn suggestion_selection_wraps_within_visible_entries() {
    let mut state = make_state();
    let mut prompt = ExpressionPromptState::new(
        ChartItemId(1),
        String::new(),
        "load(/".to_string(),
        6,
        ExpressionPromptMode::New,
    );
    prompt.suggestions = (0..6)
        .map(|idx| ExpressionPromptSuggestion {
            symbol: String::new(),
            insert_text: format!("load(/path{idx})"),
            label: format!("load(/path{idx})"),
            detail: String::new(),
            kind: ExpressionPromptSuggestionKind::Dataset,
            highlight_spans: Vec::new(),
        })
        .collect();
    prompt.selected_suggestion = Some(EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS - 1);
    state.expression_prompt = Some(prompt);

    state.expression_select_next_suggestion();
    assert_eq!(
        state
            .expression_prompt
            .as_ref()
            .unwrap()
            .selected_suggestion,
        Some(0)
    );

    state.expression_select_prev_suggestion();
    assert_eq!(
        state
            .expression_prompt
            .as_ref()
            .unwrap()
            .selected_suggestion,
        Some(EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS - 1)
    );
}

#[test]
fn suggestion_score_prefers_prefix_and_basename_matches() {
    let prefix =
        expression_suggestion_score("load(/group/series)", "load(/ser", Some("series")).unwrap();
    let fuzzy = expression_suggestion_score(
        "load(/group/alpha_series)",
        "load(/ser",
        Some("alpha_series"),
    )
    .unwrap();
    assert!(prefix > fuzzy);
}

#[test]
fn open_selected_item_for_edit_prefills_existing_expression() {
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
        .open_selected_item_for_edit()
        .expect("open edit prompt");
    let prompt = state.expression_prompt.as_ref().expect("prompt");
    assert_eq!(prompt.buffer, "$1 + 1");
    assert_eq!(
        prompt.mode,
        ExpressionPromptMode::EditExisting(ChartItemId(2))
    );
}

#[test]
fn open_selected_item_for_edit_prefills_existing_name() {
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
        .set_selected_item_name("temperature", Some(ChartItemId(2)))
        .expect("rename item");

    state
        .open_selected_item_for_edit()
        .expect("open edit prompt");
    let prompt = state.expression_prompt.as_ref().expect("prompt");
    assert_eq!(prompt.name_buffer, "temperature");
    assert_eq!(prompt.focus, ExpressionPromptFocus::Expression);
}

#[test]
fn prompt_focus_toggles_between_name_and_expression() {
    let mut state = make_state();
    state.open_expression_prompt();
    state.expression_toggle_focus();
    state.expression_insert_char('t');
    state.expression_insert_char('1');
    state.expression_toggle_focus();
    state.expression_insert_char('$');

    let prompt = state.expression_prompt.as_ref().expect("prompt");
    assert_eq!(prompt.name_buffer, "t1");
    assert_eq!(prompt.buffer, "$");
    assert_eq!(prompt.focus, ExpressionPromptFocus::Expression);
}

#[test]
fn named_series_can_be_referenced_by_name() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0, 0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(source("/group/a", selection), vec![(0.0, 1.0), (1.0, 2.0)]);
    state
        .set_selected_item_name("temperature", Some(ChartItemId(1)))
        .expect("rename item");

    let derived_id = state
        .create_expression_derived("$temperature + 1".to_string())
        .expect("create derived");
    let derived = state
        .item_by_id(derived_id)
        .expect("derived item should exist");

    assert_eq!(derived.series.points, vec![(0.0, 2.0), (1.0, 3.0)]);
}

#[test]
fn duplicate_series_names_are_rejected() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0, 0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(
        source("/group/a", selection.clone()),
        vec![(0.0, 1.0), (1.0, 2.0)],
    );
    state
        .set_selected_item_name("temperature", Some(ChartItemId(1)))
        .expect("rename first item");
    state.add_chart_item(source("/group/b", selection), vec![(0.0, 3.0), (1.0, 4.0)]);

    let err = state
        .set_selected_item_name("temperature", Some(ChartItemId(2)))
        .unwrap_err();
    assert!(err.contains("already in use"));
}

#[test]
fn recursive_name_assignment_is_rejected() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(source("/group/a", selection), vec![(0.0, 1.0), (1.0, 2.0)]);
    let id = state
        .create_expression_derived("$future + 1".to_string())
        .expect("invalid expressions are persisted as drafts");

    let err = state
        .set_selected_item_name("future", Some(id))
        .unwrap_err();
    assert!(err.contains("references $future"));
}

#[test]
fn click_item_hitbox_selects_item() {
    let mut state = make_state();
    state.add_chart_item(
        source(
            "/group/a",
            PreviewSelection {
                index: vec![0, 0],
                x: 0,
                slice: SliceSelection::All,
            },
        ),
        vec![(0.0, 1.0), (1.0, 2.0)],
    );
    state.add_chart_item(
        source(
            "/group/b",
            PreviewSelection {
                index: vec![0, 0],
                x: 0,
                slice: SliceSelection::All,
            },
        ),
        vec![(0.0, 3.0), (1.0, 4.0)],
    );
    state.item_hitboxes = vec![
        MultiChartItemHitbox {
            area: ratatui::layout::Rect::new(0, 0, 10, 2),
            index: 0,
        },
        MultiChartItemHitbox {
            area: ratatui::layout::Rect::new(0, 2, 10, 2),
            index: 1,
        },
    ];

    assert!(state.click_item_hitbox(1, 3));
    assert_eq!(state.idx, 1);
}

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

    assert_eq!(state.chart_panel_title(), "Overlay chart [x values]");

    state.viewport = Some(ChartViewport {
        x_min: 2.0,
        x_max: 6.0,
        y_min: 4.0,
        y_max: 12.0,
    });
    assert_eq!(
        state.chart_panel_title(),
        "Overlay chart [x values] · view x=[2.0000, 6.0000] y=[4.0000, 12.0000]"
    );
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
fn clearing_zoom_discards_detail_series() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0],
        x: 0,
        slice: SliceSelection::All,
    };
    state
        .add_chart_item(
            source("/group/a", selection),
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
