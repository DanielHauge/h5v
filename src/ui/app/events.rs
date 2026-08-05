use std::{
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Sender},
        Arc, RwLock,
    },
    thread,
    time::Duration,
};

use ratatui::crossterm::event;

use crate::{
    error::log_error,
    h5f::{enumerate_group_children, H5FNode},
};

use super::{AppEvent, NavigationLoadResult, NavigationLoadWork, TreeLoadResult, TreeLoadWork};

pub(super) fn handle_navigation_load(tx_events: Sender<AppEvent>) -> Sender<NavigationLoadWork> {
    let (tx_worker, rx_worker) = channel::<NavigationLoadWork>();
    thread::spawn(move || {
        while let Ok(work) = rx_worker.recv() {
            let NavigationLoadWork::Load(request) = work else {
                let NavigationLoadWork::Drain(done) = work else {
                    unreachable!()
                };
                let _ = done.send(());
                continue;
            };
            let mut node = H5FNode::new(request.node);
            if matches!(node.node, crate::h5f::Node::Dataset(_, _))
                && node.ensure_dataset_meta().is_err()
            {
                let _ = tx_events.send(AppEvent::NavigationLoad(NavigationLoadResult::Failure {
                    generation: request.generation,
                    request_id: request.request_id,
                    metadata: true,
                    message: node
                        .load_error
                        .unwrap_or_else(|| "Failed to read dataset metadata".to_string()),
                }));
                continue;
            }
            if tx_events
                .send(AppEvent::NavigationLoad(NavigationLoadResult::Metadata {
                    generation: request.generation,
                    request_id: request.request_id,
                    node: node.node.clone(),
                }))
                .is_err()
            {
                return;
            }
            match node.read_attributes() {
                Ok(_) => {
                    let attributes = node
                        .computed_attributes
                        .take()
                        .expect("attributes were cached");
                    let _ = tx_events.send(AppEvent::NavigationLoad(
                        NavigationLoadResult::Attributes {
                            generation: request.generation,
                            request_id: request.request_id,
                            attributes,
                        },
                    ));
                }
                Err(error) => {
                    let _ =
                        tx_events.send(AppEvent::NavigationLoad(NavigationLoadResult::Failure {
                            generation: request.generation,
                            request_id: request.request_id,
                            metadata: false,
                            message: error.to_string(),
                        }));
                }
            }
        }
    });
    tx_worker
}

pub(super) fn handle_tree_load(tx_events: Sender<AppEvent>) -> Sender<TreeLoadWork> {
    let (tx_worker, rx_worker) = channel::<TreeLoadWork>();
    thread::spawn(move || {
        while let Ok(work) = rx_worker.recv() {
            let TreeLoadWork::Load(request) = work else {
                let TreeLoadWork::Drain(done) = work else {
                    unreachable!()
                };
                let _ = done.send(());
                continue;
            };
            let result = match enumerate_group_children(&request.node) {
                Ok(children) => TreeLoadResult::Success {
                    generation: request.generation,
                    request_id: request.request_id,
                    children,
                },
                Err(error) => TreeLoadResult::Failure {
                    generation: request.generation,
                    request_id: request.request_id,
                    message: error.to_string(),
                },
            };
            if tx_events.send(AppEvent::TreeLoad(result)).is_err() {
                return;
            }
        }
    });
    tx_worker
}

pub(super) fn schedule_preview_debounce(tx_events: Sender<AppEvent>, generation: u64) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(95));
        let _ = tx_events.send(AppEvent::PreviewDebounceExpired(generation));
    });
}

pub(super) fn handle_file_watch_events(
    tx_events: Sender<AppEvent>,
    path: String,
    running: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let mut last_modified = fs::metadata(&path)
            .ok()
            .and_then(|metadata| metadata.modified().ok());
        while running.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(500));
            if !running.load(Ordering::Relaxed) {
                return;
            }
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

pub(super) fn handle_term_events(
    tx_events: Sender<AppEvent>,
    paused: Arc<RwLock<()>>,
    running: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            if event::poll(std::time::Duration::from_millis(16)).is_ok() {
                if !running.load(Ordering::Relaxed) {
                    return;
                }
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
                        Err(e) => {
                            log_error(e);
                            return;
                        }
                    }
                }
            }
        }
    });
}
