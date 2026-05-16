use std::{
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc, RwLock,
    },
    thread,
    time::Duration,
};

use ratatui::crossterm::event;

use crate::error::log_error;

use super::AppEvent;

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
