//! Chitach Free: stands in for the Pro module (`pro/`, built with the `pro` feature) with
//! the same functions. Nothing is paid here because nothing paid is compiled in: no Pro
//! commands, and every [`Feature`] is off.

use std::path::Path;

use tauri::AppHandle;

use crate::{
    error::CmdResult,
    features::{Feature, PRO_REQUIRED},
    settings::Mode,
};

/// Pro is not compiled into this build.
pub const AVAILABLE: bool = false;

pub fn setup(_app: &tauri::App, _settings_path: Option<&Path>, _mock: bool) {}

pub fn allows(_app: &AppHandle, _feature: Feature) -> bool {
    false
}

pub fn require(_app: &AppHandle, _feature: Feature) -> CmdResult<()> {
    Err(PRO_REQUIRED.into())
}

/// Free has only the reader mode; returns `false` so the caller starts the reader.
pub fn start_mode(_app: &AppHandle, _mode: Mode) -> bool {
    false
}

pub fn on_exit(_app: &AppHandle) {}

pub const COMMANDS: &[&str] = &[];

pub fn invoke_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    |_| false
}
