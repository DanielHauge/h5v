use super::eval::normalize_absolute_object_path;
use super::expression::{ExpressionDatasetSelector, ExpressionScalarRef, ExpressionSeriesRef};
use super::prompt::{
    consume_expression_reference_fragment, current_expression_fragment, expression_prompt_messages,
    expression_suggestion_score, ExpressionPromptFocus,
};
use super::*;
use std::{
    sync::mpsc::channel,
    time::{SystemTime, UNIX_EPOCH},
};

use hdf5_metno::File;
use image::{DynamicImage, ImageBuffer, Rgb};
use ndarray::Array;

#[allow(deprecated)]
pub(super) fn make_state() -> MultiChartState {
    let (tx_load, _rx_load) = channel();
    let (tx_render, _rx_render) = channel();
    let (tx_expression_refresh, _rx_expression_refresh) = channel();
    MultiChartState::new(
        Picker::from_fontsize((7, 14)),
        tx_load,
        tx_render,
        tx_expression_refresh,
    )
}

pub(super) fn source(path: &str, selection: PreviewSelection) -> ChartSource {
    ChartSource::DatasetSelection(DatasetChartSource {
        dataset_path: "/raw/ds".to_string(),
        display_path: path.to_string(),
        selection,
        shape: vec![4, 8],
        kind: DatasetChartKind::Dataset,
    })
}

pub(super) fn assert_viewport(state: &MultiChartState, expected: Option<(f64, f64, f64, f64)>) {
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

pub(super) fn make_attribute_test_file() -> (File, std::path::PathBuf) {
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

pub(super) fn make_dataset_ref_test_file() -> (File, std::path::PathBuf) {
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
fn current_expression_fragment_supports_function_identifiers() {
    let buffer = "rolling_me";
    let (_, _, fragment) = current_expression_fragment(buffer, buffer.len()).unwrap();
    assert_eq!(fragment, "rolling_me");
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
fn expression_prompt_handles_unicode_cursor_editing() {
    let mut state = make_state();
    state.open_expression_prompt();
    state.expression_toggle_focus();
    state.expression_insert_char('é');
    state.expression_insert_char('x');
    state.expression_move_left();
    state.expression_backspace();
    state.expression_delete();
    state.expression_set_focus_cursor(ExpressionPromptFocus::Name, 1);
    state.expression_insert_char('ø');

    state.expression_toggle_focus();
    state.expression_insert_char('é');
    state.expression_insert_char('x');
    state.expression_move_left();
    state.expression_backspace();
    state.expression_delete();
    state.expression_set_focus_cursor(ExpressionPromptFocus::Expression, 1);
    state.expression_insert_char('ø');

    let prompt = state.expression_prompt.as_ref().expect("prompt");
    assert_eq!(prompt.name_buffer, "ø");
    assert_eq!(prompt.buffer, "ø");
    assert_eq!(prompt.name_cursor, "ø".len());
    assert_eq!(prompt.cursor, "ø".len());
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
fn expression_prompt_suggests_registry_functions() {
    let mut state = make_state();
    state.expression_prompt = Some(ExpressionPromptState::new(
        ChartItemId(1),
        String::new(),
        "rolling_me".to_string(),
        "rolling_me".len(),
        ExpressionPromptMode::New,
    ));

    state.refresh_expression_prompt(None);

    let prompt = state.expression_prompt.as_ref().expect("prompt");
    assert!(prompt.suggestions.iter().any(|suggestion| suggestion.kind
        == ExpressionPromptSuggestionKind::Function
        && suggestion.insert_text == "rolling_mean($1, 16)"));
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
fn reorder_selected_item_moves_it_up_and_keeps_it_selected() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0, 0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(source("/group/a", selection.clone()), vec![(0.0, 1.0)]);
    state.add_chart_item(source("/group/b", selection.clone()), vec![(0.0, 2.0)]);
    state.add_chart_item(source("/group/c", selection), vec![(0.0, 3.0)]);
    state.idx = 1;

    assert!(state.reorder_selected_up());
    assert_eq!(state.idx, 0);
    assert_eq!(state.items[0].label, "b[..,0]");
    assert_eq!(state.items[1].label, "a[..,0]");
}

#[test]
fn reorder_selected_item_moves_it_down_and_keeps_it_selected() {
    let mut state = make_state();
    let selection = PreviewSelection {
        index: vec![0, 0],
        x: 0,
        slice: SliceSelection::All,
    };
    state.add_chart_item(source("/group/a", selection.clone()), vec![(0.0, 1.0)]);
    state.add_chart_item(source("/group/b", selection.clone()), vec![(0.0, 2.0)]);
    state.add_chart_item(source("/group/c", selection), vec![(0.0, 3.0)]);
    state.idx = 1;

    assert!(state.reorder_selected_down());
    assert_eq!(state.idx, 2);
    assert_eq!(state.items[1].label, "c[..,0]");
    assert_eq!(state.items[2].label, "b[..,0]");
}

#[test]
fn click_view_mode_hitbox_switches_modes() {
    let mut state = make_state();
    state.view_mode_hitboxes = vec![
        MultiChartViewModeHitbox {
            area: ratatui::layout::Rect::new(0, 0, 8, 1),
            mode: MultiChartViewMode::Line,
        },
        MultiChartViewModeHitbox {
            area: ratatui::layout::Rect::new(10, 0, 12, 1),
            mode: MultiChartViewMode::Histogram,
        },
    ];

    assert!(state.click_view_mode_hitbox(11, 0));
    assert_eq!(state.view_mode(), MultiChartViewMode::Histogram);
}
