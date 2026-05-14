use std::sync::mpsc::Sender;

use image::DynamicImage;
use ratatui_image::thread::ThreadProtocol;

use crate::ui::{
    app::{AppEvent, ChartPreviewLoadedResult, ImageLoadedResult},
    state::{ChartPreviewKey, ClipboardImageData, ImageLoadKey},
};

fn send_event(tx_events: &Sender<AppEvent>, event: AppEvent) {
    let _ = tx_events.send(event);
}

pub(super) fn send_image_failure(
    tx_events: &Sender<AppEvent>,
    key: ImageLoadKey,
    message: impl Into<String>,
) {
    send_event(
        tx_events,
        AppEvent::ImageLoad(ImageLoadedResult::Failure {
            key,
            message: message.into(),
        }),
    );
}

pub(super) fn clipboard_image_from_dynamic(dyn_img: &DynamicImage) -> ClipboardImageData {
    let rgba = dyn_img.to_rgba8();
    ClipboardImageData {
        width: rgba.width() as usize,
        height: rgba.height() as usize,
        bytes: rgba.into_raw(),
    }
}

pub(super) fn send_image_success(
    tx_events: &Sender<AppEvent>,
    key: ImageLoadKey,
    protocol: ThreadProtocol,
    clipboard_image: ClipboardImageData,
) {
    send_event(
        tx_events,
        AppEvent::ImageLoad(ImageLoadedResult::Success {
            key,
            protocol,
            clipboard_image,
        }),
    );
}

pub(super) fn send_chart_failure(
    tx_events: &Sender<AppEvent>,
    key: ChartPreviewKey,
    message: impl Into<String>,
) {
    send_event(
        tx_events,
        AppEvent::PreviewChartLoad(ChartPreviewLoadedResult::Failure {
            key,
            message: message.into(),
        }),
    );
}

pub(super) fn send_chart_success(
    tx_events: &Sender<AppEvent>,
    key: ChartPreviewKey,
    protocol: ThreadProtocol,
    clipboard_image: ClipboardImageData,
) {
    send_event(
        tx_events,
        AppEvent::PreviewChartLoad(ChartPreviewLoadedResult::Success {
            key,
            protocol,
            clipboard_image,
        }),
    );
}
