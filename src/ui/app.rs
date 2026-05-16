use std::{rc::Rc, thread, time::SystemTime};

use ratatui::{
    crossterm::event::{self},
    layout::{Alignment, Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    compat::RuntimeConfig,
    configure,
    error::{log_error, AppError},
    ui::{heatmap::HEATMAP_CACHE_CAPACITY, input::EventResult, state::AppToast},
    GIT_VERSION,
};

use self::boot::prepare_app;
use self::config::open_configuration_and_reload;
use self::dialogs::{
    render_attribute_create_dialog, render_attribute_delete_dialog,
    render_fixed_string_overflow_dialog, render_fixed_string_resize_dialog,
};
use self::events::{handle_file_watch_events, handle_term_events, schedule_preview_debounce};
use self::lifecycle::{
    classify_recover_loop_error, init_terminal, resolve_alternate_screen, restore_terminal,
    AppTerminal, RecoverLoopAction,
};
use self::reload::reload_current_file;
use self::update::check_for_available_update;
use super::state::{ChartPreviewKey, HeatmapLoadedPage, HeatmapRenderKey, ImageLoadKey};
use super::{
    command::{execute_command, parse_command_text, render_command_dialog, StartupCommand},
    help::render_help,
    input::handle_input_event,
    main_display::render_main_display,
    mchart::{MultiChartExpressionRefreshResult, MultiChartLoadResult},
    preview::image::{ImageResizeResult, IMAGE_CACHE_CAPACITY},
    state::{
        self, AppState, Focus, LastFocused, Mode, PreviewExpressionResult,
        CHART_PREVIEW_CACHE_CAPACITY,
    },
    tree_view::render_tree,
};

mod boot;
mod config;
mod dialogs;
mod events;
mod lifecycle;
mod reload;
mod update;

pub(super) fn primary_text_style() -> Style {
    let mut style = Style::default().fg(configure::themed_color(|colors| colors.text.primary));
    if configure::prefers_strong_text() {
        style = style.bold();
    }
    style
}

fn make_panels_rect(
    area: Rect,
    mode: &Mode,
    focus: &Focus,
    treeview: &[super::tree_view::TreeItem<'_>],
) -> Rc<[Rect]> {
    if let Mode::Search = mode {
        Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(100), Constraint::Percentage(0)])
            .split(area)
    } else {
        let layout = configure::current_auto_layout_settings();
        let tree_focus = match focus {
            Focus::Tree(_) => PanelFocus::Focused,
            Focus::Attributes | Focus::Content => PanelFocus::Unfocused,
        };
        let focused_tree_constraint =
            tree_constraint(&layout.tree.focused, preferred_tree_panel_width(treeview));
        let tree_constraint = match tree_focus {
            PanelFocus::Focused => focused_tree_constraint,
            PanelFocus::Unfocused => layout.tree.unfocused.as_constraint(),
        };
        if area.width < 100 {
            let chunks = Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([tree_constraint, Constraint::Fill(1)])
                .split(area);
            return chunks;
        }

        Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([tree_constraint, Constraint::Fill(1)])
            .split(area)
    }
}

fn preferred_tree_panel_width(treeview: &[super::tree_view::TreeItem<'_>]) -> Option<u16> {
    let widest_line = treeview.iter().map(|item| item.line.width() as u16).max()?;
    Some(widest_line.saturating_add(4).max(12))
}

fn tree_constraint(size: &configure::LayoutSize, preferred_width: Option<u16>) -> Constraint {
    match (size, preferred_width) {
        (configure::LayoutSize::Max(cap), Some(preferred)) => {
            Constraint::Length(preferred.min(*cap).max(12))
        }
        (configure::LayoutSize::Min(floor), Some(preferred)) => {
            Constraint::Length(preferred.max(*floor))
        }
        _ => size.as_constraint(),
    }
}

#[derive(Clone, Copy)]
enum PanelFocus {
    Focused,
    Unfocused,
}

pub(super) fn main_content_focus(focus: &Focus) -> LastFocused {
    match focus {
        Focus::Tree(last_focused) => last_focused.clone(),
        Focus::Attributes => LastFocused::Attributes,
        Focus::Content => LastFocused::Content,
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
    let use_alternate_screen = resolve_alternate_screen(runtime_config);
    let mut terminal = init_terminal(use_alternate_screen)?;

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
            Err(error) => match classify_recover_loop_error(error) {
                RecoverLoopAction::Retry(message) => last_message = Some(message),
                RecoverLoopAction::Break(message) => {
                    last_message = Some(message);
                    break;
                }
            },
        }
    }

    restore_terminal(use_alternate_screen, last_message)
}

fn main_recover_loop(
    terminal: &mut AppTerminal,
    filename: String,
    link: bool,
    writable: bool,
    runtime_config: RuntimeConfig,
    startup_commands: &[StartupCommand],
    new_version: Option<&str>,
) -> Result<IntendedMainLoopBreak> {
    let boot::PreparedApp {
        mut state,
        tx_events,
        rx_events,
    } = prepare_app(&filename, link, writable, runtime_config)?;

    if run_startup_commands(&mut state, startup_commands)? {
        return Ok(IntendedMainLoopBreak {});
    }

    let draw_closure = |frame: &mut Frame, state: &mut AppState| {
        let command_over_multichart = matches!(state.mode, Mode::Command)
            && matches!(state.command_return_mode, Mode::MultiChart);
        state.ui_layout = state::UiLayoutState::default();
        let content_area = render_header(frame, frame.area(), state, new_version);
        let (content_area, command_area) = match state.mode {
            Mode::Command => split_command_bar(content_area),
            _ => (content_area, Rect::new(0, 0, 0, 0)),
        };

        if let Mode::Help = state.mode {
            render_help(frame, content_area, state);
            render_toast_overlay(frame, state, command_area);
            return;
        }
        if matches!(state.mode, Mode::MultiChart) || command_over_multichart {
            state.multi_chart.render(frame, content_area);
            if matches!(state.mode, Mode::Command) {
                render_command_dialog(frame, command_area, state);
            }
            render_toast_overlay(frame, state, command_area);
            return;
        }

        let show_tree_view = state.show_tree_view;
        state.stacked_tree_layout =
            use_stacked_tree_layout(content_area, &state.mode, state.show_tree_view);

        let main_display_area = match show_tree_view {
            true => {
                let areas =
                    make_panels_rect(content_area, &state.mode, &state.focus, &state.treeview);
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
                let Some(selected_node) = state
                    .treeview
                    .get(state.tree_view_cursor)
                    .map(|item| item.node.clone())
                else {
                    render_error(frame, "Error: no tree node is currently selected");
                    return;
                };
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
        render_toast_overlay(frame, state, command_area);
    };

    // First time draw nice state
    terminal.draw(|f| draw_closure(f, &mut state))?;

    handle_term_events(tx_events.clone(), state.edit_pause.clone());
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
                state
                    .multi_chart
                    .schedule_viewport_detail_loads(state.file.as_ref());
                if let Err(error) = state
                    .multi_chart
                    .queue_expression_detail_refresh(state.file.as_ref())
                {
                    state.toast = AppToast::Error(error);
                }
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
                        terminal.draw(|f| {
                            draw_closure(f, &mut state);
                        })?;
                    }
                }
                ImageLoadedResult::Failure { key, message } => {
                    state.img_state.pending_keys.remove(&key);
                    if state.img_state.current_request_key() == Some(key) {
                        state.img_state.protocol = None;
                        state.img_state.clipboard_image = None;
                        state.img_state.error = Some(message);

                        terminal.draw(|f| {
                            draw_closure(f, &mut state);
                        })?;
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
                terminal.draw(|f| {
                    draw_closure(f, &mut state);
                })?;
            }
            AppEvent::PreviewChartLoad(image_loaded_result) => match image_loaded_result {
                ChartPreviewLoadedResult::Success {
                    key,
                    protocol,
                    clipboard_image,
                } => {
                    state.chart_preview_state.cache_preview(
                        key.clone(),
                        clipboard_image.clone(),
                        CHART_PREVIEW_CACHE_CAPACITY,
                    );
                    if state.chart_preview_state.pending_key.as_ref() == Some(&key) {
                        state.chart_preview_state.pending_key = None;
                    }
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
                    if state.chart_preview_state.pending_key.as_ref() == Some(&key) {
                        state.chart_preview_state.pending_key = None;
                    }
                    if state.chart_preview_state.current_request_key() != Some(key) {
                        continue;
                    }
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
                                line_profile: page.line_profile,
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
                        if let Err(error) = state
                            .multi_chart
                            .apply_loaded_item(item_id, kind, points, source_len)
                        {
                            state.toast = AppToast::Error(error);
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
                    state.toast = AppToast::Error(error);
                }
                terminal.draw(|f| {
                    draw_closure(f, &mut state);
                })?;
            }
            AppEvent::MultiChartExpressionRefresh(result) => {
                if let Err(error) = state.multi_chart.apply_expression_refresh_result(result) {
                    state.toast = AppToast::Error(error);
                }
                terminal.draw(|f| {
                    draw_closure(f, &mut state);
                })?;
            }
            AppEvent::MultiChartRender(result) => {
                state.multi_chart.apply_render_result(result);
                terminal.draw(|f| {
                    draw_closure(f, &mut state);
                })?;
            }
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
    PreviewExpression(PreviewExpressionResult),
    PreviewChartLoad(ChartPreviewLoadedResult),
    PreviewChartResized(ImageResizeResult),
    HeatmapLoad(HeatmapLoadedResult),
    MultiChartLoad(MultiChartLoadResult),
    MultiChartExpressionRefresh(MultiChartExpressionRefreshResult),
    MultiChartRender(crate::ui::mchart::MultiChartRenderResult),
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
    state: &mut AppState<'_>,
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

    let mchart_label = format!(
        " 📊 mchart [{}/{}] ",
        state.multi_chart.visible_item_count(),
        state.multi_chart.chart_items().len()
    );
    let mchart_style = if matches!(state.mode, Mode::MultiChart) {
        Style::default()
            .fg(configure::themed_color(|colors| colors.accent.selection_fg))
            .bg(configure::themed_color(|colors| colors.accent.selection_bg))
            .bold()
    } else {
        Style::default()
            .fg(configure::themed_color(|colors| colors.help.description))
            .bg(configure::themed_color(|colors| colors.surface.help_key_bg))
            .bold()
    };
    let right = Line::from(vec![
        Span::styled(mchart_label.clone(), mchart_style),
        Span::raw(" "),
        Span::styled(
            "(type ? for help)",
            Style::default().fg(configure::themed_color(|colors| colors.content.help_hint)),
        ),
    ]);
    let right_width = right.width() as u16;
    let mchart_width = Line::from(mchart_label.as_str()).width() as u16;
    let right_start_x = columns[2]
        .x
        .saturating_add(columns[2].width.saturating_sub(right_width));
    state.ui_layout.mchart_toggle = Some(Rect {
        x: right_start_x,
        y: columns[2].y,
        width: mchart_width,
        height: columns[2].height,
    });
    state.ui_layout.help_toggle = Some(Rect {
        x: right_start_x.saturating_add(mchart_width + 1),
        y: columns[2].y,
        width: right_width.saturating_sub(mchart_width + 1),
        height: columns[2].height,
    });
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

fn render_toast_overlay(frame: &mut Frame<'_>, state: &AppState, command_area: Rect) {
    let Some((label, message, accent_color)) = toast_parts(&state.toast) else {
        return;
    };
    let area = toast_overlay_area(frame.area(), command_area);
    if area.width == 0 || area.height == 0 {
        return;
    }

    let base_bg = configure::themed_color(|colors| colors.surface.title_bg);
    let base_fg = configure::themed_color(|colors| colors.text.primary);
    let label_fg = configure::themed_color(|colors| colors.surface.bg);
    let label_text = format!(" {label} ");
    let available_message_width = area
        .width
        .saturating_sub(label_text.chars().count() as u16)
        .saturating_sub(1) as usize;
    let message = truncate_to_width(message, available_message_width);

    let line = Line::from(vec![
        Span::styled(
            label_text,
            Style::default().fg(label_fg).bg(accent_color).bold(),
        ),
        Span::styled(" ", Style::default().bg(base_bg)),
        Span::styled(message, Style::default().fg(base_fg).bg(base_bg)),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(base_bg)),
        area,
    );
}

fn toast_overlay_area(frame_area: Rect, command_area: Rect) -> Rect {
    if frame_area.width == 0 || frame_area.height == 0 {
        return Rect::new(0, 0, 0, 0);
    }
    let y = if command_area.height > 0 && command_area.y > frame_area.y {
        command_area.y.saturating_sub(1)
    } else {
        frame_area.bottom().saturating_sub(1)
    };
    Rect::new(frame_area.x, y, frame_area.width, 1)
}

fn toast_parts(toast: &AppToast) -> Option<(&'static str, &str, ratatui::style::Color)> {
    match toast {
        AppToast::Empty => None,
        AppToast::Info(message) => Some((
            "INFO",
            message.as_str(),
            configure::themed_color(|colors| colors.toast.info),
        )),
        AppToast::Warning(message) => Some((
            "WARN",
            message.as_str(),
            configure::themed_color(|colors| colors.toast.warning),
        )),
        AppToast::Error(message) => Some((
            "ERROR",
            message.as_str(),
            configure::themed_color(|colors| colors.text.error),
        )),
    }
}

fn truncate_to_width(message: &str, max_width: usize) -> String {
    let char_count = message.chars().count();
    if char_count <= max_width {
        return message.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }
    let mut truncated = message.chars().take(max_width - 3).collect::<String>();
    truncated.push_str("...");
    truncated
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
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
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
