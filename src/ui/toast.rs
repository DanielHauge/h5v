use std::{
    sync::mpsc::Sender,
    time::{Duration, Instant},
};

use super::{
    app::AppEvent,
    state::{AppState, AppToast},
};

const TOAST_DURATION: Duration = Duration::from_secs(4);

pub fn apply_app_toast(state: &mut AppState<'_>, toast: AppToast) {
    crate::logging::log_toast(&toast);
    state.toast_expires_at = match &toast {
        AppToast::Empty => None,
        AppToast::Info(_) | AppToast::Warning(_) | AppToast::Error(_) => {
            Some(Instant::now() + TOAST_DURATION)
        }
    };
    state.toast = toast;
}

pub fn send_app_toast(tx_events: &Sender<AppEvent>, toast: AppToast) {
    crate::logging::log_toast(&toast);
    let _ = tx_events.send(AppEvent::Toast(toast));
}
