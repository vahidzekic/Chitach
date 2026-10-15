//! Chitach desktop app (Tauri v2 backend).

pub mod commands;
pub mod crypto;
pub mod error;
pub mod events;
pub mod export;
pub mod fines;
pub mod fines_defs;
pub mod history;
pub mod license;
pub mod poller;
pub mod rfzo;
pub mod settings;
pub mod signing;
pub mod smartbox;
pub mod state;
pub mod tray;
pub mod update;

use tauri::{Manager, WindowEvent};

use crate::{
    settings::{Mode, Settings},
    signing::SigningState,
    smartbox::SmartboxState,
    state::Shared,
};

/// `CHITACH_MOCK=1` (or `true`) enables mock mode.
pub fn mock_enabled() -> bool {
    std::env::var("CHITACH_MOCK")
        .map(|v| matches!(v.trim(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let path = settings::settings_path(handle.path().app_config_dir().ok());
            let settings = Settings::load(path.as_deref());
            let mock = mock_enabled();
            if mock {
                log::info!("CHITACH_MOCK: using sample documents");
            }
            let mode = settings.mode;
            let run_in_background = settings.run_in_background;
            let secrets_path = history::secrets_path(path.as_deref());
            app.manage(history::HistoryState::new(secrets_path));
            let license_path = license::license_path(path.as_deref());
            app.manage(license::LicenseState::new(license_path, mock));
            app.manage(Shared::new(settings, mock, path));
            app.manage(SigningState::default());
            app.manage(SmartboxState::default());
            app.manage(fines::FinesCache::default());
            let fines_cache_dir = handle.path().app_cache_dir().ok().map(|d| d.join("fines"));
            app.manage(fines_defs::FinesDefinitions::new(fines_cache_dir));
            if !mock {
                fines_defs::spawn_refresh(handle.clone());
                license::spawn_revalidate(handle.clone());
            }

            if let Err(e) = tray::apply(&handle, run_in_background) {
                log::warn!("tray: {e}");
            }
            match mode {
                Mode::Reader => poller::spawn(handle),
                Mode::Smartbox => smartbox::spawn(handle),
                Mode::XmlSign => {}
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let background = app
                    .try_state::<Shared>()
                    .is_some_and(|s| s.settings().run_in_background);
                if background && app.tray_by_id(tray::TRAY_ID).is_some() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_state,
            commands::select_reader,
            commands::reload_card,
            commands::export_document,
            commands::suggest_file_name,
            commands::update_rfzo,
            commands::pin_tries_left,
            commands::change_pin,
            commands::list_certificates,
            commands::save_certificate,
            commands::get_settings,
            commands::save_settings,
            commands::check_update,
            commands::open_path,
            commands::sign_load_modules,
            commands::sign_list_slots,
            commands::sign_login,
            commands::sign_xml_file,
            commands::sign_logout,
            commands::smartbox_status,
            commands::fines_providers,
            commands::fines_check,
            commands::history_info,
            commands::history_set_api_key,
            commands::history_remove_api_key,
            commands::history_api_check,
            commands::license_info,
            commands::license_activate,
            commands::license_deactivate,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Chitach")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(s) = app.try_state::<SmartboxState>() {
                    s.stop();
                }
            }
        });
}
