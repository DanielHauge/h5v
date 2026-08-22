use std::time::SystemTime;

use image::Rgba;
use ratatui::crossterm::event;
use ratatui::crossterm::terminal;
use ratatui_image::{picker::Picker, FontSize};

use crate::{
    compat::RuntimeConfig,
    data::DatasetPlotingData,
    error::AppError,
    h5f::{ComputedAttributes, Node, RequestedOpenMode},
    ui::{
        command::StartupCommand,
        mchart::{MultiChartExpressionRefreshResult, MultiChartLoadResult},
        preview::image::ImageResizeResult,
        state::{
            self, AppToast, ChartPreviewKey, ContentPreviewKey, HeatmapLoadedPage,
            HeatmapRenderKey, ImageLoadKey, PreviewExpressionResult,
        },
    },
};

use self::{
    lifecycle::{
        classify_recover_loop_error, init_terminal, install_panic_terminal_restore_hook,
        resolve_alternate_screen, restore_terminal, RecoverLoopAction,
    },
    runtime::main_recover_loop,
    update::cached_available_update,
};

pub(super) use self::render::{main_content_focus, primary_text_style};

mod boot;
mod config;
mod dialogs;
mod events;
mod lifecycle;
mod picker_cache;
mod reload;
mod render;
mod runtime;
mod startup_progress;
mod update;

pub(crate) use self::startup_progress::render_startup_progress;

type Result<T> = std::result::Result<T, AppError>;

pub(super) fn terminal_cell_size(window: terminal::WindowSize) -> Option<(u16, u16)> {
    let width = window.width.checked_div(window.columns)?;
    let height = window.height.checked_div(window.rows)?;
    (width > 0 && height > 0).then_some((width, height))
}

pub(super) fn picker_with_cell_size(
    mut picker: Picker,
    cell_size: Option<(u16, u16)>,
    background: Rgba<u8>,
) -> Picker {
    if let Some((width, height)) = cell_size {
        let current = picker.font_size();
        if (current.width, current.height) != (width, height) {
            let protocol = picker.protocol_type();
            #[allow(deprecated)]
            let mut replacement = Picker::from_fontsize(FontSize::new(width, height));
            replacement.set_protocol_type(protocol);
            picker = replacement;
        }
    }
    picker.set_background_color(Some(background));
    picker
}

#[cfg(test)]
mod picker_tests {
    use image::Rgba;
    use ratatui::crossterm::terminal::WindowSize;
    use ratatui_image::{picker::ProtocolType, FontSize};

    use super::{picker_with_cell_size, terminal_cell_size, Picker};

    #[test]
    fn live_cell_size_overrides_cached_picker_metrics() {
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize(FontSize::new(8, 16));
        picker.set_protocol_type(ProtocolType::Kitty);
        let picker = picker_with_cell_size(picker, Some((9, 18)), Rgba([1, 2, 3, 255]));
        let font_size = picker.font_size();
        assert_eq!((font_size.width, font_size.height), (9, 18));
        assert_eq!(picker.protocol_type(), ProtocolType::Kitty);
    }

    #[test]
    fn unavailable_window_pixels_keep_picker_metrics() {
        #[allow(deprecated)]
        let picker = Picker::from_fontsize(FontSize::new(8, 16));
        let picker = picker_with_cell_size(
            picker,
            terminal_cell_size(WindowSize {
                columns: 120,
                rows: 40,
                width: 0,
                height: 0,
            }),
            Rgba([1, 2, 3, 255]),
        );
        let font_size = picker.font_size();
        assert_eq!((font_size.width, font_size.height), (8, 16));
    }
}

pub fn init(
    filename: String,
    link: bool,
    requested_open_mode: RequestedOpenMode,
    runtime_config: RuntimeConfig,
    startup_commands: &[StartupCommand],
) -> Result<()> {
    let use_alternate_screen = resolve_alternate_screen(runtime_config);
    let panic_hook = install_panic_terminal_restore_hook(use_alternate_screen);
    let mut terminal = init_terminal(use_alternate_screen)?;

    let new_ver = cached_available_update(SystemTime::now());
    let mut last_message = None;

    loop {
        match main_recover_loop(
            &mut terminal,
            filename.clone(),
            link,
            requested_open_mode,
            runtime_config,
            startup_commands,
            new_ver.as_deref(),
        ) {
            Ok(()) => break,
            Err(error) => match classify_recover_loop_error(error) {
                RecoverLoopAction::Retry(message) => last_message = Some(message),
                RecoverLoopAction::Break(message) => {
                    last_message = Some(message);
                    break;
                }
            },
        }
    }

    let result = restore_terminal(use_alternate_screen, last_message);
    drop(panic_hook);
    result
}

#[allow(clippy::large_enum_variant)]
pub enum AppEvent {
    TermEvent(event::Event),
    UpdateAvailable(Option<String>),
    ImageResized(ImageResizeResult),
    ImageLoad(ImageLoadedResult),
    PreviewExpression(PreviewExpressionResult),
    ContentPreview(ContentPreviewLoadedResult),
    MatrixViewport(MatrixViewportLoadedResult),
    PreviewChartLoad(ChartPreviewLoadedResult),
    PreviewChartResized(ImageResizeResult),
    HeatmapLoad(HeatmapLoadedResult),
    MultiChartLoad(MultiChartLoadResult),
    MultiChartExpressionRefresh(MultiChartExpressionRefreshResult),
    MultiChartRender(crate::ui::mchart::MultiChartRenderResult),
    PreviewDebounceExpired(u64),
    ResizeDebounceExpired(u64),
    TreeLoad(TreeLoadResult),
    NavigationLoad(NavigationLoadResult),
    Toast(AppToast),
    FileChanged,
}

pub enum MatrixViewportLoadedResult {
    Success {
        key: state::MatrixViewportKey,
        data: state::MatrixViewportData,
    },
    Failure {
        key: state::MatrixViewportKey,
        message: String,
    },
}

pub enum ContentPreviewLoadedResult {
    Success {
        key: ContentPreviewKey,
        text: String,
    },
    Failure {
        key: ContentPreviewKey,
        message: String,
    },
}

pub struct NavigationLoadRequest {
    pub generation: u64,
    pub request_id: u64,
    pub node: Node,
}

pub enum NavigationLoadWork {
    Load(NavigationLoadRequest),
    Drain(std::sync::mpsc::Sender<()>),
}

pub enum NavigationLoadResult {
    Metadata {
        generation: u64,
        request_id: u64,
        node: Node,
    },
    Attributes {
        generation: u64,
        request_id: u64,
        attributes: ComputedAttributes,
    },
    Failure {
        generation: u64,
        request_id: u64,
        metadata: bool,
        message: String,
    },
}

pub struct TreeLoadRequest {
    pub generation: u64,
    pub request_id: u64,
    pub node: Node,
}

pub enum TreeLoadWork {
    Load(TreeLoadRequest),
    Drain(std::sync::mpsc::Sender<()>),
}

pub enum TreeLoadResult {
    Success {
        generation: u64,
        request_id: u64,
        children: Vec<Node>,
    },
    Failure {
        generation: u64,
        request_id: u64,
        message: String,
    },
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
        data_bounds: state::PreviewChartViewport,
        data_preview: DatasetPlotingData,
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
