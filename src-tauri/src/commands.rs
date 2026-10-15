//! Tauri commands (see docs/04-api-ugovor.md).

use celik_core::Document;
use celik_sign::pkcs11::CardVendor;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::{
    crypto::{self, CertInfo},
    error::{with_detail, CmdResult},
    events::emit_state,
    export::{self, Format},
    fines::{self, FinesCache},
    settings::Settings,
    signing::{SigningState, SlotInfo, TokenCert},
    smartbox::{SmartboxState, SmartboxStatus},
    state::{AppState, Shared},
    tray, update,
};

/// Run blocking work off the main thread.
async fn blocking<T: Send + 'static>(
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
}

#[tauri::command]
pub fn get_app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
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
        let shared = app.state::<Shared>();
        let doc = current_document(&shared)?;
        let cyrillic = shared.settings().pdf_cyrillic_labels;
        let out = export::export_to(&doc, format, std::path::Path::new(&path), cyrillic)?;
        if let Some(dir) = out.parent() {
            let mut s = shared.settings_mut();
            s.last_dir = dir.to_string_lossy().into_owned();
            if let Some(p) = &shared.settings_path {
                if let Err(e) = s.save(p) {
                    log::warn!("saving settings: {e}");
                }
            }
        }
        Ok(out.to_string_lossy().into_owned())
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
pub async fn pin_tries_left(app: AppHandle) -> CmdResult<u8> {
    blocking(move || crypto::pin_tries_left(&app.state::<Shared>())).await
}

#[tauri::command]
pub async fn change_pin(app: AppHandle, old_pin: String, new_pin: String) -> CmdResult<()> {
    blocking(move || crypto::change_pin(&app.state::<Shared>(), &old_pin, &new_pin)).await
}

#[tauri::command]
pub async fn list_certificates(app: AppHandle) -> CmdResult<Vec<CertInfo>> {
    blocking(move || crypto::list_certificates(&app.state::<Shared>())).await
}

#[tauri::command]
pub async fn save_certificate(app: AppHandle, index: usize, path: String) -> CmdResult<()> {
    blocking(move || crypto::save_certificate(&app.state::<Shared>(), index, &path)).await
}

#[tauri::command]
pub fn get_settings(state: State<'_, Shared>) -> Settings {
    state.settings().clone()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> CmdResult<()> {
    let shared = app.state::<Shared>();
    let run_in_background = settings.run_in_background;
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

#[tauri::command]
pub async fn sign_load_modules(app: AppHandle) -> CmdResult<Vec<CardVendor>> {
    blocking(move || {
        let settings = app.state::<Shared>().settings().clone();
        app.state::<SigningState>().load_modules(&settings)
    })
    .await
}

#[tauri::command]
pub async fn sign_list_slots(app: AppHandle, vendor: CardVendor) -> CmdResult<Vec<SlotInfo>> {
    blocking(move || app.state::<SigningState>().list_slots(vendor)).await
}

#[tauri::command]
pub async fn sign_login(
    app: AppHandle,
    vendor: CardVendor,
    slot: u64,
    pin: String,
) -> CmdResult<Vec<TokenCert>> {
    blocking(move || app.state::<SigningState>().login(vendor, slot, &pin)).await
}

#[tauri::command]
pub async fn sign_xml_file(
    app: AppHandle,
    cert_id: String,
    input_path: String,
    output_path: String,
) -> CmdResult<()> {
    blocking(move || {
        app.state::<SigningState>()
            .sign_file(&cert_id, &input_path, &output_path)
    })
    .await
}

#[tauri::command]
pub async fn sign_logout(app: AppHandle) -> CmdResult<()> {
    blocking(move || app.state::<SigningState>().logout()).await
}

#[tauri::command]
pub fn smartbox_status(state: State<'_, SmartboxState>) -> SmartboxStatus {
    state.status()
}

#[tauri::command]
pub fn fines_providers(state: State<'_, Shared>) -> fines::FinesInfo {
    // Without a vehicle card (e.g. on the settings page) all providers are listed for an
    // empty plate.
    let plate = fines::document_plate(state.current_document().as_ref()).unwrap_or_default();
    fines::providers_for(&plate, &fines::providers())
}

#[tauri::command]
pub async fn fines_check(
    app: AppHandle,
    provider_id: String,
    force: Option<bool>,
) -> CmdResult<fines::CheckResult> {
    blocking(move || {
        let shared = app.state::<Shared>();
        let plate = fines::document_plate(shared.current_document().as_ref())?;
        let provider = fines::providers()
            .into_iter()
            .find(|p| p.id == provider_id)
            .ok_or_else(|| format!("error.finesUnknownProvider|{provider_id}"))?;
        fines::check_allowed(&shared.settings().fines, &provider)?;
        let mock = shared.mock;
        let cache = app.state::<FinesCache>();
        Ok(fines::check_with(
            &cache,
            &provider,
            &plate,
            force.unwrap_or(false),
            std::time::Instant::now(),
            |url| {
                if mock {
                    std::thread::sleep(std::time::Duration::from_millis(700));
                    fines::mock_parsed(&plate)
                } else {
                    fines::interpret(
                        provider
                            .check
                            .as_ref()
                            .map_or(fines::Parser::Unknown, |c| c.parser),
                        fines::fetch(url),
                    )
                }
            },
        ))
    })
    .await
}
