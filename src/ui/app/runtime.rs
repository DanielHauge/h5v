use std::{
    any::Any,
    panic::{self, AssertUnwindSafe},
    sync::mpsc::{Receiver, RecvTimeoutError},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::SystemTime,
};

use crate::{
    compat::RuntimeConfig,
    configure,
    error::{log_error, AppError},
    h5f::{HasPath, Node, RequestedOpenMode},
    ui::{
        command::{execute_command, parse_command_text, StartupCommand},
        cursor::strip_blink_modifiers,
        heatmap::HEATMAP_CACHE_CAPACITY,
        input::{handle_input_event, EventResult},
        mchart::{MultiChartLoadKind, MultiChartLoadResult},
        preview::image::{ImageResizeResult, IMAGE_CACHE_CAPACITY},
        state::{
            self, AppState, AppToast, ContentShowMode, PreviewExpressionResult,
            CHART_PREVIEW_CACHE_CAPACITY,
        },
        toast::apply_app_toast,
    },
};

use super::{
    boot::prepare_app,
    config::open_configuration_and_reload,
    events::{handle_file_watch_events, handle_term_events, schedule_preview_debounce},
    lifecycle::AppTerminal,
    reload::reload_current_file,
    render::{draw_app_frame, render_error},
    update::spawn_update_check,
    AppEvent, ChartPreviewLoadedResult, HeatmapLoadedResult, ImageLoadedResult,
    NavigationLoadResult, TreeLoadResult,
};

type Result<T> = std::result::Result<T, AppError>;

fn tree_load_is_current(
    current_generation: u64,
    mut pending_request_ids: impl Iterator<Item = u64>,
    generation: u64,
    request_id: u64,
) -> bool {
    generation == current_generation && pending_request_ids.any(|id| id == request_id)
}

fn navigation_load_is_current(
    current_generation: u64,
    pending_request: Option<u64>,
    generation: u64,
    request_id: u64,
) -> bool {
    generation == current_generation && pending_request == Some(request_id)
}

fn startup_navigation_pending(
    pending_tree_selection: bool,
    pending_navigation_request: bool,
) -> bool {
    pending_tree_selection || pending_navigation_request
}

fn apply_navigation_load_result(
    state: &mut AppState<'_>,
    result: NavigationLoadResult,
) -> Result<bool> {
    let (generation, request_id) = match &result {
        NavigationLoadResult::Metadata {
            generation,
            request_id,
            ..
        }
        | NavigationLoadResult::Attributes {
            generation,
            request_id,
            ..
        }
        | NavigationLoadResult::Failure {
            generation,
            request_id,
            ..
        } => (*generation, *request_id),
    };
    if !navigation_load_is_current(
        state.navigation_generation,
        state.pending_navigation_request,
        generation,
        request_id,
    ) {
        return Ok(false);
    }
    let Some(item) = state.treeview.get(state.tree_view_cursor) else {
        return Ok(false);
    };
    let node_ref = item.node.clone();
    let attributes_loaded = matches!(&result, NavigationLoadResult::Attributes { .. });
    let mut node = node_ref.borrow_mut();
    match result {
        NavigationLoadResult::Metadata { node: loaded, .. } => {
            node.node = loaded;
            node.metadata_loading = false;
            let rank = match &node.node {
                crate::h5f::Node::Dataset(_, crate::h5f::DatasetMetaState::Loaded(meta)) => {
                    meta.shape.len()
                }
                _ => 0,
            };
            node.sync_selection_rank(rank);
            state.restore_pending_tree_selection_metadata(&mut node);
        }
        NavigationLoadResult::Attributes { attributes, .. } => {
            node.computed_attributes = Some(attributes);
            node.attributes_loading = false;
            state.pending_navigation_request = None;
        }
        NavigationLoadResult::Failure {
            metadata, message, ..
        } => {
            if metadata {
                node.metadata_loading = false;
                node.attributes_loading = false;
                node.metadata_error = Some(message);
            } else {
                node.attributes_loading = false;
                node.attributes_error = Some(message);
            }
            state.pending_navigation_request = None;
        }
    }
    drop(node);
    if attributes_loaded {
        state.restore_pending_tree_attribute_selection()?;
    }
    Ok(true)
}

fn panic_payload_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

fn recovered_ui_panic_message(context: &str, payload: Box<dyn Any + Send>) -> String {
    format!(
        "Recovered from UI {context} panic: {}",
        panic_payload_message(payload)
    )
}

pub(super) fn main_recover_loop(
    terminal: &mut AppTerminal,
    filename: String,
    link: bool,
    requested_open_mode: RequestedOpenMode,
    runtime_config: RuntimeConfig,
    startup_commands: &[StartupCommand],
    new_version: Option<&str>,
) -> Result<()> {
    super::render_startup_progress(
        "Loading configuration...",
        Some("Preparing plugins, commands, and keymaps."),
    );
    let super::boot::PreparedApp {
        mut state,
        tx_events,
        rx_events,
    } = prepare_app(&filename, link, requested_open_mode, runtime_config)?;
    let mut new_version = new_version.map(str::to_owned);

    if run_startup_commands(&mut state, startup_commands, &rx_events)? {
        return Ok(());
    }

    redraw(terminal, &mut state, new_version.as_deref())?;
    configure::spawn_pending_plugin_refreshes(tx_events.clone());
    spawn_update_check(tx_events.clone(), SystemTime::now());

    let worker_running = Arc::new(AtomicBool::new(true));
    let _worker_shutdown = WorkerShutdownGuard {
        running: worker_running.clone(),
    };

    handle_term_events(
        tx_events.clone(),
        state.edit_pause.clone(),
        worker_running.clone(),
    );
    handle_file_watch_events(
        tx_events.clone(),
        state.file_watch.path.clone(),
        worker_running,
    );

    loop {
        let event = match state
            .toast_expires_at
            .map(|expires_at| expires_at.saturating_duration_since(std::time::Instant::now()))
        {
            Some(timeout) => match rx_events.recv_timeout(timeout) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => {
                    apply_app_toast(&mut state, AppToast::Empty);
                    redraw(terminal, &mut state, new_version.as_deref())?;
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(AppError::ChannelError(
                        "Failed to receive event from channel: channel disconnected".to_string(),
                    ));
                }
            },
            None => match rx_events.recv() {
                Ok(event) => event,
                Err(error) => {
                    log_error(error);
                    return Err(AppError::ChannelError(format!(
                        "Failed to receive event from channel: {error}"
                    )));
                }
            },
        };
        if state.editing {
            continue;
        }

        match event {
            AppEvent::UpdateAvailable(available_version) => {
                if new_version != available_version {
                    let toast_version = available_version.clone();
                    new_version = available_version;
                    if let Some(version) = toast_version {
                        apply_app_toast(
                            &mut state,
                            AppToast::Info(format!("Update available: {version}")),
                        );
                    }
                    redraw(terminal, &mut state, new_version.as_deref())?;
                }
            }
            AppEvent::Toast(toast) => {
                apply_app_toast(&mut state, toast);
                redraw(terminal, &mut state, new_version.as_deref())?;
            }
            AppEvent::TermEvent(event) => {
                let selected_before = state.selected_tree_path();
                let selected_kind_before = selected_item_kind(&state).map(str::to_string);
                let content_mode_before = state.active_content_mode_handle();
                let focus_before = state.focus.clone();
                let mode_before = state.mode.clone();
                let help_open_before = matches!(state.mode, state::Mode::Help);
                let logs_open_before = matches!(state.mode, state::Mode::Logs);
                let command_open_before = matches!(state.mode, state::Mode::Command);
                let search_open_before = matches!(state.mode, state::Mode::Search);
                let multichart_open_before = matches!(state.mode, state::Mode::MultiChart);
                let show_tree_view_before = state.show_tree_view;
                let mut event_result = match panic::catch_unwind(AssertUnwindSafe(|| {
                    handle_input_event(&mut state, event)
                })) {
                    Ok(result) => result.unwrap_or_else(|error| {
                        EventResult::Toast(AppToast::Error(error.to_string()), false)
                    }),
                    Err(payload) => {
                        let message = recovered_ui_panic_message("input handling", payload);
                        log_error(&message);
                        apply_app_toast(&mut state, AppToast::Error(message));
                        redraw(terminal, &mut state, new_version.as_deref())?;
                        continue;
                    }
                };
                state
                    .multi_chart
                    .schedule_viewport_detail_loads(state.file.as_ref());
                if let Err(error) = state
                    .multi_chart
                    .queue_expression_detail_refresh(state.file.as_ref(), state.resolved_open_mode)
                {
                    apply_app_toast(&mut state, AppToast::Error(error));
                }
                event_result = combine_event_results(
                    event_result,
                    commit_selection_change(
                        &mut state,
                        selected_before,
                        selected_kind_before,
                        &tx_events,
                    ),
                );
                state.request_selected_navigation_data();
                if focus_before != state.focus {
                    let previous_focus = focus_label(&focus_before);
                    let focus = focus_label(&state.focus);
                    let callback_result =
                        dispatch_runtime_event(&mut state, "builtin.event.focus_changed", |lua| {
                            let event = lua.create_table()?;
                            event.set("previous_focus", previous_focus)?;
                            event.set("focus", focus)?;
                            Ok(event)
                        });
                    event_result = combine_event_results(event_result, callback_result);
                }
                if show_tree_view_before != state.show_tree_view {
                    let visible = state.show_tree_view;
                    let callback_result = dispatch_runtime_event(
                        &mut state,
                        "builtin.event.tree_view_toggled",
                        |lua| {
                            let event = lua.create_table()?;
                            event.set("visible", visible)?;
                            Ok(event)
                        },
                    );
                    event_result = combine_event_results(event_result, callback_result);
                }
                if !multichart_open_before && matches!(state.mode, state::Mode::MultiChart) {
                    let selected_path = state.selected_tree_path();
                    let callback_result = dispatch_runtime_event(
                        &mut state,
                        "builtin.event.multichart_opened",
                        |lua| {
                            let event = lua.create_table()?;
                            if let Some(path) = &selected_path {
                                event.set("path", path.clone())?;
                            }
                            Ok(event)
                        },
                    );
                    event_result = combine_event_results(event_result, callback_result);
                }
                if multichart_open_before && !matches!(state.mode, state::Mode::MultiChart) {
                    let selected_path = state.selected_tree_path();
                    let callback_result = dispatch_runtime_event(
                        &mut state,
                        "builtin.event.multichart_closed",
                        |lua| {
                            let event = lua.create_table()?;
                            if let Some(path) = &selected_path {
                                event.set("selected_path", path.clone())?;
                            }
                            Ok(event)
                        },
                    );
                    event_result = combine_event_results(event_result, callback_result);
                }
                let content_mode_after = state.active_content_mode_handle();
                if content_mode_before != content_mode_after {
                    let selected_path = state.selected_tree_path();
                    let callback_result = configure::dispatch_lua_event(
                        &mut state,
                        "builtin.event.content_mode_changed",
                        |lua| {
                            let event = lua.create_table()?;
                            let mode = ContentShowMode::parse_handle(content_mode_after.as_str())
                                .map(|mode| mode.as_str().to_string())
                                .unwrap_or_else(|| content_mode_after.as_str().to_string());
                            event.set("mode", mode)?;
                            if let Some(path) = &selected_path {
                                event.set("path", path.clone())?;
                            }
                            Ok(event)
                        },
                    )
                    .unwrap_or_else(|error| {
                        EventResult::Toast(AppToast::Warning(error.to_string()), false)
                    });
                    event_result = combine_event_results(event_result, callback_result);
                }
                if mode_before != state.mode {
                    let previous_mode = mode_label(&mode_before);
                    let mode = mode_label(&state.mode);
                    let callback_result =
                        dispatch_runtime_event(&mut state, "builtin.event.mode_changed", |lua| {
                            let event = lua.create_table()?;
                            event.set("previous_mode", previous_mode)?;
                            event.set("mode", mode)?;
                            Ok(event)
                        });
                    event_result = combine_event_results(event_result, callback_result);
                }
                if !help_open_before && matches!(state.mode, state::Mode::Help) {
                    let return_mode = match &state.help_return_mode {
                        mode => mode_label(mode),
                    };
                    let callback_result =
                        dispatch_runtime_event(&mut state, "builtin.event.help_opened", |lua| {
                            let event = lua.create_table()?;
                            event.set("return_mode", return_mode)?;
                            Ok(event)
                        });
                    event_result = combine_event_results(event_result, callback_result);
                }
                if help_open_before && !matches!(state.mode, state::Mode::Help) {
                    let mode = mode_label(&state.mode);
                    let callback_result =
                        dispatch_runtime_event(&mut state, "builtin.event.help_closed", |lua| {
                            let event = lua.create_table()?;
                            event.set("mode", mode)?;
                            Ok(event)
                        });
                    event_result = combine_event_results(event_result, callback_result);
                }
                if !logs_open_before && matches!(state.mode, state::Mode::Logs) {
                    let return_mode = mode_label(&state.logs_return_mode);
                    let callback_result =
                        dispatch_runtime_event(&mut state, "builtin.event.logs_opened", |lua| {
                            let event = lua.create_table()?;
                            event.set("return_mode", return_mode)?;
                            Ok(event)
                        });
                    event_result = combine_event_results(event_result, callback_result);
                }
                if logs_open_before && !matches!(state.mode, state::Mode::Logs) {
                    let mode = mode_label(&state.mode);
                    let callback_result =
                        dispatch_runtime_event(&mut state, "builtin.event.logs_closed", |lua| {
                            let event = lua.create_table()?;
                            event.set("mode", mode)?;
                            Ok(event)
                        });
                    event_result = combine_event_results(event_result, callback_result);
                }
                if !command_open_before && matches!(state.mode, state::Mode::Command) {
                    let return_mode = mode_label(&state.command_return_mode);
                    let callback_result =
                        dispatch_runtime_event(&mut state, "builtin.event.command_opened", |lua| {
                            let event = lua.create_table()?;
                            event.set("return_mode", return_mode)?;
                            Ok(event)
                        });
                    event_result = combine_event_results(event_result, callback_result);
                }
                if command_open_before && !matches!(state.mode, state::Mode::Command) {
                    let mode = mode_label(&state.mode);
                    let callback_result =
                        dispatch_runtime_event(&mut state, "builtin.event.command_closed", |lua| {
                            let event = lua.create_table()?;
                            event.set("mode", mode)?;
                            Ok(event)
                        });
                    event_result = combine_event_results(event_result, callback_result);
                }
                if !search_open_before && matches!(state.mode, state::Mode::Search) {
                    let previous_mode = mode_label(&mode_before);
                    let callback_result =
                        dispatch_runtime_event(&mut state, "builtin.event.search_opened", |lua| {
                            let event = lua.create_table()?;
                            event.set("previous_mode", previous_mode)?;
                            Ok(event)
                        });
                    event_result = combine_event_results(event_result, callback_result);
                }
                if search_open_before && !matches!(state.mode, state::Mode::Search) {
                    let mode = mode_label(&state.mode);
                    let callback_result =
                        dispatch_runtime_event(&mut state, "builtin.event.search_closed", |lua| {
                            let event = lua.create_table()?;
                            event.set("mode", mode)?;
                            Ok(event)
                        });
                    event_result = combine_event_results(event_result, callback_result);
                }
                match event_result {
                    EventResult::Quit => {
                        let closing_path = state.file_watch.path.clone();
                        let readonly = state.readonly;
                        let callback_result = configure::dispatch_lua_event(
                            &mut state,
                            "builtin.event.app_shutting_down",
                            |lua| {
                                let event = lua.create_table()?;
                                event.set("path", closing_path.clone())?;
                                event.set("readonly", readonly)?;
                                Ok(event)
                            },
                        )
                        .unwrap_or_else(|error| {
                            EventResult::Toast(AppToast::Warning(error.to_string()), false)
                        });
                        if !matches!(callback_result, EventResult::Continue) {
                            match callback_result {
                                EventResult::Quit => {}
                                EventResult::Redraw | EventResult::Copying => {
                                    redraw(terminal, &mut state, new_version.as_deref())?;
                                }
                                EventResult::ReloadFile { .. }
                                | EventResult::Configure { .. }
                                | EventResult::Error(_)
                                | EventResult::Toast(_, _)
                                | EventResult::Continue => {}
                            }
                        }
                        break;
                    }
                    EventResult::Continue => {}
                    EventResult::Redraw => {
                        redraw(terminal, &mut state, new_version.as_deref())?;
                    }
                    EventResult::Copying => {
                        state.copying = true;
                        redraw(terminal, &mut state, new_version.as_deref())?;
                        state.copying = false;
                        thread::sleep(std::time::Duration::from_millis(100));
                        redraw(terminal, &mut state, new_version.as_deref())?;
                    }
                    EventResult::ReloadFile { write } => {
                        match reload_current_file(&mut state, write) {
                            Ok(message) => {
                                terminal.clear()?;
                                terminal.flush()?;
                                apply_app_toast(&mut state, AppToast::Info(message));
                            }
                            Err(error) => {
                                apply_app_toast(&mut state, AppToast::Error(error.to_string()));
                            }
                        }
                        redraw(terminal, &mut state, new_version.as_deref())?;
                    }
                    EventResult::Configure { reset } => {
                        match open_configuration_and_reload(&mut state, tx_events.clone(), reset) {
                            Ok(toast) => {
                                terminal.clear()?;
                                terminal.flush()?;
                                apply_app_toast(&mut state, toast);
                            }
                            Err(error) => {
                                apply_app_toast(&mut state, AppToast::Error(error.to_string()));
                            }
                        }
                        redraw(terminal, &mut state, new_version.as_deref())?;
                    }
                    EventResult::Error(error) => {
                        draw_error(terminal, &error)?;
                        thread::sleep(std::time::Duration::from_secs(2));
                        redraw(terminal, &mut state, new_version.as_deref())?;
                    }
                    EventResult::Toast(toast, full_redraw) => {
                        if full_redraw {
                            state.compute_tree_view();
                            terminal.clear()?;
                            terminal.flush()?;
                        }
                        apply_app_toast(&mut state, toast);
                        redraw(terminal, &mut state, new_version.as_deref())?;
                    }
                }
            }
            AppEvent::ImageResized(resize_response) => match resize_response {
                ImageResizeResult::Success(resize_response) => {
                    if let Some(ref mut img_thread_protocol) = state.img_state.protocol {
                        let _ = img_thread_protocol.update_resized_protocol(resize_response);
                        redraw(terminal, &mut state, new_version.as_deref())?;
                    }
                }
                ImageResizeResult::Error(error) => {
                    state.img_state.error = Some(format!("Error resizing image: {error}"));
                    redraw(terminal, &mut state, new_version.as_deref())?;
                }
            },
            AppEvent::ImageLoad(img_load) => match img_load {
                ImageLoadedResult::Success {
                    key,
                    protocol,
                    clipboard_image,
                } => {
                    state.img_state.pending_keys.remove(&key);
                    state.img_state.cache_image(
                        key.clone(),
                        clipboard_image.clone(),
                        IMAGE_CACHE_CAPACITY,
                    );
                    if state.img_state.current_request_key() == Some(key) {
                        state.img_state.protocol = Some(protocol);
                        state.img_state.clipboard_image = Some(clipboard_image);
                        state.img_state.error = None;
                        redraw(terminal, &mut state, new_version.as_deref())?;
                    }
                }
                ImageLoadedResult::Failure { key, message } => {
                    state.img_state.pending_keys.remove(&key);
                    if state.img_state.current_request_key() == Some(key) {
                        state.img_state.protocol = None;
                        state.img_state.clipboard_image = None;
                        state.img_state.error = Some(message);
                        redraw(terminal, &mut state, new_version.as_deref())?;
                    }
                }
            },
            AppEvent::PreviewExpression(result) => {
                match result {
                    PreviewExpressionResult::Success { key, data_preview } => {
                        if state.preview_expression_state.pending_key.as_ref() == Some(&key) {
                            state.preview_expression_state.pending_key = None;
                            state.preview_expression_state.current_key = Some(key);
                            state.preview_expression_state.data_preview = Some(data_preview);
                            state.preview_expression_state.error = None;
                        }
                    }
                    PreviewExpressionResult::Failure { key, message } => {
                        if state.preview_expression_state.pending_key.as_ref() == Some(&key) {
                            state.preview_expression_state.pending_key = None;
                            state.preview_expression_state.current_key = Some(key);
                            state.preview_expression_state.data_preview = None;
                            state.preview_expression_state.error = Some(message);
                        }
                    }
                }
                redraw(terminal, &mut state, new_version.as_deref())?;
            }
            AppEvent::ContentPreview(result) => {
                match result {
                    super::ContentPreviewLoadedResult::Success { key, text } => {
                        if !crate::ui::preview::content::content_preview_is_current(
                            state.content_preview_state.pending_key.as_ref(),
                            &key,
                        ) {
                            continue;
                        }
                        state.content_preview_state.pending_key = None;
                        state.content_preview_state.error = None;
                        state
                            .content_preview_state
                            .cached
                            .retain(|entry| entry.key != key);
                        state
                            .content_preview_state
                            .cached
                            .push_back(state::CachedContentPreview { key, text });
                        while state.content_preview_state.cached.len()
                            > state::CONTENT_CACHE_CAPACITY
                        {
                            state.content_preview_state.cached.pop_front();
                        }
                    }
                    super::ContentPreviewLoadedResult::Failure { key, message } => {
                        if !crate::ui::preview::content::content_preview_is_current(
                            state.content_preview_state.pending_key.as_ref(),
                            &key,
                        ) {
                            continue;
                        }
                        state.content_preview_state.pending_key = None;
                        state.content_preview_state.error = Some((key, message));
                    }
                }
                redraw(terminal, &mut state, new_version.as_deref())?;
            }
            AppEvent::MatrixViewport(result) => {
                match result {
                    super::MatrixViewportLoadedResult::Success { key, data } => {
                        if state.matrix_viewport_state.pending_key.as_ref() != Some(&key) {
                            continue;
                        }
                        state.matrix_viewport_state.pending_key = None;
                        state.matrix_viewport_state.error = None;
                        state
                            .matrix_viewport_state
                            .cached
                            .retain(|entry| entry.key != key);
                        state
                            .matrix_viewport_state
                            .cached
                            .push_back(state::CachedMatrixViewport { key, data });
                        while state.matrix_viewport_state.cached.len()
                            > state::MATRIX_VIEWPORT_CACHE_CAPACITY
                        {
                            state.matrix_viewport_state.cached.pop_front();
                        }
                    }
                    super::MatrixViewportLoadedResult::Failure { key, message } => {
                        if state.matrix_viewport_state.pending_key.as_ref() != Some(&key) {
                            continue;
                        }
                        state.matrix_viewport_state.pending_key = None;
                        state.matrix_viewport_state.error = Some((key, message));
                    }
                }
                redraw(terminal, &mut state, new_version.as_deref())?;
            }
            AppEvent::PreviewChartLoad(image_loaded_result) => match image_loaded_result {
                ChartPreviewLoadedResult::Success {
                    key,
                    protocol,
                    clipboard_image,
                    data_bounds,
                    data_preview,
                } => {
                    state.chart_preview_state.cache_preview(
                        key.clone(),
                        clipboard_image.clone(),
                        data_bounds,
                        data_preview.clone(),
                        CHART_PREVIEW_CACHE_CAPACITY,
                    );
                    if state.chart_preview_state.pending_key.as_ref() == Some(&key) {
                        state.chart_preview_state.pending_key = None;
                    }
                    if state.chart_preview_state.current_request_key() != Some(key.clone()) {
                        continue;
                    }
                    state.chart_preview_state.protocol = Some(protocol);
                    state.chart_preview_state.clipboard_image = Some(clipboard_image);
                    state.chart_preview_state.error = None;
                    state.chart_preview_state.rendered_mode = Some(key.mode);
                    state.chart_preview_state.rendered_viewport = key.viewport;
                    state.chart_preview_state.rendered_roi = key.roi;
                    state.chart_preview_state.rendered_size = Some((key.width, key.height));
                    state
                        .chart_preview_state
                        .set_current_data(Some(data_preview));
                    state
                        .chart_preview_state
                        .sync_data_bounds(Some(data_bounds));
                    redraw(terminal, &mut state, new_version.as_deref())?;
                }
                ChartPreviewLoadedResult::Failure { key, message } => {
                    if state.chart_preview_state.pending_key.as_ref() == Some(&key) {
                        state.chart_preview_state.pending_key = None;
                    }
                    if state.chart_preview_state.current_request_key() != Some(key) {
                        continue;
                    }
                    state.chart_preview_state.error = Some(message);
                    redraw(terminal, &mut state, new_version.as_deref())?;
                }
            },
            AppEvent::PreviewChartResized(image_resize_result) => match image_resize_result {
                ImageResizeResult::Success(resize_response) => {
                    if let Some(ref mut protocol) = state.chart_preview_state.protocol {
                        let _ = protocol.update_resized_protocol(resize_response);
                        redraw(terminal, &mut state, new_version.as_deref())?;
                    }
                }
                ImageResizeResult::Error(error) => {
                    state.chart_preview_state.error =
                        Some(format!("Error resizing chart preview: {error}"));
                    redraw(terminal, &mut state, new_version.as_deref())?;
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
                                line_profile: page.line_profile,
                            });
                        while state.heatmap_render.cached_pages.len() > HEATMAP_CACHE_CAPACITY {
                            state.heatmap_render.cached_pages.pop_front();
                        }
                        if should_redraw {
                            redraw(terminal, &mut state, new_version.as_deref())?;
                        }
                    }
                }
                HeatmapLoadedResult::Failure { key, message } => {
                    state.heatmap_render.pending_keys.remove(&key);
                    if state.heatmap_render.current_key.as_ref() == Some(&key) {
                        apply_app_toast(
                            &mut state,
                            AppToast::Error(format!("Heatmap prefetch failed: {message}")),
                        );
                        redraw(terminal, &mut state, new_version.as_deref())?;
                    }
                }
                HeatmapLoadedResult::Dropped { key } => {
                    state.heatmap_render.pending_keys.remove(&key);
                }
            },
            AppEvent::MultiChartLoad(result) => {
                match result {
                    MultiChartLoadResult::Started { item_id, kind } => {
                        state.multi_chart.apply_load_started(item_id, kind);
                    }
                    MultiChartLoadResult::Success {
                        item_id,
                        kind,
                        points,
                        source_len,
                    } => {
                        let should_refresh_dependents =
                            matches!(kind, MultiChartLoadKind::Overview { .. });
                        if let Err(error) = state
                            .multi_chart
                            .apply_loaded_item(item_id, kind, points, source_len)
                        {
                            apply_app_toast(&mut state, AppToast::Error(error));
                        } else if should_refresh_dependents {
                            if let Err(error) =
                                state.multi_chart.refresh_expression_dependents_for_item(
                                    item_id,
                                    state.file.as_ref(),
                                )
                            {
                                apply_app_toast(&mut state, AppToast::Error(error));
                            }
                        }
                    }
                    MultiChartLoadResult::Failure {
                        item_id,
                        kind,
                        message,
                    } => {
                        state.multi_chart.apply_load_failure(item_id, kind, message);
                    }
                }
                if let Err(error) = state
                    .multi_chart
                    .queue_expression_detail_refresh(state.file.as_ref(), state.resolved_open_mode)
                {
                    apply_app_toast(&mut state, AppToast::Error(error));
                }
                redraw(terminal, &mut state, new_version.as_deref())?;
            }
            AppEvent::MultiChartExpressionRefresh(result) => {
                if let Err(error) = state.multi_chart.apply_expression_refresh_result(result) {
                    apply_app_toast(&mut state, AppToast::Error(error));
                }
                redraw(terminal, &mut state, new_version.as_deref())?;
            }
            AppEvent::MultiChartRender(result) => {
                state.multi_chart.apply_render_result(result);
                redraw(terminal, &mut state, new_version.as_deref())?;
            }
            AppEvent::TreeLoad(result) => {
                let (generation, request_id) = match &result {
                    TreeLoadResult::Success {
                        generation,
                        request_id,
                        ..
                    }
                    | TreeLoadResult::Failure {
                        generation,
                        request_id,
                        ..
                    } => (*generation, *request_id),
                };
                if !tree_load_is_current(
                    state.tree_load_generation,
                    state.pending_tree_loads.iter().map(|(id, _)| *id),
                    generation,
                    request_id,
                ) {
                    continue;
                }
                let Some(index) = state
                    .pending_tree_loads
                    .iter()
                    .position(|(id, _)| *id == request_id)
                else {
                    continue;
                };
                let (_, node) = state.pending_tree_loads.remove(index);
                match result {
                    TreeLoadResult::Success { children, .. } => {
                        node.borrow_mut().apply_enumerated_children(children)
                    }
                    TreeLoadResult::Failure { message, .. } => {
                        node.borrow_mut().apply_enumeration_error(message)
                    }
                }
                let selected_before = state.selected_tree_path();
                let selected_kind_before = selected_item_kind(&state).map(str::to_string);
                if let Err(error) = state.resume_tree_requests() {
                    apply_app_toast(&mut state, AppToast::Error(error.to_string()));
                }
                let selection_result = commit_selection_change(
                    &mut state,
                    selected_before,
                    selected_kind_before,
                    &tx_events,
                );
                if let EventResult::Toast(toast, _) = selection_result {
                    apply_app_toast(&mut state, toast);
                }
                state.request_selected_navigation_data();
                state.compute_tree_view();
                redraw(terminal, &mut state, new_version.as_deref())?;
            }
            AppEvent::NavigationLoad(result) => {
                let applied = match apply_navigation_load_result(&mut state, result) {
                    Ok(applied) => applied,
                    Err(error) => {
                        apply_app_toast(&mut state, AppToast::Error(error.to_string()));
                        true
                    }
                };
                if !applied {
                    continue;
                }
                redraw(terminal, &mut state, new_version.as_deref())?;
            }
            AppEvent::PreviewDebounceExpired(generation) => {
                if state.resolve_preview_debounce(generation) {
                    redraw(terminal, &mut state, new_version.as_deref())?;
                }
            }
            AppEvent::FileChanged => {
                if let Some(toast) = state.register_file_watch_change() {
                    apply_app_toast(&mut state, toast);
                    redraw(terminal, &mut state, new_version.as_deref())?;
                }
            }
        }
    }
    state.drain_tree_loads();
    state.drain_navigation_loads();
    state.drain_content_previews();
    state.drain_matrix_viewports();
    if let Some(file) = state.file.take() {
        file.close()?;
    }
    Ok(())
}

fn redraw(
    terminal: &mut AppTerminal,
    state: &mut AppState<'_>,
    new_version: Option<&str>,
) -> Result<()> {
    match panic::catch_unwind(AssertUnwindSafe(|| {
        draw_app_terminal_frame(terminal, state, new_version)
    })) {
        Ok(result) => {
            result?;
        }
        Err(payload) => {
            let message = recovered_ui_panic_message("rendering", payload);
            log_error(&message);
            apply_app_toast(state, AppToast::Error(message.clone()));
            match panic::catch_unwind(AssertUnwindSafe(|| {
                draw_error_terminal_frame(terminal, &message)
            })) {
                Ok(result) => {
                    result?;
                }
                Err(fallback_payload) => {
                    return Err(AppError::DrawingError(format!(
                        "{message}; fallback error screen also panicked: {}",
                        panic_payload_message(fallback_payload)
                    )));
                }
            }
        }
    }
    Ok(())
}

fn draw_error(terminal: &mut AppTerminal, error: &str) -> Result<()> {
    draw_error_terminal_frame(terminal, error)?;
    Ok(())
}

fn draw_app_terminal_frame(
    terminal: &mut AppTerminal,
    state: &mut AppState<'_>,
    new_version: Option<&str>,
) -> std::io::Result<()> {
    terminal.draw(|frame| {
        draw_app_frame(frame, state, new_version);
        strip_blink_modifiers(frame);
    })?;
    Ok(())
}

fn draw_error_terminal_frame(terminal: &mut AppTerminal, error: &str) -> std::io::Result<()> {
    terminal.draw(|frame| {
        render_error(frame, error);
        strip_blink_modifiers(frame);
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{navigation_load_is_current, startup_navigation_pending, tree_load_is_current};

    #[test]
    fn delayed_tree_result_is_rejected_after_reload() {
        assert!(!tree_load_is_current(2, [7].into_iter(), 1, 7));
    }

    #[test]
    fn distinct_pending_tree_expansions_are_retained() {
        assert!(tree_load_is_current(3, [11, 12].into_iter(), 3, 11));
        assert!(tree_load_is_current(3, [11, 12].into_iter(), 3, 12));
    }

    #[test]
    fn delayed_navigation_result_is_rejected_after_selection() {
        assert!(!navigation_load_is_current(2, Some(8), 1, 8));
        assert!(!navigation_load_is_current(2, Some(8), 2, 7));
        assert!(navigation_load_is_current(2, Some(8), 2, 8));
    }

    #[test]
    fn startup_commands_wait_for_navigation_metadata_and_attributes() {
        assert!(startup_navigation_pending(false, true));
        assert!(startup_navigation_pending(true, false));
        assert!(!startup_navigation_pending(false, false));
    }
}

fn combine_event_results(primary: EventResult, secondary: EventResult) -> EventResult {
    match secondary {
        EventResult::Continue => primary,
        other => other,
    }
}

fn commit_selection_change(
    state: &mut AppState<'_>,
    selected_before: Option<String>,
    selected_kind_before: Option<String>,
    tx_events: &std::sync::mpsc::Sender<AppEvent>,
) -> EventResult {
    let selected_after = state.selected_tree_path();
    if selected_before == selected_after {
        return EventResult::Continue;
    }
    state.navigation_generation = state.navigation_generation.wrapping_add(1);
    state.pending_navigation_request = None;
    if let Some(path) = &selected_after {
        let generation = state.begin_preview_debounce(path.clone());
        schedule_preview_debounce(tx_events.clone(), generation);
    } else {
        state.clear_preview_debounce();
    }

    let mut result = EventResult::Continue;
    if let Some(dataset_path) = selected_dataset_path(state) {
        let callback_result =
            configure::dispatch_lua_event(state, "builtin.event.dataset_opened", |lua| {
                let event = lua.create_table()?;
                event.set("path", dataset_path)?;
                Ok(event)
            })
            .unwrap_or_else(|error| {
                EventResult::Toast(AppToast::Warning(error.to_string()), false)
            });
        result = combine_event_results(result, callback_result);
    }
    let selected_kind_after = selected_item_kind(state).map(str::to_string);
    let callback_result = dispatch_runtime_event(state, "builtin.event.selection_changed", |lua| {
        let event = lua.create_table()?;
        if let Some(path) = selected_before {
            event.set("previous_path", path)?;
        }
        if let Some(kind) = selected_kind_before {
            event.set("previous_kind", kind)?;
        }
        if let Some(path) = selected_after {
            event.set("path", path)?;
        }
        if let Some(kind) = selected_kind_after {
            event.set("kind", kind)?;
        }
        Ok(event)
    });
    combine_event_results(result, callback_result)
}

fn dispatch_runtime_event(
    state: &mut AppState<'_>,
    handle: &str,
    payload: impl FnOnce(&mlua::Lua) -> std::result::Result<mlua::Table, mlua::Error>,
) -> EventResult {
    configure::dispatch_lua_event(state, handle, payload)
        .unwrap_or_else(|error| EventResult::Toast(AppToast::Warning(error.to_string()), false))
}

fn mode_label(mode: &state::Mode) -> &'static str {
    match mode {
        state::Mode::Normal => "normal",
        state::Mode::Search => "search",
        state::Mode::Help => "help",
        state::Mode::Logs => "logs",
        state::Mode::Command => "command",
        state::Mode::MultiChart => "mchart",
        state::Mode::AttributeCreateDialog => "attribute-create-dialog",
        state::Mode::AttributeDeleteDialog => "attribute-delete-dialog",
        state::Mode::FixedStringOverflowDialog => "fixed-string-overflow-dialog",
        state::Mode::FixedStringResizeDialog => "fixed-string-resize-dialog",
    }
}

fn focus_label(focus: &state::Focus) -> &'static str {
    match focus {
        state::Focus::Tree(_) => "tree",
        state::Focus::Attributes => "attributes",
        state::Focus::Content => "content",
    }
}

fn selected_item_kind(state: &AppState<'_>) -> Option<&'static str> {
    let item = state.treeview.get(state.tree_view_cursor)?;
    let node = item.node.try_borrow().ok()?;
    Some(match &node.node {
        Node::File(_) => "file",
        Node::Group(_, _) => "group",
        Node::Dataset(_, _) => "dataset",
        Node::Broken(_) => "broken",
    })
}

fn selected_dataset_path(state: &AppState<'_>) -> Option<String> {
    let item = state.treeview.get(state.tree_view_cursor)?;
    let node = item.node.borrow();
    matches!(&node.node, Node::Dataset(_, _)).then(|| node.node.path())
}

fn apply_startup_event_result(state: &mut AppState<'_>, event_result: EventResult) -> Result<bool> {
    match event_result {
        EventResult::Quit => Ok(true),
        EventResult::Continue | EventResult::Redraw | EventResult::Copying => Ok(false),
        EventResult::ReloadFile { write } => {
            match reload_current_file(state, write) {
                Ok(message) => apply_app_toast(state, AppToast::Info(message)),
                Err(error) => apply_app_toast(state, AppToast::Error(error.to_string())),
            }
            Ok(false)
        }
        EventResult::Configure { .. } => {
            apply_app_toast(
                state,
                AppToast::Info(
                    "The configure command is only available after startup completes".to_string(),
                ),
            );
            Ok(false)
        }
        EventResult::Error(error) => {
            apply_app_toast(state, AppToast::Error(error));
            Ok(false)
        }
        EventResult::Toast(toast, full_redraw) => {
            if full_redraw {
                state.compute_tree_view();
            }
            apply_app_toast(state, toast);
            Ok(false)
        }
    }
}

fn run_startup_commands(
    state: &mut AppState<'_>,
    startup_commands: &[StartupCommand],
    rx_events: &Receiver<AppEvent>,
) -> Result<bool> {
    for startup_command in startup_commands {
        let invocation = parse_command_text(&startup_command.command_text).map_err(|error| {
            AppError::InvalidCommand(format!("{}: {}", startup_command.origin, error))
        })?;
        let event_result = execute_command(state, &invocation).map_err(|error| {
            AppError::InvalidCommand(format!("{}: {}", startup_command.origin, error))
        })?;
        state.command_state.record_successful_command(&invocation);
        while startup_navigation_pending(
            state.pending_tree_selection.is_some(),
            state.pending_navigation_request.is_some(),
        ) {
            let event = rx_events.recv().map_err(|error| {
                AppError::ChannelError(format!("Failed to receive tree load: {error}"))
            })?;
            let AppEvent::TreeLoad(result) = event else {
                if let AppEvent::NavigationLoad(result) = event {
                    apply_navigation_load_result(state, result).map_err(|error| {
                        AppError::InvalidCommand(format!("{}: {}", startup_command.origin, error))
                    })?;
                }
                continue;
            };
            let (generation, request_id) = match &result {
                TreeLoadResult::Success {
                    generation,
                    request_id,
                    ..
                }
                | TreeLoadResult::Failure {
                    generation,
                    request_id,
                    ..
                } => (*generation, *request_id),
            };
            let Some(index) = (generation == state.tree_load_generation)
                .then(|| {
                    state
                        .pending_tree_loads
                        .iter()
                        .position(|(id, _)| *id == request_id)
                })
                .flatten()
            else {
                continue;
            };
            let (_, node) = state.pending_tree_loads.remove(index);
            match result {
                TreeLoadResult::Success { children, .. } => {
                    node.borrow_mut().apply_enumerated_children(children)
                }
                TreeLoadResult::Failure { message, .. } => {
                    node.borrow_mut().apply_enumeration_error(message)
                }
            }
            state.resume_tree_requests().map_err(|error| {
                AppError::InvalidCommand(format!("{}: {}", startup_command.origin, error))
            })?;
        }
        if apply_startup_event_result(state, event_result)? {
            return Ok(true);
        }
    }
    Ok(false)
}

struct WorkerShutdownGuard {
    running: Arc<AtomicBool>,
}

impl Drop for WorkerShutdownGuard {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
