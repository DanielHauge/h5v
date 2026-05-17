use std::sync::mpsc::Sender;

use super::{
    app::AppEvent,
    state::{AppState, AppToast},
};

pub fn apply_app_toast(state: &mut AppState<'_>, toast: AppToast) {
    crate::logging::log_toast(&toast);
    state.toast = toast;
}

pub fn send_app_toast(tx_events: &Sender<AppEvent>, toast: AppToast) {
    crate::logging::log_toast(&toast);
    let _ = tx_events.send(AppEvent::Toast(toast));
}
