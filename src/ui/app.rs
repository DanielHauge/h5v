use std::time::SystemTime;

use ratatui::crossterm::event;

use crate::{
    compat::RuntimeConfig,
    data::DatasetPlotingData,
    error::AppError,
    ui::{
        command::StartupCommand,
        mchart::{MultiChartExpressionRefreshResult, MultiChartLoadResult},
        preview::image::ImageResizeResult,
        state::{
            self, AppToast, ChartPreviewKey, HeatmapLoadedPage, HeatmapRenderKey, ImageLoadKey,
            PreviewExpressionResult,
        },
    },
};

use self::{
    lifecycle::{
        classify_recover_loop_error, init_terminal, resolve_alternate_screen, restore_terminal,
        RecoverLoopAction,
    },
    runtime::main_recover_loop,
    update::check_for_available_update,
};

pub(super) use self::render::{main_content_focus, primary_text_style};

mod boot;
mod config;
mod dialogs;
mod events;
mod lifecycle;
mod reload;
mod render;
mod runtime;
mod startup_progress;
mod update;

pub(crate) use self::startup_progress::render_startup_progress;

type Result<T> = std::result::Result<T, AppError>;

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

    restore_terminal(use_alternate_screen, last_message)
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
