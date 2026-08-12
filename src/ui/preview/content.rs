use std::{
    sync::mpsc::{channel, Sender},
    thread,
};

use crate::{
    h5f::{read_opaque_dataset_preview, read_string_dataset_preview},
    ui::{
        app::{AppEvent, ContentPreviewLoadedResult},
        state::{ContentPreviewKey, ContentPreviewWork},
    },
};

/// One bounded latest-request worker for text-only direct dataset content.
pub(crate) fn handle_content_preview_load(
    tx_events: Sender<AppEvent>,
) -> Sender<ContentPreviewWork> {
    let (tx_worker, rx_worker) = channel::<ContentPreviewWork>();
    thread::spawn(move || {
        while let Ok(work) = rx_worker.recv() {
            let ContentPreviewWork::Load(mut request) = work else {
                let ContentPreviewWork::Drain(done) = work else {
                    unreachable!()
                };
                let _ = done.send(());
                continue;
            };
            let mut pending_drains = Vec::new();
            while let Ok(queued) = rx_worker.try_recv() {
                match queued {
                    ContentPreviewWork::Load(next) => request = next,
                    // A drain is acknowledged below, after the active read.
                    ContentPreviewWork::Drain(done) => pending_drains.push(done),
                }
            }
            let key = request.key.clone();
            let result = if key.opaque {
                read_opaque_dataset_preview(
                    &request.dataset,
                    &request.meta,
                    key.value_start,
                    key.value_count,
                )
            } else {
                read_string_dataset_preview(
                    &request.dataset,
                    &request.meta.encoding,
                    key.value_start,
                    key.value_count,
                )
            };
            let event = match result {
                Ok(text) => ContentPreviewLoadedResult::Success { key, text },
                Err(error) => ContentPreviewLoadedResult::Failure {
                    key,
                    message: error.to_string(),
                },
            };
            if tx_events.send(AppEvent::ContentPreview(event)).is_err() {
                return;
            }
            for done in pending_drains {
                let _ = done.send(());
            }
        }
    });
    tx_worker
}

pub(crate) fn content_preview_is_current(
    pending: Option<&ContentPreviewKey>,
    key: &ContentPreviewKey,
) -> bool {
    pending == Some(key)
}

#[cfg(test)]
mod tests {
    use super::content_preview_is_current;
    use crate::ui::state::ContentPreviewKey;
    fn key(n: u64) -> ContentPreviewKey {
        ContentPreviewKey {
            file_generation: n,
            metadata_revision: 1,
            ds_path: "/d".into(),
            opaque: false,
            value_start: 0,
            value_count: 0,
        }
    }
    #[test]
    fn stale_content_result_is_rejected() {
        assert!(!content_preview_is_current(Some(&key(2)), &key(1)));
    }
}
