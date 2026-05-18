use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};

use crate::{
    compat::RuntimeConfig,
    configure,
    error::{log_error, AppError},
    h5f::{HasPath, Node},
    ui::{
        command::{execute_command, parse_command_text, StartupCommand},
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
    AppEvent, ChartPreviewLoadedResult, HeatmapLoadedResult, ImageLoadedResult,
};

type Result<T> = std::result::Result<T, AppError>;

pub(super) fn main_recover_loop(
    terminal: &mut AppTerminal,
    filename: String,
    link: bool,
    writable: bool,
    runtime_config: RuntimeConfig,
    startup_commands: &[StartupCommand],
    new_version: Option<&str>,
) -> Result<()> {
    let super::boot::PreparedApp {
        mut state,
        tx_events,
        rx_events,
    } = prepare_app(&filename, link, writable, runtime_config)?;

    if run_startup_commands(&mut state, startup_commands)? {
        return Ok(());
    }

    redraw(terminal, &mut state, new_version)?;

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
        let event = match rx_events.recv() {
            Ok(event) => event,
            Err(error) => {
                log_error(error);
                return Err(AppError::ChannelError(format!(
                    "Failed to receive event from channel: {error}"
                )));
            }
        };
        if state.editing {
            continue;
        }

        match event {
            AppEvent::Toast(toast) => {
                state.toast = toast;
                redraw(terminal, &mut state, new_version)?;
            }
            AppEvent::TermEvent(event) => {
                let selected_before = state.selected_tree_path();
                let content_mode_before = state.active_content_mode_handle();
                let help_open_before = matches!(state.mode, state::Mode::Help);
                let multichart_open_before = matches!(state.mode, state::Mode::MultiChart);
                let mut event_result =
                    handle_input_event(&mut state, event).unwrap_or_else(|error| {
                        EventResult::Toast(AppToast::Error(error.to_string()), false)
                    });
                state
                    .multi_chart
                    .schedule_viewport_detail_loads(state.file.as_ref());
                if let Err(error) = state
                    .multi_chart
                    .queue_expression_detail_refresh(state.file.as_ref())
                {
                    apply_app_toast(&mut state, AppToast::Error(error));
                }
                let selected_after = state.selected_tree_path();
                if selected_before != selected_after {
                    if let Some(path) = selected_after {
                        let generation = state.begin_preview_debounce(path);
                        schedule_preview_debounce(tx_events.clone(), generation);
                    } else {
                        state.clear_preview_debounce();
                    }
                    if let Some(dataset_path) = selected_dataset_path(&state) {
                        let opened_path = dataset_path.clone();
                        let callback_result = configure::dispatch_lua_event(
                            &mut state,
                            "builtin.event.dataset_opened",
                            |lua| {
                                let event = lua.create_table()?;
                                event.set("path", opened_path.clone())?;
                                Ok(event)
                            },
                        )
                        .unwrap_or_else(|error| {
                            EventResult::Toast(AppToast::Warning(error.to_string()), false)
                        });
                        event_result = combine_event_results(event_result, callback_result);
                    }
                }
                if !multichart_open_before && matches!(state.mode, state::Mode::MultiChart) {
                    let selected_path = state.selected_tree_path();
                    let callback_result = configure::dispatch_lua_event(
                        &mut state,
                        "builtin.event.multichart_opened",
                        |lua| {
                            let event = lua.create_table()?;
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
                if !help_open_before && matches!(state.mode, state::Mode::Help) {
                    let return_mode = match &state.help_return_mode {
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
                    };
                    let callback_result = configure::dispatch_lua_event(
                        &mut state,
                        "builtin.event.help_opened",
                        |lua| {
                            let event = lua.create_table()?;
                            event.set("return_mode", return_mode)?;
                            Ok(event)
                        },
                    )
                    .unwrap_or_else(|error| {
                        EventResult::Toast(AppToast::Warning(error.to_string()), false)
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
                                    apply_app_toast(&mut state, AppToast::Empty);
                                    redraw(terminal, &mut state, new_version)?;
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
                        apply_app_toast(&mut state, AppToast::Empty);
                        redraw(terminal, &mut state, new_version)?;
                    }
                    EventResult::Copying => {
                        apply_app_toast(&mut state, AppToast::Empty);
                        state.copying = true;
                        redraw(terminal, &mut state, new_version)?;
                        state.copying = false;
                        thread::sleep(std::time::Duration::from_millis(100));
                        redraw(terminal, &mut state, new_version)?;
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
                        redraw(terminal, &mut state, new_version)?;
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
                        redraw(terminal, &mut state, new_version)?;
                    }
                    EventResult::Error(error) => {
                        draw_error(terminal, &error)?;
                        thread::sleep(std::time::Duration::from_secs(2));
                        redraw(terminal, &mut state, new_version)?;
                    }
                    EventResult::Toast(toast, full_redraw) => {
                        if full_redraw {
                            state.compute_tree_view();
                            terminal.clear()?;
                            terminal.flush()?;
                        }
                        apply_app_toast(&mut state, toast);
                        redraw(terminal, &mut state, new_version)?;
                    }
                }
            }
            AppEvent::ImageResized(resize_response) => match resize_response {
                ImageResizeResult::Success(resize_response) => {
                    if let Some(ref mut img_thread_protocol) = state.img_state.protocol {
                        let _ = img_thread_protocol.update_resized_protocol(resize_response);
                        redraw(terminal, &mut state, new_version)?;
                    }
                }
                ImageResizeResult::Error(error) => {
                    state.img_state.error = Some(format!("Error resizing image: {error}"));
                    redraw(terminal, &mut state, new_version)?;
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
                        redraw(terminal, &mut state, new_version)?;
                    }
                }
                ImageLoadedResult::Failure { key, message } => {
                    state.img_state.pending_keys.remove(&key);
                    if state.img_state.current_request_key() == Some(key) {
                        state.img_state.protocol = None;
                        state.img_state.clipboard_image = None;
                        state.img_state.error = Some(message);
                        redraw(terminal, &mut state, new_version)?;
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
                redraw(terminal, &mut state, new_version)?;
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
                    state.chart_preview_state.rendered_viewport = key.viewport;
                    state.chart_preview_state.rendered_roi = key.roi;
                    state
                        .chart_preview_state
                        .set_current_data(Some(data_preview));
                    state
                        .chart_preview_state
                        .sync_data_bounds(Some(data_bounds));
                    redraw(terminal, &mut state, new_version)?;
                }
                ChartPreviewLoadedResult::Failure { key, message } => {
                    if state.chart_preview_state.pending_key.as_ref() == Some(&key) {
                        state.chart_preview_state.pending_key = None;
                    }
                    if state.chart_preview_state.current_request_key() != Some(key) {
                        continue;
                    }
                    state.chart_preview_state.error = Some(message);
                    redraw(terminal, &mut state, new_version)?;
                }
            },
            AppEvent::PreviewChartResized(image_resize_result) => match image_resize_result {
                ImageResizeResult::Success(resize_response) => {
                    if let Some(ref mut protocol) = state.chart_preview_state.protocol {
                        let _ = protocol.update_resized_protocol(resize_response);
                        redraw(terminal, &mut state, new_version)?;
                    }
                }
                ImageResizeResult::Error(error) => {
                    state.chart_preview_state.error =
                        Some(format!("Error resizing chart preview: {error}"));
                    redraw(terminal, &mut state, new_version)?;
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
                            redraw(terminal, &mut state, new_version)?;
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
                        redraw(terminal, &mut state, new_version)?;
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
                    .queue_expression_detail_refresh(state.file.as_ref())
                {
                    apply_app_toast(&mut state, AppToast::Error(error));
                }
                redraw(terminal, &mut state, new_version)?;
            }
            AppEvent::MultiChartExpressionRefresh(result) => {
                if let Err(error) = state.multi_chart.apply_expression_refresh_result(result) {
                    apply_app_toast(&mut state, AppToast::Error(error));
                }
                redraw(terminal, &mut state, new_version)?;
            }
            AppEvent::MultiChartRender(result) => {
                state.multi_chart.apply_render_result(result);
                redraw(terminal, &mut state, new_version)?;
            }
            AppEvent::PreviewDebounceExpired(generation) => {
                if state.resolve_preview_debounce(generation) {
                    redraw(terminal, &mut state, new_version)?;
                }
            }
            AppEvent::FileChanged => {
                if let Some(toast) = state.register_file_watch_change() {
                    apply_app_toast(&mut state, toast);
                    redraw(terminal, &mut state, new_version)?;
                }
            }
        }
    }
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
    terminal.draw(|frame| draw_app_frame(frame, state, new_version))?;
    Ok(())
}

fn draw_error(terminal: &mut AppTerminal, error: &str) -> Result<()> {
    terminal.draw(|frame| render_error(frame, error))?;
    Ok(())
}

fn combine_event_results(primary: EventResult, secondary: EventResult) -> EventResult {
    match secondary {
        EventResult::Continue => primary,
        other => other,
    }
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

struct WorkerShutdownGuard {
    running: Arc<AtomicBool>,
}

impl Drop for WorkerShutdownGuard {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
