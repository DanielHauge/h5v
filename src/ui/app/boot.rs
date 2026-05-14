use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Arc, RwLock,
};

use arboard::Clipboard;
use image::Rgba;
use ratatui_image::picker::{Picker, ProtocolType};

use crate::{
    compat::RuntimeConfig,
    configure,
    configure::run_lua_engine,
    error::AppError,
    h5f,
    ui::command::CommandState,
    ui::{
        heatmap::handle_heatmap_load,
        mchart::MultiChartState,
        preview::image::{
            handle_chartpreview_load, handle_chartpreview_resize, handle_image_load,
            handle_image_resize, handle_imagefs_load, handle_imagefsvlen_load,
        },
        state::{
            self, AppState, AppToast, ChartPreviwState, ContentShowMode, FileWatchState, Focus,
            ImgState, LastFocused, MatrixViewState, Mode,
        },
    },
};

use super::{
    config::{configuration_warning_message, log_configuration_error},
    AppEvent,
};

type Result<T> = std::result::Result<T, AppError>;

pub(super) struct PreparedApp<'a> {
    pub(super) state: AppState<'a>,
    pub(super) tx_events: Sender<AppEvent>,
    pub(super) rx_events: Receiver<AppEvent>,
}

pub(super) fn prepare_app<'a>(
    filename: &str,
    link: bool,
    writable: bool,
    runtime_config: RuntimeConfig,
) -> Result<PreparedApp<'a>> {
    let h5f = h5f::H5F::open(filename.to_string(), link, writable).map_err(|error| {
        AppError::Hdf5(hdf5_metno::Error::from(format!(
            "Failed to open HDF5 file: {}",
            error
        )))
    })?;

    let (tx_events, rx_events) = channel();
    let startup_config_error = run_lua_engine(tx_events.clone(), runtime_config.compatibility_mode)
        .err()
        .map(|error| {
            log_configuration_error(&error);
            configuration_warning_message(&error, false)
        });

    #[allow(deprecated)]
    let mut picker = if runtime_config.terminal_graphics {
        Picker::from_query_stdio().unwrap_or(Picker::halfblocks())
    } else {
        Picker::halfblocks()
    };
    let (bg_r, bg_g, bg_b) =
        configure::rgb_channels(configure::themed_color(|colors| colors.surface.bg));
    picker.set_background_color(Rgba([bg_r, bg_g, bg_b, 255]));
    let image_cell_size = picker.font_size();
    let tx_resize = tx_events.clone();
    let tx_resize_img = handle_image_resize(tx_resize);
    let tx_load_imgfs =
        handle_imagefs_load(tx_events.clone(), tx_resize_img.clone(), picker.clone());
    let tx_load_imgfsvlen =
        handle_imagefsvlen_load(tx_events.clone(), tx_resize_img.clone(), picker.clone());
    let tx_load_img = handle_image_load(tx_events.clone(), tx_resize_img.clone(), picker.clone());
    let tx_chart_preview_resize = handle_chartpreview_resize(tx_events.clone());
    let tx_load_chartpreview =
        handle_chartpreview_load(tx_events.clone(), tx_chart_preview_resize, picker.clone());
    let tx_load_heatmap = handle_heatmap_load(tx_events.clone());

    let img_state = ImgState {
        protocol: None,
        tx_resize_img,
        tx_load_imgfs,
        tx_load_imgfsvlen,
        tx_load_img,
        ds: None,
        current_key: None,
        clipboard_image: None,
        window: None,
        idx_to_load: 0,
        idx_loaded: -1,
        error: None,
        cached_images: Default::default(),
        pending_keys: Default::default(),
    };

    let chart_preview_state = ChartPreviwState {
        ds_loaded: None,
        protocol: None,
        clipboard_image: None,
        error: None,
        ds_selection: None,
        pending_key: None,
        tx_load_chartpreview,
    };

    let matrix_view_state = MatrixViewState {
        col_offset: 0,
        row_offset: 0,
        rows_currently_available: 0,
        cols_currently_available: 0,
        cursor_row: 0,
        cursor_col: 0,
    };
    let (clipboard, clipboard_init_error) = match Clipboard::new() {
        Ok(clipboard) => (Some(clipboard), None),
        Err(error) => (None, Some(error.to_string())),
    };

    let segment_state = state::SegmentState {
        idx: 0,
        segment_count: 0,
        segumented: state::SegmentType::NoSegment,
    };

    let command_state = CommandState {
        command_buffer: String::new(),
        last_command: None,
        cursor: 0,
        selected_suggestion: 0,
        history: Default::default(),
        history_cursor: None,
        history_draft: None,
    };
    let edit_pause = Arc::new(RwLock::new(()));

    let root_node = h5f.root.clone();
    let mut state = AppState {
        readonly: !writable,
        root: root_node,
        editing: false,
        file: Some(h5f.file),
        toast: AppToast::Empty,
        configuration_warning: startup_config_error.clone(),
        file_watch: FileWatchState {
            path: filename.to_string(),
            linked: link,
            last_known_modified: None,
            pending_external_change: false,
        },
        compatibility_mode: runtime_config.compatibility_mode,
        multi_chart: MultiChartState::new(picker.clone()),
        segment_state,
        edit_pause: edit_pause.clone(),
        command_state,
        attribute_create_dialog: None,
        attribute_delete_dialog: None,
        fixed_string_overflow_dialog: None,
        treeview: vec![],
        tree_view_cursor: 0,
        focus: Focus::Tree(LastFocused::Attributes),
        clipboard,
        clipboard_init_error,
        mode: Mode::Normal,
        command_return_mode: Mode::Normal,
        help_return_mode: Mode::Normal,
        copying: false,
        searcher: None,
        help: state::HelpViewState::default(),
        pending_chord: None,
        binding_command_depth: 0,
        show_tree_view: true,
        stacked_tree_layout: false,
        image_protocol_enabled: picker.protocol_type() != ProtocolType::Halfblocks,
        image_cell_size,
        preview_debounce_generation: 0,
        preview_debounce_until: None,
        preview_debounce_path: None,
        content_mode: configure::current_content_mode_order()
            .first()
            .copied()
            .unwrap_or(ContentShowMode::Preview),
        img_state,
        matrix_view_state,
        heatmap_viewport_region: None,
        heatmap_region: None,
        heatmap_render: state::HeatmapRenderState {
            current_key: None,
            current_selection: None,
            current_line_profile: None,
            current_legend_summary: None,
            current_slice_summary: None,
            viewport: None,
            selected_cells: None,
            selected_line: None,
            drag_state: None,
            segment: None,
            cached_pages: Default::default(),
            pending_keys: Default::default(),
            tx_load_heatmap,
            settings: configure::current_heatmap_default_settings(),
            selected_setting: 0,
            session_range_modes: Vec::new(),
        },
        chart_preview_state,
        ui_layout: state::UiLayoutState::default(),
    };
    if let Some(message) = startup_config_error {
        state.toast = AppToast::Warning(message);
    }
    state.sync_heatmap_configuration();
    state.sync_file_watch();
    state.compute_tree_view();

    Ok(PreparedApp {
        state,
        tx_events,
        rx_events,
    })
}
