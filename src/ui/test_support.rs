use std::{cell::RefCell, rc::Rc, str::FromStr, sync::mpsc::channel, sync::Arc, sync::RwLock};

use hdf5_metno::{types::TypeDescriptor, Dataset, H5Type};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

use crate::h5f::{
    DatasetHandle, DatasetIdentity, DatasetMetaState, H5FNode, Node, ReadOpenMode,
    RequestedOpenMode, ResolvedOpenMode,
};

use super::{command::CommandState, mchart::MultiChartState, state::*};

fn disconnected_sender<T>() -> std::sync::mpsc::Sender<T> {
    channel().0
}

/// Renderer-only state: no startup configuration, terminal queries, or worker threads.
pub(super) fn renderer_state() -> AppState<'static> {
    AppState {
        readonly: true,
        root: Rc::new(RefCell::new(H5FNode::new(Node::Broken("test".into())))),
        tree_load_tx: disconnected_sender(),
        navigation_load_tx: disconnected_sender(),
        content_preview_tx: disconnected_sender(),
        content_generation: 0,
        navigation_generation: 0,
        next_navigation_request_id: 0,
        pending_navigation_request: None,
        tree_load_generation: 0,
        next_tree_load_request_id: 0,
        pending_tree_loads: vec![],
        pending_tree_expansions: vec![],
        pending_tree_selection: None,
        pending_tree_selection_state: None,
        pending_tree_attribute_selection: None,
        treeview: vec![],
        file: None,
        requested_open_mode: RequestedOpenMode::Read(ReadOpenMode::Standard),
        resolved_open_mode: ResolvedOpenMode::ReadOnly,
        snapshot_file: None,
        editing: false,
        edit_pause: Arc::new(RwLock::new(())),
        tree_view_cursor: 0,
        clipboard: None,
        clipboard_init_error: None,
        copying: false,
        toast: AppToast::Empty,
        toast_expires_at: None,
        configuration_warning: None,
        file_watch: FileWatchState {
            path: String::new(),
            linked: false,
            last_known_modified: None,
            pending_external_change: false,
        },
        compatibility_mode: true,
        focus: Focus::Content,
        multi_chart: MultiChartState::new(
            ratatui_image::picker::Picker::halfblocks(),
            disconnected_sender(),
            disconnected_sender(),
            disconnected_sender(),
        ),
        mode: Mode::Normal,
        command_return_mode: Mode::Normal,
        help_return_mode: Mode::Normal,
        logs_return_mode: Mode::Normal,
        searcher: None,
        help: HelpViewState::default(),
        logs: LogsViewState::default(),
        pending_chord: None,
        binding_command_depth: 0,
        show_tree_view: false,
        stacked_tree_layout: false,
        image_protocol_enabled: false,
        image_cell_size: (8, 16),
        preview_debounce_generation: 0,
        preview_debounce_until: None,
        preview_debounce_path: None,
        content_mode: ContentShowMode::Preview.handle(),
        img_state: ImgState {
            protocol: None,
            tx_resize_img: disconnected_sender(),
            tx_load_imgfs: disconnected_sender(),
            tx_load_imgfsvlen: disconnected_sender(),
            tx_load_img: disconnected_sender(),
            ds: None,
            current_key: None,
            clipboard_image: None,
            window: None,
            error: None,
            idx_to_load: 0,
            idx_loaded: -1,
            cached_images: Default::default(),
            pending_keys: Default::default(),
        },
        matrix_view_state: MatrixViewState {
            col_offset: 0,
            row_offset: 0,
            rows_currently_available: 0,
            cols_currently_available: 0,
            cursor_row: 0,
            cursor_col: 0,
        },
        heatmap_viewport_region: None,
        heatmap_region: None,
        heatmap_render: HeatmapRenderState {
            current_key: None,
            current_selection: None,
            current_line_profile: None,
            current_legend_summary: None,
            current_slice_summary: None,
            viewport: None,
            selected_cells: None,
            selected_line: None,
            drag_state: None,
            page_window: None,
            cached_pages: Default::default(),
            pending_keys: Default::default(),
            tx_load_heatmap: disconnected_sender(),
            settings: HeatmapSettings::default(),
            selected_setting: 0,
            session_range_modes: vec![],
        },
        chart_preview_state: ChartPreviwState {
            mode: PreviewChartMode::Line,
            x_axis_scale: super::mchart::ChartAxisScale::Linear,
            y_axis_scale: super::mchart::ChartAxisScale::Linear,
            ds_loaded: None,
            protocol: None,
            clipboard_image: None,
            error: None,
            ds_selection: None,
            rendered_mode: None,
            rendered_viewport: None,
            rendered_roi: None,
            rendered_size: None,
            pending_key: None,
            tx_resize_chartpreview: disconnected_sender(),
            tx_load_chartpreview: disconnected_sender(),
            cached_previews: Default::default(),
            viewport: None,
            data_bounds: None,
            current_data: None,
            roi: None,
            histogram_selection: None,
            histogram_range: None,
            histogram_history: vec![None],
            histogram_history_index: 0,
            last_chart_area: None,
            last_plot_area: None,
            drag_state: None,
        },
        preview_expression_state: PreviewExpressionState {
            current_key: None,
            pending_key: None,
            data_preview: None,
            error: None,
            tx_load: disconnected_sender(),
        },
        content_preview_state: ContentPreviewState {
            pending_key: None,
            error: None,
            cached: Default::default(),
            tx_load: disconnected_sender(),
        },
        matrix_viewport_state: MatrixViewportState {
            pending_key: None,
            error: None,
            cached: Default::default(),
            tx_load: disconnected_sender(),
        },
        page_state: PageState {
            idx: 0,
            paged: PageType::Unpaged,
            page_count: 0,
        },
        command_state: CommandState {
            command_buffer: String::new(),
            last_command: None,
            cursor: 0,
            selected_suggestion: 0,
            history: Default::default(),
            history_cursor: None,
            history_draft: None,
        },
        attribute_create_dialog: None,
        attribute_delete_dialog: None,
        fixed_string_overflow_dialog: None,
        ui_layout: UiLayoutState::default(),
    }
}

#[allow(clippy::expect_used)]
pub(super) fn dataset_node(dataset: &Dataset) -> H5FNode {
    let mut node = H5FNode::new(Node::Dataset(
        DatasetHandle::Loaded(dataset.clone()),
        DatasetMetaState::Pending(DatasetIdentity {
            display_name: dataset.name(),
            is_link: false,
            link_name: None,
            path: dataset.name(),
            is_compound_container: false,
        }),
    ));
    node.ensure_dataset_meta().expect("load renderer metadata");
    node
}

#[allow(clippy::expect_used)]
pub(super) fn projected_flag_node(dataset: &Dataset) -> H5FNode {
    let mut root = dataset_node(dataset);
    root.ensure_expanded().expect("expand compound fields");
    let child = root.children[0].borrow();
    H5FNode::new(child.node.clone())
}

#[derive(Clone, Copy)]
#[repr(C)]
pub(super) struct BooleanRecord {
    pub flag: bool,
    pub count: u8,
}

// The descriptor uses the actual C-layout offsets of these two scalar fields.
unsafe impl H5Type for BooleanRecord {
    fn type_descriptor() -> TypeDescriptor {
        use hdf5_metno::types::{CompoundField, CompoundType, IntSize};
        TypeDescriptor::Compound(CompoundType {
            fields: vec![
                CompoundField::new(
                    "flag",
                    TypeDescriptor::Boolean,
                    std::mem::offset_of!(Self, flag),
                    0,
                ),
                CompoundField::new(
                    "count",
                    TypeDescriptor::Unsigned(IntSize::U1),
                    std::mem::offset_of!(Self, count),
                    1,
                ),
            ],
            size: std::mem::size_of::<Self>(),
        })
    }
}

#[allow(clippy::expect_used)]
pub(super) fn mark_imported_boolean(dataset: &Dataset) {
    let tag = hdf5_metno::types::VarLenUnicode::from_str("bool").expect("boolean type tag");
    dataset
        .new_attr::<hdf5_metno::types::VarLenUnicode>()
        .shape(())
        .create("H5V_INFERRED_TYPE")
        .expect("create imported type tag")
        .write_scalar(&tag)
        .expect("write imported type tag");
}

#[allow(clippy::expect_used)]
pub(super) fn assert_text_color(buffer: &Buffer, area: Rect, text: &str, color: Color) {
    for y in area.y..area.bottom() {
        let row = (area.x..area.right())
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>();
        if let Some(offset) = row.find(text) {
            let offset = row[..offset].chars().count();
            for x in area.x + offset as u16..area.x + offset as u16 + text.len() as u16 {
                assert_eq!(buffer[(x, y)].fg, color, "foreground for {text}");
                assert_eq!(
                    buffer[(x, y)]
                        .modifier
                        .contains(ratatui::style::Modifier::BOLD),
                    crate::configure::prefers_strong_text(),
                    "strong-text preference for {text}",
                );
            }
            return;
        }
    }
    panic!("missing {text:?} in {area:?}");
}
