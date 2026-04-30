use std::{
    fs,
    io::stdout,
    rc::Rc,
    sync::{
        mpsc::{channel, Sender},
        Arc, RwLock,
    },
    thread,
    time::Duration,
};

use arboard::Clipboard;
use ratatui::{
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    },
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    prelude::CrosstermBackend,
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame, Terminal,
};
use ratatui_image::picker::Picker;

use crate::{
    color_consts,
    error::{log_error, AppError},
    h5f::{self, HasPath, Node, NodeType},
    ui::{
        image_preview::{handle_chartpreview_load, handle_chartpreview_resize},
        input::EventResult,
        mchart::MultiChartState,
        state::{AppToast, ChartPreviwState, FileWatchState},
    },
    GIT_VERSION,
};

use super::state::{ChartPreviewKey, ImageLoadKey};
use super::{
    command::{Command, CommandState},
    command_view::render_command_dialog,
    image_preview::{
        handle_image_load, handle_image_resize, handle_imagefs_load, handle_imagefsvlen_load,
        ImageResizeResult,
    },
    input::handle_input_event,
    main_display::render_main_display,
    state::{
        self, AppState, AttributeCursor, ContentShowMode, FixedStringOverflowChoice, Focus,
        ImgState, LastFocused, MatrixViewState, Mode,
    },
    tree_view::render_tree,
};

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

#[derive(Clone)]
struct SelectedNodeSnapshot {
    path: String,
    selected_dim: usize,
    selected_x: usize,
    selected_row: usize,
    selected_col: usize,
    line_offset: usize,
    col_offset: isize,
    selected_indexes: Vec<usize>,
    attributes_view_cursor: AttributeCursor,
}

#[derive(Clone)]
struct ReloadSnapshot {
    root_selected: bool,
    selected_path: Option<String>,
    expanded_paths: Vec<String>,
    selected_node: Option<SelectedNodeSnapshot>,
    focus: Focus,
    show_tree_view: bool,
    content_mode: ContentShowMode,
    segment_state: state::SegmentState,
    matrix_view_state: MatrixViewState,
    img_idx_to_load: i32,
}

fn normalized_node_path(path: &str) -> &str {
    path.trim_start_matches('/')
}

fn same_node_path(lhs: &str, rhs: &str) -> bool {
    normalized_node_path(lhs) == normalized_node_path(rhs)
}

fn collect_expanded_paths(node: &Rc<std::cell::RefCell<h5f::H5FNode>>, out: &mut Vec<String>) {
    let node_ref = node.borrow();
    for child in &node_ref.children {
        let child_ref = child.borrow();
        if child_ref.is_expandable() && child_ref.expanded {
            out.push(child_ref.node.path());
        }
        drop(child_ref);
        collect_expanded_paths(child, out);
    }
}

fn snapshot_selected_node(state: &AppState<'_>) -> Option<SelectedNodeSnapshot> {
    let tree_item = state.treeview.get(state.tree_view_cursor)?;
    let node = tree_item.node.borrow();
    Some(SelectedNodeSnapshot {
        path: node.node.path(),
        selected_dim: node.selected_dim,
        selected_x: node.selected_x,
        selected_row: node.selected_row,
        selected_col: node.selected_col,
        line_offset: node.line_offset,
        col_offset: node.col_offset,
        selected_indexes: node.selected_indexes.clone(),
        attributes_view_cursor: node.attributes_view_cursor.clone(),
    })
}

fn snapshot_reload_state(state: &AppState<'_>) -> ReloadSnapshot {
    let mut expanded_paths = Vec::new();
    collect_expanded_paths(&state.root, &mut expanded_paths);
    expanded_paths.sort_by_key(|path| normalized_node_path(path).matches('/').count());

    ReloadSnapshot {
        root_selected: state.tree_view_cursor == 0,
        selected_path: state.selected_tree_path(),
        expanded_paths,
        selected_node: snapshot_selected_node(state),
        focus: state.focus.clone(),
        show_tree_view: state.show_tree_view,
        content_mode: state.content_mode,
        segment_state: state.segment_state.clone(),
        matrix_view_state: state.matrix_view_state.clone(),
        img_idx_to_load: state.img_state.idx_to_load,
    }
}

fn restore_tree_selection(state: &mut AppState<'_>, snapshot: &ReloadSnapshot) {
    if snapshot.root_selected {
        state.tree_view_cursor = 0;
        return;
    }

    let Some(selected_path) = snapshot.selected_path.as_deref() else {
        state.tree_view_cursor = 0;
        return;
    };

    if let Some((idx, _)) = state
        .treeview
        .iter()
        .enumerate()
        .find(|(_, item)| same_node_path(&item.node.borrow().node.path(), selected_path))
    {
        state.tree_view_cursor = idx;
        return;
    }

    let mut fallback = selected_path.to_string();
    while let Some((prefix, _)) = normalized_node_path(&fallback).rsplit_once('/') {
        if prefix.is_empty() {
            break;
        }
        fallback = prefix.to_string();
        if let Some((idx, _)) = state
            .treeview
            .iter()
            .enumerate()
            .find(|(_, item)| same_node_path(&item.node.borrow().node.path(), &fallback))
        {
            state.tree_view_cursor = idx;
            return;
        }
    }

    state.tree_view_cursor = 0;
}

fn restore_selected_node_state(state: &mut AppState<'_>, snapshot: &ReloadSnapshot) {
    let Some(selected_snapshot) = snapshot.selected_node.as_ref() else {
        return;
    };
    let Some(tree_item) = state.treeview.get(state.tree_view_cursor) else {
        return;
    };
    if !same_node_path(
        &tree_item.node.borrow().node.path(),
        &selected_snapshot.path,
    ) {
        return;
    }

    let mut node = tree_item.node.borrow_mut();
    let shape = match &node.node {
        Node::Dataset(_, meta) => meta.shape.clone(),
        _ => Vec::new(),
    };
    let rank = shape.len();
    node.sync_selection_rank(rank);
    for ((dst, src), dim_len) in node
        .selected_indexes
        .iter_mut()
        .zip(selected_snapshot.selected_indexes.iter().copied())
        .zip(shape.iter().copied())
    {
        *dst = src.min(dim_len.saturating_sub(1));
    }
    node.selected_dim = node.selected_dim.min(rank.saturating_sub(1));
    node.selected_x = selected_snapshot.selected_x.min(rank.saturating_sub(1));
    node.selected_row = selected_snapshot.selected_row.min(rank.saturating_sub(1));
    node.selected_col = if rank > 1 {
        selected_snapshot.selected_col.min(rank.saturating_sub(1))
    } else {
        0
    };
    node.selected_dim = selected_snapshot.selected_dim.min(rank.saturating_sub(1));
    node.line_offset = selected_snapshot.line_offset;
    node.col_offset = selected_snapshot.col_offset.max(0);
    node.attributes_view_cursor = selected_snapshot.attributes_view_cursor.clone();
}

fn clear_preview_state(state: &mut AppState<'_>, snapshot: &ReloadSnapshot) {
    state.clear_preview_debounce();
    state.segment_state = snapshot.segment_state.clone();
    state.matrix_view_state = snapshot.matrix_view_state.clone();
    state.img_state.protocol = None;
    state.img_state.ds = None;
    state.img_state.current_key = None;
    state.img_state.window = None;
    state.img_state.idx_loaded = -1;
    state.img_state.idx_to_load = snapshot.img_idx_to_load;
    state.img_state.error = None;
    state.chart_preview_state.ds_loaded = None;
    state.chart_preview_state.protocol = None;
    state.chart_preview_state.error = None;
    state.chart_preview_state.ds_selection = None;
}

fn placeholder_root(path: &str) -> Rc<std::cell::RefCell<h5f::H5FNode>> {
    Rc::new(std::cell::RefCell::new(h5f::H5FNode::new(Node::Broken(
        NodeType::Group,
        path.to_string(),
        String::new(),
    ))))
}

fn reload_current_file(state: &mut AppState<'_>, write: bool) -> Result<String> {
    let snapshot = snapshot_reload_state(state);
    let file_path = state.file_watch.path.clone();
    let linked = state.file_watch.linked;
    let previous_write = !state.readonly;

    clear_preview_state(state, &snapshot);
    state.treeview.clear();
    state.searcher = None;
    let old_root = std::mem::replace(&mut state.root, placeholder_root(&file_path));
    let old_file = state.file.take();
    drop(old_root);
    if let Some(old_file) = old_file {
        old_file.close().map_err(|e| {
            AppError::Hdf5(hdf5_metno::Error::from(format!(
                "Failed to close HDF5 file '{}' before reload: {}",
                file_path, e
            )))
        })?;
    }

    let reopened = match h5f::H5F::open(file_path.clone(), linked, write) {
        Ok(reopened) => reopened,
        Err(target_error) => {
            let fallback = h5f::H5F::open(file_path.clone(), linked, previous_write).map_err(
                |fallback_error| {
                    AppError::Hdf5(hdf5_metno::Error::from(format!(
                        "Failed to reopen HDF5 file '{}' in {:?} mode after reload error (reload error: {}; fallback error: {})",
                        file_path,
                        if previous_write { "write" } else { "read-only" },
                        target_error,
                        fallback_error
                    )))
                },
            )?;
            state.file = Some(fallback.file);
            state.root = fallback.root;
            state.readonly = !previous_write;
            state.focus = snapshot.focus.clone();
            state.show_tree_view = snapshot.show_tree_view;
            state.content_mode = snapshot.content_mode;
            for path in &snapshot.expanded_paths {
                let relative = normalized_node_path(path);
                if relative.is_empty() {
                    continue;
                }
                let _ = state.root.borrow_mut().expand_path(relative);
            }
            state.compute_tree_view();
            restore_tree_selection(state, &snapshot);
            restore_selected_node_state(state, &snapshot);
            state.compute_tree_view();
            restore_tree_selection(state, &snapshot);
            state.sync_file_watch();
            return Err(AppError::Hdf5(hdf5_metno::Error::from(format!(
                "Failed to reopen HDF5 file '{}' in {} mode: {}",
                file_path,
                if write { "write" } else { "read-only" },
                target_error
            ))));
        }
    };

    state.file = Some(reopened.file);
    state.root = reopened.root;
    state.readonly = !write;
    state.focus = snapshot.focus.clone();
    state.show_tree_view = snapshot.show_tree_view;
    state.content_mode = snapshot.content_mode;

    for path in &snapshot.expanded_paths {
        let relative = normalized_node_path(path);
        if relative.is_empty() {
            continue;
        }
        let _ = state.root.borrow_mut().expand_path(relative);
    }

    state.compute_tree_view();
    restore_tree_selection(state, &snapshot);
    restore_selected_node_state(state, &snapshot);
    state.compute_tree_view();
    restore_tree_selection(state, &snapshot);
    state.sync_file_watch();

    Ok(if write {
        "Reloaded file in write mode".to_string()
    } else {
        "Reloaded file".to_string()
    })
}

pub fn init(filename: String, link: bool, writable: bool) -> Result<()> {
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(EnableMouseCapture)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let mut last_message = None;

    loop {
        match main_recover_loop(&mut terminal, filename.clone(), link, writable) {
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
            },
        }
    }

    stdout().execute(DisableMouseCapture)?;
    stdout().execute(LeaveAlternateScreen)?;
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
) -> Result<IntendedMainLoopBreak> {
    let h5f = h5f::H5F::open(filename.clone(), link, writable).map_err(|e| {
        AppError::Hdf5(hdf5_metno::Error::from(format!(
            "Failed to open HDF5 file: {}",
            e
        )))
    })?;

    let (tx_events, rx_events) = channel();
    #[allow(deprecated)]
    let picker = Picker::from_query_stdio().unwrap_or(Picker::from_fontsize((7, 14)));
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

    let img_state = ImgState {
        protocol: None,
        tx_load_imgfs,
        tx_load_imgfsvlen,
        tx_load_img,
        ds: None,
        current_key: None,
        window: None,
        idx_to_load: 0,
        idx_loaded: -1,
        error: None,
    };

    let chart_preview_state = ChartPreviwState {
        ds_loaded: None,
        protocol: None,
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
    let clipboard = Clipboard::new()
        .map_err(|e| AppError::ClipboardError(format!("Failed to initialize clipboard: {}", e)))?;

    let segment_state = state::SegmentState {
        idx: 0,
        segment_count: 0,
        segumented: state::SegmentType::NoSegment,
    };

    let command_state = CommandState {
        command_buffer: String::new(),
        last_command: Command::Noop,
        cursor: 0,
    };
    let edit_pause = Arc::new(RwLock::new(()));

    let root_node = h5f.root.clone();
    let mut state = AppState {
        readonly: !writable,
        root: root_node,
        editing: false,
        file: Some(h5f.file),
        toast: AppToast::Empty,
        file_watch: FileWatchState {
            path: filename.clone(),
            linked: link,
            last_known_modified: None,
            pending_external_change: false,
        },
        multi_chart: MultiChartState::new(picker.clone()),
        segment_state,
        edit_pause: edit_pause.clone(),
        command_state,
        fixed_string_overflow_dialog: None,
        treeview: vec![],
        tree_view_cursor: 0,
        focus: Focus::Tree(LastFocused::Attributes),
        clipboard,
        mode: Mode::Normal,
        copying: false,
        searcher: None,
        pending_chord: None,
        show_tree_view: true,
        stacked_tree_layout: false,
        image_cell_size,
        preview_debounce_generation: 0,
        preview_debounce_until: None,
        preview_debounce_path: None,
        content_mode: ContentShowMode::Preview,
        img_state,
        matrix_view_state,
        chart_preview_state,
        ui_layout: state::UiLayoutState::default(),
    };

    state.sync_file_watch();
    state.compute_tree_view();

    let draw_closure = |frame: &mut Frame, state: &mut AppState| {
        let frame_area = match state.toast {
            AppToast::Empty => frame.area(),
            AppToast::Info(_) | AppToast::Warning(_) | AppToast::Error(_) => {
                split_render_toast(frame, state)
            }
        };
        let content_area = render_header(frame, frame_area, state);
        state.ui_layout = state::UiLayoutState::default();

        if let Mode::Help = state.mode {
            render_help(frame, content_area);
            return;
        }
        if let Mode::MultiChart = state.mode {
            state.multi_chart.render(frame, content_area);
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
            Mode::Command => render_command_dialog(frame, state),
            Mode::Normal | Mode::FixedStringOverflowDialog | Mode::FixedStringResizeDialog => {
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
                ImageLoadedResult::Success { key, protocol } => {
                    if state.img_state.current_request_key() != Some(key) {
                        continue;
                    }
                    state.img_state.protocol = Some(protocol);
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
                    state.img_state.error = Some(message);

                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
            },
            AppEvent::PreviewChartLoad(image_loaded_result) => match image_loaded_result {
                ChartPreviewLoadedResult::Success { key, protocol } => {
                    if state.chart_preview_state.current_request_key() != Some(key) {
                        continue;
                    }
                    state.chart_preview_state.protocol = Some(protocol);
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

#[allow(clippy::large_enum_variant)]
pub enum AppEvent {
    TermEvent(event::Event),
    ImageResized(ImageResizeResult),
    ImageLoad(ImageLoadedResult),
    PreviewChartLoad(ChartPreviewLoadedResult),
    PreviewChartResized(ImageResizeResult),
    PreviewDebounceExpired(u64),
    FileChanged,
}

fn schedule_preview_debounce(tx_events: Sender<AppEvent>, generation: u64) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(95));
        let _ = tx_events.send(AppEvent::PreviewDebounceExpired(generation));
    });
}

fn handle_file_watch_events(tx_events: Sender<AppEvent>, path: String) {
    thread::spawn(move || {
        let mut last_modified = fs::metadata(&path)
            .ok()
            .and_then(|metadata| metadata.modified().ok());
        loop {
            thread::sleep(Duration::from_millis(500));
            let current_modified = fs::metadata(&path)
                .ok()
                .and_then(|metadata| metadata.modified().ok());
            if current_modified == last_modified {
                continue;
            }
            last_modified = current_modified;
            if tx_events.send(AppEvent::FileChanged).is_err() {
                return;
            }
        }
    });
}

#[allow(clippy::large_enum_variant)]
pub enum ImageLoadedResult {
    Success {
        key: ImageLoadKey,
        protocol: ratatui_image::thread::ThreadProtocol,
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
    },
    Failure {
        key: ChartPreviewKey,
        message: String,
    },
}

fn render_header(frame: &mut Frame<'_>, area: Rect, state: &AppState<'_>) -> Rect {
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
                .bg(color_consts::BG_VAL3_COLOR)
                .fg(color_consts::COLOR_WHITE),
        ),
        header_area,
    );

    let left = Line::from(vec![
        if state.readonly {
            Span::styled(" 🔒 read-only ", Style::default().fg(Color::Yellow).bold())
        } else {
            Span::styled(" ✏ write ", Style::default().fg(Color::LightGreen).bold())
        },
        if state.file_watch.linked {
            Span::styled(" linked ", Style::default().fg(Color::Cyan))
        } else {
            Span::raw("")
        },
    ]);
    frame.render_widget(Paragraph::new(left), columns[0]);

    let center = Line::from(vec![
        Span::styled(
            " h5v ",
            Style::default()
                .fg(color_consts::TITLE)
                .bg(color_consts::BREAK_COLOR)
                .bold(),
        ),
        Span::raw(" "),
        Span::styled(
            GIT_VERSION,
            Style::default()
                .fg(color_consts::BUILT_IN_VALUE_COLOR)
                .bold(),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(center).alignment(Alignment::Center),
        columns[1],
    );

    let right = Line::from(vec![Span::styled(
        "(type ? for help)",
        Style::default().fg(color_consts::TYPE_DESC_COLOR),
    )]);
    frame.render_widget(
        Paragraph::new(right).alignment(Alignment::Right),
        columns[2],
    );
    body_area
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
                        .bg(color_consts::BG_COLOR)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(match state.toast {
                            AppToast::Info(_) => Color::LightGreen,
                            AppToast::Error(_) => Color::Red,
                            AppToast::Warning(_) => Color::Yellow,
                            _ => Color::White,
                        }))
                        .border_type(ratatui::widgets::BorderType::Rounded)
                        .title(match state.toast {
                            AppToast::Info(_) => "Info",
                            AppToast::Error(_) => "Error",
                            AppToast::Warning(_) => "Warning",
                            _ => "",
                        })
                        .title_style(Style::default().fg(Color::Yellow).bold())
                        .title_alignment(Alignment::Center),
                )
                .wrap(Wrap { trim: true });
            frame.render_widget(toast_paragraph, toast_area);

            areas[0]
        }
    }
}

fn handle_term_events(tx_events: Sender<AppEvent>, paused: Arc<RwLock<()>>) {
    thread::spawn(move || loop {
        if event::poll(std::time::Duration::from_millis(16)).is_ok() {
            let Ok(pause) = paused.read() else {
                tx_events
                    .send(AppEvent::TermEvent(event::Event::Resize(0, 0)))
                    .unwrap_or_else(log_error);
                return;
            };
            drop(pause);
            if let Ok(event) = event::read() {
                match tx_events.send(AppEvent::TermEvent(event)) {
                    Ok(_) => {}
                    Err(e) => log_error(e),
                }
            }
        }
    });
}

fn render_error(frame: &mut Frame<'_>, error: &str) {
    let error_text = Text::from(error);
    let error_paragraph = Paragraph::new(error_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title("Error")
                .title_style(Style::default().fg(Color::Yellow).bold())
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(error_paragraph, frame.area());
}

fn render_fixed_string_overflow_dialog(frame: &mut Frame<'_>, area: Rect, state: &AppState<'_>) {
    let Some(dialog) = state.fixed_string_overflow_dialog.as_ref() else {
        return;
    };

    let popup = centered_rect(area, 72, 12);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default().style(Style::default().bg(color_consts::BG_VAL3_COLOR)),
        popup,
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(" Fixed string overflow ")
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(color_consts::FOCUS_BG_COLOR));
    let inner = popup.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    frame.render_widget(block, popup);

    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(3),
    ])
    .split(inner);

    let message = Paragraph::new(format!(
        "{} needs {} bytes, current fixed size is {} bytes.",
        dialog.overflow.kind, dialog.overflow.required_size, dialog.overflow.current_size
    ))
    .wrap(Wrap { trim: true });
    frame.render_widget(message, rows[0]);

    let choices = [
        (FixedStringOverflowChoice::Cancel, "Cancel"),
        (FixedStringOverflowChoice::ChangeToVarLen, "Change to Vlen"),
        (FixedStringOverflowChoice::ChangeSize, "Change size"),
    ]
    .into_iter()
    .map(|(choice, label)| {
        let style = if dialog.selected_choice == choice {
            Style::default().fg(Color::Black).bg(Color::Yellow).bold()
        } else {
            Style::default().fg(Color::White)
        };
        Span::styled(format!(" {label} "), style)
    })
    .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(Line::from(choices))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        rows[2],
    );
}

fn render_fixed_string_resize_dialog(frame: &mut Frame<'_>, area: Rect, state: &AppState<'_>) {
    let Some(dialog) = state.fixed_string_overflow_dialog.as_ref() else {
        return;
    };

    let popup = centered_rect(area, 56, 10);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default().style(Style::default().bg(color_consts::BG_VAL3_COLOR)),
        popup,
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(" Change fixed string size ")
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(color_consts::FOCUS_BG_COLOR));
    frame.render_widget(block, popup);

    let inner = popup.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let rows = Layout::vertical([Constraint::Length(2), Constraint::Length(1)]).split(inner);

    frame.render_widget(
        Paragraph::new(format!(
            "Enter new byte size (minimum {}).",
            dialog.overflow.required_size
        )),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(format!("> {}", dialog.size_input))
            .style(Style::default().fg(Color::White).bold()),
        rows[1],
    );
    frame.set_cursor_position(ratatui::layout::Position::new(
        rows[1].x + 2 + dialog.size_input.len() as u16,
        rows[1].y,
    ));
}

fn render_help(frame: &mut Frame<'_>, area: Rect) {
    let popup = centered_rect(area, 140, 28);

    frame.render_widget(
        Block::default().style(Style::default().bg(color_consts::BG_VAL3_COLOR)),
        area,
    );
    frame.render_widget(Clear, popup);

    let help_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color_consts::BREAK_COLOR))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(" Help ")
        .title_style(Style::default().fg(color_consts::TITLE).bold())
        .title_bottom(Line::from(vec![
            Span::styled(" Esc ", help_key_style()),
            Span::styled(" close ", help_desc_style()),
        ]))
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(color_consts::FOCUS_BG_COLOR));
    frame.render_widget(help_block, popup);

    let inner = popup.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let columns = Layout::horizontal([
        Constraint::Percentage(37),
        Constraint::Percentage(30),
        Constraint::Percentage(33),
    ])
    .split(inner);

    let column_style = Style::default().bg(color_consts::FOCUS_BG_COLOR);
    frame.render_widget(
        Paragraph::new(render_help_column_text(
            "General",
            &[
                (
                    "Move",
                    &[
                        (&["j", "k", "↑", "↓"], "move"),
                        (&["h", "l", "←", "→"], "open / close / move"),
                        (&["g", "Home", "G", "End"], "top / bottom"),
                        (&["Ctrl-U", "PgUp", "Ctrl-D", "PgDn"], "half-page"),
                    ],
                ),
                (
                    "Panes",
                    &[
                        (&["Shift + ←↑↓→"], "focus"),
                        (&["Ctrl-W", "h/j/k/l"], "vim focus"),
                        (&["s", "Ctrl-W o"], "toggle sidebar"),
                    ],
                ),
            ],
        ))
        .style(column_style)
        .wrap(Wrap { trim: true }),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(render_help_column_text(
            "Views",
            &[
                (
                    "View",
                    &[
                        (&["Tab"], "preview / matrix"),
                        (&["y"], "copy selected"),
                        (&["m", "M"], "add / open chart"),
                    ],
                ),
                (
                    "Selectors",
                    &[
                        (&["x", "X"], "preview x-axis"),
                        (&["r", "R"], "matrix row axis"),
                        (&["c", "C"], "matrix col axis"),
                        (&["[", "]"], "selected dim"),
                        (&["Ctrl-X", "Ctrl-A"], "index - / +"),
                    ],
                ),
            ],
        ))
        .style(column_style)
        .wrap(Wrap { trim: true }),
        columns[1],
    );
    frame.render_widget(
        Paragraph::new(render_help_column_text(
            "Modes",
            &[
                (
                    "Search + commands",
                    &[
                        (&["/"], "search"),
                        (&[":"], "command mode"),
                        (&["."], "repeat command"),
                        (&[":n"], "go to item n"),
                        (&[":+n", ":-n"], "move by n"),
                        (&["Enter", "Esc"], "run / leave"),
                    ],
                ),
                ("File", &[(&["Ctrl-R"], "reload file")]),
                (
                    "Multi chart",
                    &[
                        (&["M", "Esc"], "open / close"),
                        (&["j", "k"], "select series"),
                        (&["h", "l", "Shift+←→"], "pan"),
                        (&["+", "-", "Shift+↑↓"], "zoom"),
                        (&["d", "Backspace", "Delete"], "remove"),
                        (&["c"], "reset zoom"),
                        (&["q", "Ctrl-C"], "quit app"),
                    ],
                ),
                ("Other", &[(&["?"], "help"), (&["q", "Ctrl-C"], "quit")]),
            ],
        ))
        .style(column_style)
        .wrap(Wrap { trim: true }),
        columns[2],
    );
}

fn centered_rect(area: Rect, max_width: u16, max_height: u16) -> Rect {
    let width = area.width.saturating_sub(4).min(max_width).max(20);
    let height = area.height.saturating_sub(4).min(max_height).max(10);

    let vertical = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .split(area);
    let horizontal = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width),
        Constraint::Fill(1),
    ])
    .split(vertical[1]);
    horizontal[1]
}

fn help_key_style() -> Style {
    Style::default()
        .fg(color_consts::COLOR_WHITE)
        .bg(Color::Rgb(60, 90, 120))
        .underlined()
        .bold()
}

fn help_section_style() -> Style {
    Style::default().fg(color_consts::TITLE).bold().underlined()
}

fn help_desc_style() -> Style {
    Style::default().fg(color_consts::BUILT_IN_VALUE_COLOR)
}

fn help_muted_style() -> Style {
    Style::default().fg(color_consts::TYPE_DESC_COLOR)
}

fn help_keys(keys: &[&'static str], desc: &'static str) -> Line<'static> {
    let mut spans = Vec::new();
    for (idx, key) in keys.iter().enumerate() {
        spans.push(Span::styled(format!(" {key} "), help_key_style()));
        if idx + 1 != keys.len() {
            spans.push(Span::styled("  ", help_muted_style()));
        }
    }
    spans.push(Span::raw("  "));
    spans.push(Span::styled(desc.to_string(), help_desc_style()));
    Line::from(spans)
}

fn help_section(
    title: &'static str,
    entries: &[(&[&'static str], &'static str)],
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(Span::styled(
        title.to_string(),
        help_section_style(),
    ))];
    for (keys, desc) in entries {
        lines.push(help_keys(keys, desc));
    }
    lines
}

fn render_help_column_text(
    title: &'static str,
    sections: &[(&'static str, &[(&[&'static str], &'static str)])],
) -> Text<'static> {
    let mut lines = vec![
        Line::from(vec![Span::styled(
            title.to_string(),
            Style::default().fg(color_consts::TITLE).bold(),
        )])
        .centered(),
        Line::raw(""),
    ];

    for (idx, (section_title, entries)) in sections.iter().enumerate() {
        lines.extend(help_section(section_title, entries));
        if idx + 1 != sections.len() {
            lines.push(Line::raw(""));
        }
    }

    Text::from(lines)
}
