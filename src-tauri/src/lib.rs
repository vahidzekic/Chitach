//! Chitach desktop app (Tauri v2 backend).

pub mod commands;
pub mod error;
pub mod events;
pub mod export;
pub mod features;
pub mod poller;
#[cfg(feature = "pro")]
pub mod pro;
#[cfg(not(feature = "pro"))]
#[path = "pro_stub.rs"]
pub mod pro;
pub mod rfzo;
pub mod settings;
pub mod state;
pub mod transport;
pub mod tray;
pub mod update;

use tauri::{Manager, WindowEvent};

use crate::{
    features::Feature,
    settings::{Mode, Settings},
    state::Shared,
};

/// `CHITACH_MOCK=1` (or `true`) enables mock mode.
pub fn mock_enabled() -> bool {
    std::env::var("CHITACH_MOCK")
        .map(|v| matches!(v.trim(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    #[cfg(mobile)]
    let builder = builder
        .plugin(tauri_plugin_ccid::init())
        // Writes to the content:// URIs returned by Android's save dialog.
        .plugin(tauri_plugin_fs::init());
    builder
        .setup(|app| {
            let handle = app.handle().clone();
            let path = settings::settings_path(handle.path().app_config_dir().ok());
            let settings = Settings::load(path.as_deref());
            let mock = mock_enabled();
            if mock {
                log::info!("CHITACH_MOCK: using sample documents");
            }
            // Phones only have the reader: Smartbox and XML signing need desktop
            // PKCS#11 modules and a local web socket server.
            let mode = if cfg!(mobile) {
                Mode::Reader
            } else {
                settings.mode
            };
            let run_in_background = settings.run_in_background;
            pro::setup(app, path.as_deref(), mock);
            app.manage(Shared::new(settings, mock, path));
            transport::init(&handle);

            let run_in_background = run_in_background && pro::allows(&handle, Feature::Background);
            if let Err(e) = tray::apply(&handle, run_in_background) {
                log::warn!("tray: {e}");
            }
            if !pro::start_mode(&handle, mode) {
                poller::spawn(handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let background = app
                    .try_state::<Shared>()
                    .is_some_and(|s| s.settings().run_in_background);
                if background && tray::has_tray(app) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(invoke_handler())
        .build(tauri::generate_context!())
        .expect("error while building Chitach")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                pro::on_exit(app);
            }
        });
}

/// Free commands plus the Pro ones (none in Chitach Free), routed by name.
fn invoke_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    let free = with_invoke_type(tauri::generate_handler![
        commands::get_app_info,
        commands::get_state,
        commands::select_reader,
        commands::reload_card,
        commands::export_document,
        commands::suggest_file_name,
        commands::update_rfzo,
        commands::get_settings,
        commands::save_settings,
        commands::check_update,
        commands::open_path,
    ]);
    let pro = pro::invoke_handler();
    move |invoke| {
        if pro::COMMANDS.contains(&invoke.message.command()) {
            pro(invoke)
        } else {
            free(invoke)
        }
    }
}

/// Pins the closure's argument type (`generate_handler!` alone leaves it open).
fn with_invoke_type<F: Fn(tauri::ipc::Invoke) -> bool>(f: F) -> F {
    f
}
