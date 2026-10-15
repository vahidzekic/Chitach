//! Backend -> UI events (`app-state`, `notify`, `smartbox-status`).

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::Shared;

pub const APP_STATE: &str = "app-state";
pub const NOTIFY: &str = "notify";
pub const SMARTBOX_STATUS: &str = "smartbox-status";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Notify {
    pub level: Level,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

pub fn emit_state(app: &AppHandle) {
    let state = app.state::<Shared>().snapshot();
    if let Err(e) = app.emit(APP_STATE, state) {
        log::warn!("emit app-state: {e}");
    }
}

pub fn notify(app: &AppHandle, level: Level, key: &str, detail: Option<String>) {
    let payload = Notify {
        level,
        key: key.to_string(),
        detail,
    };
    if let Err(e) = app.emit(NOTIFY, payload) {
        log::warn!("emit notify: {e}");
    }
}

/// Notify about an error string following the `key|detail` convention.
pub fn notify_error(app: &AppHandle, err: &str) {
    let (key, detail) = match err.split_once('|') {
        Some((k, d)) => (k, Some(d.to_string())),
        None => (err, None),
    };
    notify(app, Level::Error, key, detail);
}
