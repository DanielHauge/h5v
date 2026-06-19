use std::{
    sync::mpsc::{channel, Sender},
    thread,
};

use image::DynamicImage;
use ratatui_image::thread::ThreadProtocol;

use crate::data::DatasetPlotingData;
use crate::ui::{
    app::{AppEvent, ChartPreviewLoadedResult, ImageLoadedResult},
    mchart::background::evaluate_preview_expression,
    state::{
        ChartPreviewKey, ClipboardImageData, ImageLoadKey, PreviewChartViewport,
        PreviewExpressionRequest, PreviewExpressionResult,
    },
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
    data_bounds: PreviewChartViewport,
    data_preview: DatasetPlotingData,
) {
    send_event(
        tx_events,
        AppEvent::PreviewChartLoad(ChartPreviewLoadedResult::Success {
            key,
            protocol,
            clipboard_image,
            data_bounds,
            data_preview,
        }),
    );
}

pub(super) fn send_preview_expression_result(
    tx_events: &Sender<AppEvent>,
    result: PreviewExpressionResult,
) {
    send_event(tx_events, AppEvent::PreviewExpression(result));
}

pub(crate) fn handle_preview_expression_eval(
    tx_events: Sender<AppEvent>,
) -> Sender<PreviewExpressionRequest> {
    let (tx_eval, rx_eval) = channel::<PreviewExpressionRequest>();
    thread::spawn(move || loop {
        let Ok(mut request) = rx_eval.recv() else {
            return;
        };
        while let Ok(next_request) = rx_eval.try_recv() {
            request = next_request;
        }
        let result = match evaluate_preview_expression(
            &request.items,
            &request.key.expression,
            request.file_path.as_deref(),
            request.open_mode,
        ) {
            Ok(data_preview) => PreviewExpressionResult::Success {
                key: request.key,
                data_preview,
            },
            Err(message) => PreviewExpressionResult::Failure {
                key: request.key,
                message,
            },
        };
        send_preview_expression_result(&tx_events, result);
    });
    tx_eval
}
