use std::{
    env,
    io::stdout,
    rc::Rc,
    sync::{mpsc::channel, Arc, RwLock},
    thread,
    time::SystemTime,
};

use arboard::Clipboard;
use image::Rgba;
use ratatui::{
    crossterm::{
        cursor::{Hide, Show},
        event::{self, DisableMouseCapture, EnableMouseCapture},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    },
    layout::{Alignment, Constraint, Layout, Rect},
    prelude::CrosstermBackend,
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use ratatui_image::picker::{Picker, ProtocolType};

use crate::{
    compat::RuntimeConfig,
    configure,
    configure::run_lua_engine,
    error::{log_error, AppError},
    h5f,
    ui::{
        heatmap::{handle_heatmap_load, HEATMAP_CACHE_CAPACITY},
        input::EventResult,
        mchart::MultiChartState,
        preview::image::{handle_chartpreview_load, handle_chartpreview_resize},
        state::{AppToast, ChartPreviwState, FileWatchState},
    },
    GIT_VERSION,
};

use self::config::{
    configuration_warning_message, log_configuration_error, open_configuration_and_reload,
    should_use_alternate_screen,
};
use self::dialogs::{
    render_attribute_create_dialog, render_attribute_delete_dialog,
    render_fixed_string_overflow_dialog, render_fixed_string_resize_dialog,
};
use self::events::{handle_file_watch_events, handle_term_events, schedule_preview_debounce};
use self::reload::reload_current_file;
use self::update::check_for_available_update;
use super::state::{ChartPreviewKey, HeatmapLoadedPage, HeatmapRenderKey, ImageLoadKey};
use super::{
    command::{
        execute_command, parse_command_text, render_command_dialog, CommandState, StartupCommand,
    },
    help::render_help,
    input::handle_input_event,
    main_display::render_main_display,
    preview::image::{
        handle_image_load, handle_image_resize, handle_imagefs_load, handle_imagefsvlen_load,
        ImageResizeResult,
    },
    state::{self, AppState, ContentShowMode, Focus, ImgState, LastFocused, MatrixViewState, Mode},
    tree_view::render_tree,
};

mod config;
mod dialogs;
mod events;
mod reload;
mod update;

pub(super) fn primary_text_style() -> Style {
    let mut style = Style::default().fg(configure::themed_color(|colors| colors.text.primary));
    if configure::prefers_strong_text() {
        style = style.bold();
    }
    style
}

fn make_panels_rect(area: Rect, mode: Mode) -> Rc<[Rect]> {
    if let Mode::Search = mode {
        Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(100), Constraint::Percentage(0)])
            .split(area)
    } else {
        if area.width < 100 {
            let chunks = Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(area);
            return chunks;
        }

        Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area)
    }
}

fn use_stacked_tree_layout(area: Rect, mode: &Mode, show_tree_view: bool) -> bool {
    show_tree_view && !matches!(mode, Mode::Search) && area.width < 100
}

type Result<T> = std::result::Result<T, AppError>;

pub struct IntendedMainLoopBreak {}

const HEADER_HEIGHT: u16 = 1;
const COMMAND_BAR_HEIGHT: u16 = 6;
pub fn init(
    filename: String,
    link: bool,
    writable: bool,
    runtime_config: RuntimeConfig,
    startup_commands: &[StartupCommand],
) -> Result<()> {
    let use_alternate_screen =
        should_use_alternate_screen(runtime_config, env::var("CROS_CONTAINER").ok().as_deref());

    if use_alternate_screen {
        stdout().execute(EnterAlternateScreen)?;
    }
    stdout().execute(EnableMouseCapture)?;
    stdout().execute(Hide)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let new_ver = check_for_available_update(SystemTime::now());

    let mut last_message = None;

    loop {
        match main_recover_loop(
            &mut terminal,
            filename.clone(),
            link,
            writable,
            runtime_config,
            startup_commands,
            new_ver.as_deref(),
        ) {
            Ok(_) => break,
            Err(e) => match e {
                AppError::FileError(_) => {
                    last_message = Some("No files given error".to_string());
                }
                AppError::Io(error) => {
                    last_message = Some(format!("IO Error: - {error}"));
                }
                AppError::Hdf5(error) => match error {
                    hdf5_metno::Error::HDF5(_) => {
                        last_message = Some("HDF5 Error".to_string());
                        break;
                    }
                    hdf5_metno::Error::Internal(e) => {
                        last_message = Some(format!("HDF5 Internal: - {e}"));
                        break;
                    }
                },
                AppError::ChannelError(c) => last_message = Some(format!("Channel Error: - {c}")),
                AppError::ClipboardError(msg) => {
                    last_message = Some(format!("Clipboard Error: - {msg}"));
                    break;
                }
                AppError::InvalidCommand(cmd) => {
                    last_message = Some(format!("Invalid Command: - {cmd}"));
                    break;
                }
                AppError::EditError(e) => {
                    last_message = Some(format!("Edit Error: - {e}"));
                    break;
                }
                AppError::EditWarning(e) => {
                    last_message = Some(format!("Edit Warning: - {e}"));
                    break;
                }
                AppError::FixedStringOverflow(e) => {
                    last_message = Some(format!("Edit Error: - {e}"));
                    break;
                }
                AppError::ChildNotFound(e) => {
                    last_message = Some(format!("Child not found: - {e}"));
                    break;
                }
                AppError::PoisionedLockError(e) => {
                    last_message = Some(format!("Poisioned lock error: - {e}"));
                    break;
                }
                AppError::DrawingError(e) => {
                    last_message = Some(format!("Drawing error: - {e}"));
                    break;
                }
                AppError::LuaError(e) => {
                    last_message = Some(format!("Lua error: - {e}"));
                    break;
                }
            },
        }
    }

    stdout().execute(Show)?;
    stdout().execute(DisableMouseCapture)?;
    if use_alternate_screen {
        stdout().execute(LeaveAlternateScreen)?;
    }
    disable_raw_mode()?;
    if let Some(message) = last_message {
        eprintln!("Unrecoverable AppError: {}", message);
    }
    Ok(())
}

fn main_recover_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    filename: String,
    link: bool,
    writable: bool,
    runtime_config: RuntimeConfig,
    startup_commands: &[StartupCommand],
    new_version: Option<&str>,
) -> Result<IntendedMainLoopBreak> {
    let h5f = h5f::H5F::open(filename.clone(), link, writable).map_err(|e| {
        AppError::Hdf5(hdf5_metno::Error::from(format!(
            "Failed to open HDF5 file: {}",
            e
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
    let tx_events_2 = tx_events.clone();
    let tx_load_img = handle_image_resize(tx_events_2);
    let tx_load_imgfs = handle_imagefs_load(tx_events.clone(), tx_load_img.clone(), picker.clone());
    let tx_load_imgfsvlen =
        handle_imagefsvlen_load(tx_events.clone(), tx_load_img.clone(), picker.clone());
    let tx_load_img = handle_image_load(tx_events.clone(), tx_load_img.clone(), picker.clone());
    let tx_chart_preview_resize = handle_chartpreview_resize(tx_events.clone());
    let tx_load_chartpreview =
        handle_chartpreview_load(tx_events.clone(), tx_chart_preview_resize, picker.clone());
    let tx_load_heatmap = handle_heatmap_load(tx_events.clone());

    let img_state = ImgState {
        protocol: None,
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
    };

    let chart_preview_state = ChartPreviwState {
        ds_loaded: None,
        protocol: None,
        clipboard_image: None,
        error: None,
        ds_selection: None,
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
            path: filename.clone(),
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
            current_slice_summary: None,
            viewport: None,
            selected_cells: None,
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

    if run_startup_commands(&mut state, startup_commands)? {
        return Ok(IntendedMainLoopBreak {});
    }

    let draw_closure = |frame: &mut Frame, state: &mut AppState| {
        let command_over_multichart = matches!(state.mode, Mode::Command)
            && matches!(state.command_return_mode, Mode::MultiChart);
        let frame_area = match state.toast {
            AppToast::Empty => frame.area(),
            AppToast::Info(_) | AppToast::Warning(_) | AppToast::Error(_) => {
                split_render_toast(frame, state)
            }
        };
        let content_area = render_header(frame, frame_area, state, new_version);
        let (content_area, command_area) = match state.mode {
            Mode::Command => split_command_bar(content_area),
            _ => (content_area, Rect::new(0, 0, 0, 0)),
        };
        state.ui_layout = state::UiLayoutState::default();

        if let Mode::Help = state.mode {
            render_help(frame, content_area, state);
            return;
        }
        if matches!(state.mode, Mode::MultiChart) || command_over_multichart {
            state.multi_chart.render(frame, content_area);
            if matches!(state.mode, Mode::Command) {
                render_command_dialog(frame, command_area, state);
            }
            return;
        }

        let show_tree_view = state.show_tree_view;
        state.stacked_tree_layout =
            use_stacked_tree_layout(content_area, &state.mode, state.show_tree_view);

        let main_display_area = match show_tree_view {
            true => {
                let areas = make_panels_rect(content_area, state.mode.clone());
                let (tree_area, main_display_area) = (areas[0], areas[1]);
                render_tree(frame, tree_area, state);
                main_display_area
            }
            false => content_area,
        };

        match state.mode {
            Mode::Search => {}
            Mode::Command
            | Mode::Normal
            | Mode::AttributeCreateDialog
            | Mode::AttributeDeleteDialog
            | Mode::FixedStringOverflowDialog
            | Mode::FixedStringResizeDialog => {
                let selected_node = state.treeview[state.tree_view_cursor].node.clone();
                match render_main_display(frame, &main_display_area, &selected_node, state) {
                    Ok(()) => {}
                    Err(e) => render_error(frame, &format!("Error: {}", e)),
                }
            }
            Mode::Help => {}       // already handled above,
            Mode::MultiChart => {} // already handled above,
        }

        match state.mode {
            Mode::Command => render_command_dialog(frame, command_area, state),
            Mode::AttributeCreateDialog => {
                render_attribute_create_dialog(frame, content_area, state)
            }
            Mode::AttributeDeleteDialog => {
                render_attribute_delete_dialog(frame, content_area, state)
            }
            Mode::FixedStringOverflowDialog => {
                render_fixed_string_overflow_dialog(frame, content_area, state)
            }
            Mode::FixedStringResizeDialog => {
                render_fixed_string_resize_dialog(frame, content_area, state)
            }
            _ => {}
        }
    };

    // First time draw nice state
    terminal.draw(|f| draw_closure(f, &mut state))?;

    handle_term_events(tx_events.clone(), edit_pause);
    handle_file_watch_events(tx_events.clone(), state.file_watch.path.clone());

    loop {
        let event = rx_events.recv();
        let event = match event {
            Ok(event) => event,
            Err(error) => {
                log_error(error);
                return Err(AppError::ChannelError(format!(
                    "Failed to receive event from channel: {}",
                    error
                )));
            }
        };
        if state.editing {
            continue;
        }

        match event {
            AppEvent::Toast(toast) => {
                state.toast = toast;
                terminal.draw(|f| {
                    draw_closure(f, &mut state);
                })?;
            }
            AppEvent::TermEvent(event) => {
                let selected_before = state.selected_tree_path();
                let event_result = handle_input_event(&mut state, event)
                    .unwrap_or_else(|e| EventResult::Toast(AppToast::Error(e.to_string()), false));
                let selected_after = state.selected_tree_path();
                if selected_before != selected_after {
                    if let Some(path) = selected_after {
                        let generation = state.begin_preview_debounce(path);
                        schedule_preview_debounce(tx_events.clone(), generation);
                    } else {
                        state.clear_preview_debounce();
                    }
                }
                match event_result {
                    EventResult::Quit => break,
                    EventResult::Continue => {}
                    EventResult::Redraw => {
                        state.toast = AppToast::Empty;
                        terminal.draw(|f| {
                            draw_closure(f, &mut state);
                        })?;
                    }
                    EventResult::Copying => {
                        state.toast = AppToast::Empty;
                        state.copying = true;
                        terminal.draw(|f| {
                            draw_closure(f, &mut state);
                        })?;
                        state.copying = false;
                        thread::sleep(std::time::Duration::from_millis(100));
                        terminal.draw(|f| {
                            draw_closure(f, &mut state);
                        })?;
                    }
                    EventResult::ReloadFile { write } => {
                        match reload_current_file(&mut state, write) {
                            Ok(message) => {
                                terminal.clear()?;
                                terminal.flush()?;
                                state.toast = AppToast::Info(message);
                            }
                            Err(error) => {
                                state.toast = AppToast::Error(error.to_string());
                            }
                        }
                        terminal.draw(|f| {
                            draw_closure(f, &mut state);
                        })?;
                    }
                    EventResult::Configure { reset } => {
                        match open_configuration_and_reload(&mut state, tx_events.clone(), reset) {
                            Ok(toast) => {
                                terminal.clear()?;
                                terminal.flush()?;
                                state.toast = toast;
                            }
                            Err(error) => {
                                state.toast = AppToast::Error(error.to_string());
                            }
                        }
                        terminal.draw(|f| {
                            draw_closure(f, &mut state);
                        })?;
                    }
                    EventResult::Error(e) => {
                        terminal.draw(|f| {
                            render_error(f, &e);
                        })?;
                        thread::sleep(std::time::Duration::from_secs(2));
                        terminal.draw(|f| {
                            draw_closure(f, &mut state);
                        })?;
                    }
                    EventResult::Toast(toast, full_redraw) => {
                        if full_redraw {
                            state.compute_tree_view();
                            terminal.clear()?;
                            terminal.flush()?;
                        }
                        state.toast = toast;
                        terminal.draw(|f| {
                            draw_closure(f, &mut state);
                        })?;
                    }
                }
            }
            AppEvent::ImageResized(resize_response) => match resize_response {
                ImageResizeResult::Success(resize_response) => {
                    if let Some(ref mut img_thread_protocol) = state.img_state.protocol {
                        let _ = img_thread_protocol.update_resized_protocol(resize_response);
                        terminal.draw(|f| {
                            draw_closure(f, &mut state);
                        })?;
                    }
                }
                ImageResizeResult::Error(e) => {
                    state.img_state.error = Some(format!("Error resizing image: {}", e));
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
            },
            AppEvent::ImageLoad(img_load) => match img_load {
                ImageLoadedResult::Success {
                    key,
                    protocol,
                    clipboard_image,
                } => {
                    if state.img_state.current_request_key() != Some(key) {
                        continue;
                    }
                    state.img_state.protocol = Some(protocol);
                    state.img_state.clipboard_image = Some(clipboard_image);
                    state.img_state.error = None;
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
                ImageLoadedResult::Failure { key, message } => {
                    if state.img_state.current_request_key() != Some(key) {
                        continue;
                    }
                    state.img_state.protocol = None;
                    state.img_state.clipboard_image = None;
                    state.img_state.error = Some(message);

                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
            },
            AppEvent::PreviewChartLoad(image_loaded_result) => match image_loaded_result {
                ChartPreviewLoadedResult::Success {
                    key,
                    protocol,
                    clipboard_image,
                } => {
                    if state.chart_preview_state.current_request_key() != Some(key) {
                        continue;
                    }
                    state.chart_preview_state.protocol = Some(protocol);
                    state.chart_preview_state.clipboard_image = Some(clipboard_image);
                    state.chart_preview_state.error = None;
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
                ChartPreviewLoadedResult::Failure { key, message } => {
                    if state.chart_preview_state.current_request_key() != Some(key) {
                        continue;
                    }
                    state.chart_preview_state.protocol = None;
                    state.chart_preview_state.clipboard_image = None;
                    state.chart_preview_state.error = Some(message);

                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
            },
            AppEvent::PreviewChartResized(image_resize_result) => match image_resize_result {
                ImageResizeResult::Success(resize_response) => {
                    if let Some(ref mut protocol) = state.chart_preview_state.protocol {
                        let _ = protocol.update_resized_protocol(resize_response);
                        terminal.draw(|f| {
                            draw_closure(f, &mut state);
                        })?;
                    }
                }
                ImageResizeResult::Error(e) => {
                    state.chart_preview_state.error =
                        Some(format!("Error resizing chart preview: {}", e));
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
            },
            AppEvent::HeatmapLoad(heatmap_loaded_result) => match heatmap_loaded_result {
                HeatmapLoadedResult::Success { page } => {
                    state.heatmap_render.pending_keys.remove(&page.key);
                    let should_redraw =
                        state.heatmap_render.current_key.as_ref() == Some(&page.key);
                    if let Some(image) = image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(
                        page.pixel_width,
                        page.pixel_height,
                        page.rgb_bytes,
                    ) {
                        let dyn_img = image::DynamicImage::ImageRgb8(image);
                        state
                            .heatmap_render
                            .cached_pages
                            .retain(|entry| entry.key != page.key);
                        state
                            .heatmap_render
                            .cached_pages
                            .push_back(state::HeatmapCachedPage {
                                key: page.key,
                                protocol: state.multi_chart.picker.new_resize_protocol(dyn_img),
                                slice_summary: page.slice_summary,
                                legend_summary: page.legend_summary,
                                viewport_selection: page.viewport_selection,
                                selection: page.selection,
                            });
                        while state.heatmap_render.cached_pages.len() > HEATMAP_CACHE_CAPACITY {
                            state.heatmap_render.cached_pages.pop_front();
                        }
                        if should_redraw {
                            terminal.draw(|f| {
                                draw_closure(f, &mut state);
                            })?;
                        }
                    }
                }
                HeatmapLoadedResult::Failure { key, message } => {
                    state.heatmap_render.pending_keys.remove(&key);
                    if state.heatmap_render.current_key.as_ref() == Some(&key) {
                        state.toast =
                            AppToast::Error(format!("Heatmap prefetch failed: {message}"));
                        terminal.draw(|f| {
                            draw_closure(f, &mut state);
                        })?;
                    }
                }
                HeatmapLoadedResult::Dropped { key } => {
                    state.heatmap_render.pending_keys.remove(&key);
                }
            },
            AppEvent::PreviewDebounceExpired(generation) => {
                if state.resolve_preview_debounce(generation) {
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
            }
            AppEvent::FileChanged => {
                if let Some(toast) = state.register_file_watch_change() {
                    state.toast = toast;
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
            }
        }
    }
    if let Some(file) = state.file.take() {
        file.close()?;
    }
    Ok(IntendedMainLoopBreak {})
}

fn apply_startup_event_result(state: &mut AppState<'_>, event_result: EventResult) -> Result<bool> {
    match event_result {
        EventResult::Quit => Ok(true),
        EventResult::Continue | EventResult::Redraw | EventResult::Copying => Ok(false),
        EventResult::ReloadFile { write } => {
            match reload_current_file(state, write) {
                Ok(message) => state.toast = AppToast::Info(message),
                Err(error) => state.toast = AppToast::Error(error.to_string()),
            }
            Ok(false)
        }
        EventResult::Configure { .. } => {
            state.toast = AppToast::Info(
                "The configure command is only available after startup completes".to_string(),
            );
            Ok(false)
        }
        EventResult::Error(error) => {
            state.toast = AppToast::Error(error);
            Ok(false)
        }
        EventResult::Toast(toast, full_redraw) => {
            if full_redraw {
                state.compute_tree_view();
            }
            state.toast = toast;
            Ok(false)
        }
    }
}

fn run_startup_commands(
    state: &mut AppState<'_>,
    startup_commands: &[StartupCommand],
) -> Result<bool> {
    for startup_command in startup_commands {
        let invocation = parse_command_text(&startup_command.command_text).map_err(|error| {
            AppError::InvalidCommand(format!("{}: {}", startup_command.origin, error))
        })?;
        let event_result = execute_command(state, &invocation).map_err(|error| {
            AppError::InvalidCommand(format!("{}: {}", startup_command.origin, error))
        })?;
        state.command_state.record_successful_command(&invocation);
        if apply_startup_event_result(state, event_result)? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[allow(clippy::large_enum_variant)]
pub enum AppEvent {
    TermEvent(event::Event),
    ImageResized(ImageResizeResult),
    ImageLoad(ImageLoadedResult),
    PreviewChartLoad(ChartPreviewLoadedResult),
    PreviewChartResized(ImageResizeResult),
    HeatmapLoad(HeatmapLoadedResult),
    PreviewDebounceExpired(u64),
    Toast(AppToast),
    FileChanged,
}

#[allow(clippy::large_enum_variant)]
pub enum ImageLoadedResult {
    Success {
        key: ImageLoadKey,
        protocol: ratatui_image::thread::ThreadProtocol,
        clipboard_image: state::ClipboardImageData,
    },
    Failure {
        key: ImageLoadKey,
        message: String,
    },
}

#[allow(clippy::large_enum_variant)]
pub enum ChartPreviewLoadedResult {
    Success {
        key: ChartPreviewKey,
        protocol: ratatui_image::thread::ThreadProtocol,
        clipboard_image: state::ClipboardImageData,
    },
    Failure {
        key: ChartPreviewKey,
        message: String,
    },
}

#[allow(clippy::large_enum_variant)]
pub enum HeatmapLoadedResult {
    Success {
        page: HeatmapLoadedPage,
    },
    Failure {
        key: HeatmapRenderKey,
        message: String,
    },
    Dropped {
        key: HeatmapRenderKey,
    },
}

fn render_header(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &AppState<'_>,
    new_version: Option<&str>,
) -> Rect {
    if area.height <= HEADER_HEIGHT {
        return area;
    }

    let sections =
        Layout::vertical([Constraint::Length(HEADER_HEIGHT), Constraint::Min(0)]).split(area);
    let header_area = sections[0];
    let body_area = sections[1];

    let columns = Layout::horizontal([
        Constraint::Percentage(32),
        Constraint::Percentage(40),
        Constraint::Percentage(28),
    ])
    .split(header_area);

    frame.render_widget(
        Paragraph::new(Line::raw("")).style(
            Style::default()
                .bg(configure::themed_color(|colors| colors.surface.bg_val3))
                .fg(configure::themed_color(|colors| colors.text.primary)),
        ),
        header_area,
    );

    let left = Line::from(vec![
        Span::styled(
            if state.readonly {
                configure::configured_symbol(|symbols| symbols.badge.readonly)
            } else {
                configure::configured_symbol(|symbols| symbols.badge.writable)
            },
            Style::default()
                .fg(if state.readonly {
                    configure::themed_color(|colors| colors.status.readonly)
                } else {
                    configure::themed_color(|colors| colors.status.writable)
                })
                .bold(),
        ),
        if state.file_watch.linked {
            Span::styled(
                configure::configured_symbol(|symbols| symbols.badge.linked),
                Style::default().fg(configure::themed_color(|colors| colors.status.linked)),
            )
        } else {
            Span::raw("")
        },
        if state.compatibility_mode {
            Span::styled(
                configure::configured_symbol(|symbols| symbols.badge.compatibility_mode),
                Style::default()
                    .fg(configure::themed_color(|colors| colors.status.compability))
                    .bold(),
            )
        } else {
            Span::raw("")
        },
        if state.configuration_warning.is_some() {
            Span::styled(
                " ! config ",
                Style::default()
                    .fg(configure::themed_color(|colors| colors.toast.warning))
                    .bold(),
            )
        } else {
            Span::raw("")
        },
    ]);
    frame.render_widget(Paragraph::new(left).style(primary_text_style()), columns[0]);

    let mut center = vec![
        Span::styled(
            " h5v ",
            Style::default()
                .fg(configure::themed_color(|colors| colors.content.app_brand))
                .bg(configure::themed_color(|colors| colors.surface.title_bg))
                .bold(),
        ),
        Span::raw(" "),
        Span::styled(
            GIT_VERSION,
            Style::default()
                .fg(configure::themed_color(|colors| colors.content.app_version))
                .bold(),
        ),
    ];
    if let Some(new_version) = new_version {
        center.push(Span::raw("  "));
        center.push(Span::styled(
            format!("update available: {new_version}"),
            Style::default()
                .fg(configure::themed_color(|colors| {
                    colors.status.update_available
                }))
                .bold(),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(center))
            .style(primary_text_style())
            .alignment(Alignment::Center),
        columns[1],
    );

    let right = Line::from(vec![Span::styled(
        "(type ? for help)",
        Style::default().fg(configure::themed_color(|colors| colors.content.help_hint)),
    )]);
    frame.render_widget(
        Paragraph::new(right).alignment(Alignment::Right),
        columns[2],
    );
    body_area
}

fn split_command_bar(area: Rect) -> (Rect, Rect) {
    if area.height <= COMMAND_BAR_HEIGHT {
        (area, area)
    } else {
        let sections =
            Layout::vertical([Constraint::Min(0), Constraint::Length(COMMAND_BAR_HEIGHT)])
                .split(area);
        (sections[0], sections[1])
    }
}

fn split_render_toast(frame: &mut Frame<'_>, state: &AppState) -> Rect {
    let area = frame.area();
    match state.toast {
        AppToast::Empty => area,
        AppToast::Info(ref msg) | AppToast::Error(ref msg) | AppToast::Warning(ref msg) => {
            let areas = Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(area);

            let toast_area = areas[1];
            let toast_text = Text::from(msg.to_string());
            let toast_paragraph = Paragraph::new(toast_text)
                .block(
                    Block::default()
                        .bg(configure::themed_color(|colors| colors.surface.bg))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(match state.toast {
                            AppToast::Info(_) => {
                                configure::themed_color(|colors| colors.toast.info)
                            }
                            AppToast::Error(_) => {
                                configure::themed_color(|colors| colors.text.error)
                            }
                            AppToast::Warning(_) => {
                                configure::themed_color(|colors| colors.toast.warning)
                            }
                            _ => configure::themed_color(|colors| colors.toast.neutral),
                        }))
                        .border_type(ratatui::widgets::BorderType::Rounded)
                        .title(match state.toast {
                            AppToast::Info(_) => "Info",
                            AppToast::Error(_) => "Error",
                            AppToast::Warning(_) => "Warning",
                            _ => "",
                        })
                        .title_style(
                            Style::default()
                                .fg(configure::themed_color(|colors| colors.surface.panel_title))
                                .bold(),
                        )
                        .title_alignment(Alignment::Center),
                )
                .wrap(Wrap { trim: true });
            frame.render_widget(toast_paragraph, toast_area);

            areas[0]
        }
    }
}

fn render_error(frame: &mut Frame<'_>, error: &str) {
    let error_text = Text::from(error);
    let error_paragraph = Paragraph::new(error_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(
                    Style::default().fg(configure::themed_color(|colors| colors.text.error)),
                )
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(configure::configured_symbol(|symbols| symbols.title.error))
                .title_style(
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.surface.panel_title))
                        .bold(),
                )
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(error_paragraph, frame.area());
}

#[cfg(test)]
mod tests {
    use super::update::{
        resolve_available_update, update_check_cache_is_fresh, write_update_check_cache,
        UpdateCheckCache, UPDATE_CHECK_INTERVAL,
    };
    use std::{
        cell::Cell,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use tempfile::tempdir;

    #[test]
    fn uses_fresh_cached_update_without_fetching() {
        let tempdir = tempdir().expect("tempdir");
        let cache_path = tempdir.path().join("update-check.json");
        let now = UNIX_EPOCH + Duration::from_secs(200_000);
        write_update_check_cache(
            &cache_path,
            &UpdateCheckCache {
                current_version: "0.1.0".to_string(),
                checked_at_unix_secs: 200_000 - UPDATE_CHECK_INTERVAL.as_secs() + 1,
                available_version: Some("v0.2.0".to_string()),
            },
        )
        .expect("write cache");

        let fetch_calls = Cell::new(0);
        let version = resolve_available_update(Some(&cache_path), "0.1.0", now, || {
            fetch_calls.set(fetch_calls.get() + 1);
            Ok(Some("v9.9.9".to_string()))
        });

        assert_eq!(version.as_deref(), Some("v0.2.0"));
        assert_eq!(fetch_calls.get(), 0);
    }

    #[test]
    fn refreshes_stale_update_cache_after_one_day() {
        let tempdir = tempdir().expect("tempdir");
        let cache_path = tempdir.path().join("update-check.json");
        let now = UNIX_EPOCH + Duration::from_secs(200_000);
        write_update_check_cache(
            &cache_path,
            &UpdateCheckCache {
                current_version: "0.1.0".to_string(),
                checked_at_unix_secs: 200_000 - UPDATE_CHECK_INTERVAL.as_secs(),
                available_version: Some("v0.2.0".to_string()),
            },
        )
        .expect("write cache");

        let fetch_calls = Cell::new(0);
        let version = resolve_available_update(Some(&cache_path), "0.1.0", now, || {
            fetch_calls.set(fetch_calls.get() + 1);
            Ok(Some("v0.3.0".to_string()))
        });

        assert_eq!(version.as_deref(), Some("v0.3.0"));
        assert_eq!(fetch_calls.get(), 1);
    }

    #[test]
    fn update_cache_is_not_fresh_for_different_version() {
        let now = SystemTime::now();
        let cache = UpdateCheckCache {
            current_version: "0.1.0".to_string(),
            checked_at_unix_secs: now.duration_since(UNIX_EPOCH).expect("unix time").as_secs(),
            available_version: Some("v0.2.0".to_string()),
        };

        assert!(!update_check_cache_is_fresh(&cache, "0.2.0", now));
    }
}
