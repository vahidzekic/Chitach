//! Chitach Pro: fines check, vehicle history, PIN change and certificates, XML signing,
//! Smartbox and the licence. Built only with the `pro` feature; without it
//! `pro_stub.rs` takes this module's place and the app is Chitach Free.
//!
//! The rest of the app talks to Pro only through the functions below (same names and
//! signatures in `pro_stub.rs`).

pub mod commands;
pub mod crypto;
pub mod fines;
pub mod fines_defs;
pub mod history;
pub mod license;
pub mod signing;
pub mod smartbox;

use std::path::Path;

use tauri::{AppHandle, Manager};

use crate::{error::CmdResult, features::Feature, settings::Mode};

/// Pro is compiled into this build.
pub const AVAILABLE: bool = true;

/// Manage the Pro state and start its background work (definitions refresh, licence
/// re-validation). Called from the app's `setup`.
pub fn setup(app: &tauri::App, settings_path: Option<&Path>, mock: bool) {
    let handle = app.handle();
    app.manage(history::HistoryState::new(history::secrets_path(
        settings_path,
    )));
    app.manage(license::LicenseState::new(
        license::license_path(settings_path),
        mock,
    ));
    app.manage(signing::SigningState::default());
    app.manage(smartbox::SmartboxState::default());
    app.manage(fines::FinesCache::default());
    let fines_cache_dir = handle.path().app_cache_dir().ok().map(|d| d.join("fines"));
    app.manage(fines_defs::FinesDefinitions::new(fines_cache_dir));
    if !mock {
        fines_defs::spawn_refresh(handle.clone());
        license::spawn_revalidate(handle.clone());
    }
}

/// The feature may be used (licence active, or the gate is off in this build).
pub fn allows(app: &AppHandle, feature: Feature) -> bool {
    app.try_state::<license::LicenseState>()
        .is_some_and(|l| l.allows(feature))
}

/// `Ok` when the feature may be used, otherwise `error.proRequired`.
pub fn require(app: &AppHandle, feature: Feature) -> CmdResult<()> {
    match app.try_state::<license::LicenseState>() {
        Some(l) => l.require(feature),
        None => Err(crate::features::PRO_REQUIRED.into()),
    }
}

/// Start the work of a Pro mode (Smartbox server). Returns `false` for modes Pro does not
/// run in the background (the reader is started by the caller).
pub fn start_mode(app: &AppHandle, mode: Mode) -> bool {
    match mode {
        Mode::Reader => false,
        Mode::Smartbox => {
            if allows(app, Feature::Smartbox) {
                smartbox::spawn(app.clone());
            }
            true
        }
        Mode::XmlSign => true,
    }
}

/// Stop Pro background work on exit.
pub fn on_exit(app: &AppHandle) {
    if let Some(s) = app.try_state::<smartbox::SmartboxState>() {
        s.stop();
    }
}

/// Lists the Pro commands once: their names (to route calls) and their invoke handler.
macro_rules! pro_commands {
    ($($name:ident),* $(,)?) => {
        /// Names of the Pro commands.
        pub const COMMANDS: &[&str] = &[$(stringify!($name)),*];

        /// Invoke handler for [`COMMANDS`].
        pub fn invoke_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
            tauri::generate_handler![$(commands::$name),*]
        }
    };
}

pro_commands![
    pin_tries_left,
    change_pin,
    list_certificates,
    save_certificate,
    sign_load_modules,
    sign_list_slots,
    sign_login,
    sign_xml_file,
    sign_logout,
    smartbox_status,
    fines_providers,
    fines_check,
    history_info,
    history_set_api_key,
    history_remove_api_key,
    history_api_check,
    license_info,
    license_activate,
    license_deactivate,
];
