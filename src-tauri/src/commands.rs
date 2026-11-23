//! Tauri commands (see docs/04-api-ugovor.md).

use celik_core::Document;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::{
    error::{with_detail, CmdResult},
    events::emit_state,
    export::{self, Format},
    features::Feature,
    pro,
    settings::Settings,
    state::{AppState, Shared},
    tray, update,
};

/// Run blocking work off the main thread.
pub(crate) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| with_detail("error.internal", e))?
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub os: String,
    pub arch: String,
    /// Chitach Pro is compiled into this build (official builds); `false` = Chitach Free.
    pub pro: bool,
}

#[tauri::command]
pub fn get_app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        pro: pro::AVAILABLE,
    }
}

#[tauri::command]
pub fn get_state(state: State<'_, Shared>) -> AppState {
    state.snapshot()
}

#[tauri::command]
pub fn select_reader(state: State<'_, Shared>, name: Option<String>) {
    state.core().select_reader(name);
}

#[tauri::command]
pub fn reload_card(state: State<'_, Shared>) {
    state.core().request_reload();
}

fn current_document(shared: &Shared) -> CmdResult<Document> {
    shared
        .current_document()
        .ok_or_else(|| "error.noDocument".to_string())
}

#[tauri::command]
pub async fn export_document(app: AppHandle, format: Format, path: String) -> CmdResult<String> {
    blocking(move || {
        if format != Format::Pdf {
            pro::require(&app, Feature::DataExport)?;
        }
        let shared = app.state::<Shared>();
        let doc = current_document(&shared)?;
        let cyrillic = shared.settings().pdf_cyrillic_labels;
        let out = export::export_to_target(&app, &doc, format, &path, cyrillic)?;
        if export::is_uri(&out) {
            return Ok(out);
        }
        if let Some(dir) = std::path::Path::new(&out).parent() {
            let mut s = shared.settings_mut();
            s.last_dir = dir.to_string_lossy().into_owned();
            if let Some(p) = &shared.settings_path {
                if let Err(e) = s.save(p) {
                    log::warn!("saving settings: {e}");
                }
            }
        }
        Ok(out)
    })
    .await
}

#[tauri::command]
pub fn suggest_file_name(state: State<'_, Shared>, format: Format) -> CmdResult<String> {
    let doc = current_document(&state)?;
    Ok(export::suggest_file_name(&doc, format))
}

#[tauri::command]
pub async fn update_rfzo(app: AppHandle) -> CmdResult<Document> {
    let app2 = app.clone();
    let doc = blocking(move || crate::rfzo::update(&app2.state::<Shared>())).await?;
    emit_state(&app);
    Ok(doc)
}

#[tauri::command]
pub fn get_settings(state: State<'_, Shared>) -> Settings {
    state.settings().clone()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> CmdResult<()> {
    let shared = app.state::<Shared>();
    let run_in_background = settings.run_in_background && pro::allows(&app, Feature::Background);
    {
        let mut s = shared.settings_mut();
        *s = settings;
        if let Some(p) = &shared.settings_path {
            s.save(p)
                .map_err(|e| with_detail("error.saveSettings", e))?;
        }
    }
    if let Err(e) = tray::apply(&app, run_in_background) {
        log::warn!("tray: {e}");
    }
    Ok(())
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> CmdResult<update::UpdateInfo> {
    let current = app.package_info().version.to_string();
    blocking(move || update::check(&current)).await
}

#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> CmdResult<()> {
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| with_detail("error.openFile", e))
}
