//! Tauri commands of the Pro features (see docs/04-api-ugovor.md).

use celik_sign::pkcs11::CardVendor;
use tauri::{AppHandle, Manager, State};

use crate::{
    commands::blocking,
    error::CmdResult,
    export,
    pro::{
        crypto::{self, CertInfo},
        fines::{self, FinesCache},
        fines_defs::FinesDefinitions,
        history::{self, HistoryState},
        license::{Feature, LicenseInfo, LicenseState},
        signing::{SigningState, SlotInfo, TokenCert},
        smartbox::{SmartboxState, SmartboxStatus},
    },
    state::Shared,
};

#[tauri::command]
pub async fn pin_tries_left(app: AppHandle) -> CmdResult<u8> {
    blocking(move || crypto::pin_tries_left(&app.state::<Shared>())).await
}

#[tauri::command]
pub async fn change_pin(app: AppHandle, old_pin: String, new_pin: String) -> CmdResult<()> {
    blocking(move || {
        app.state::<LicenseState>().require(Feature::Pin)?;
        crypto::change_pin(&app.state::<Shared>(), &old_pin, &new_pin)
    })
    .await
}

#[tauri::command]
pub async fn list_certificates(app: AppHandle) -> CmdResult<Vec<CertInfo>> {
    blocking(move || {
        app.state::<LicenseState>().require(Feature::Certificates)?;
        crypto::list_certificates(&app.state::<Shared>())
    })
    .await
}

#[tauri::command]
pub async fn save_certificate(app: AppHandle, index: usize, path: String) -> CmdResult<()> {
    blocking(move || {
        app.state::<LicenseState>().require(Feature::Certificates)?;
        let der = crypto::certificate_der(&app.state::<Shared>(), index)?;
        export::write_target(&app, &path, &der)
    })
    .await
}

#[tauri::command]
pub async fn sign_load_modules(app: AppHandle) -> CmdResult<Vec<CardVendor>> {
    blocking(move || {
        app.state::<LicenseState>().require(Feature::XmlSign)?;
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
    blocking(move || {
        app.state::<LicenseState>().require(Feature::XmlSign)?;
        app.state::<SigningState>().login(vendor, slot, &pin)
    })
    .await
}

#[tauri::command]
pub async fn sign_xml_file(
    app: AppHandle,
    cert_id: String,
    input_path: String,
    output_path: String,
) -> CmdResult<()> {
    blocking(move || {
        app.state::<LicenseState>().require(Feature::XmlSign)?;
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
pub fn fines_providers(
    state: State<'_, Shared>,
    defs: State<'_, FinesDefinitions>,
) -> fines::FinesInfo {
    // Without a vehicle card (e.g. on the settings page) all providers are listed for an
    // empty plate.
    let plate = fines::document_plate(state.current_document().as_ref()).unwrap_or_default();
    let mut info = fines::providers_for(&plate, &defs.providers());
    info.definitions = Some(defs.info());
    info
}

#[tauri::command]
pub async fn fines_check(
    app: AppHandle,
    provider_id: String,
    force: Option<bool>,
) -> CmdResult<fines::CheckResult> {
    blocking(move || {
        app.state::<LicenseState>().require(Feature::Fines)?;
        let shared = app.state::<Shared>();
        let plate = fines::document_plate(shared.current_document().as_ref())?;
        let provider = app
            .state::<FinesDefinitions>()
            .providers()
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
            |req| {
                if mock {
                    std::thread::sleep(std::time::Duration::from_millis(700));
                    fines::mock_parsed(&plate)
                } else {
                    fines::interpret(req, fines::fetch(req))
                }
            },
        ))
    })
    .await
}

#[tauri::command]
pub fn history_info(app: AppHandle) -> history::HistoryInfo {
    let shared = app.state::<Shared>();
    let doc = shared.current_document();
    let vehicle = history::document_vehicle(doc.as_ref()).ok();
    let secrets = app.state::<HistoryState>().secrets().clone();
    history::info_for(
        vehicle,
        &history::providers(),
        &history::WmiTable::embedded(),
        &secrets,
        chrono::Datelike::year(&chrono::Local::now()),
    )
}

fn history_provider(id: &str) -> CmdResult<history::Provider> {
    history::providers()
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("error.historyUnknownProvider|{id}"))
}

#[tauri::command]
pub fn history_set_api_key(
    state: State<'_, HistoryState>,
    provider_id: String,
    api_key: String,
    secret: String,
) -> CmdResult<()> {
    let p = history_provider(&provider_id)?;
    if p.kind != history::Kind::Api {
        return Err("error.historyNotApi".into());
    }
    let (api_key, secret) = (api_key.trim().to_string(), secret.trim().to_string());
    if api_key.is_empty() {
        return Err("error.historyNoKey".into());
    }
    state.set_key(&provider_id, Some(history::ApiKey { api_key, secret }))
}

#[tauri::command]
pub fn history_remove_api_key(
    state: State<'_, HistoryState>,
    provider_id: String,
) -> CmdResult<()> {
    state.set_key(&provider_id, None)
}

#[tauri::command]
pub async fn history_api_check(
    app: AppHandle,
    provider_id: String,
    operation: String,
    force: Option<bool>,
) -> CmdResult<history::ApiResult> {
    blocking(move || {
        app.state::<LicenseState>().require(Feature::History)?;
        let shared = app.state::<Shared>();
        let doc = shared.current_document();
        let vin = history::document_vehicle(doc.as_ref())?
            .vehicle_id_number
            .clone();
        let provider = history_provider(&provider_id)?;
        let state = app.state::<HistoryState>();
        let secrets = state.secrets().clone();
        let settings = shared.settings().history.clone();
        let (def, key) = history::api_allowed(&settings, &provider, &operation, &secrets)?;
        let mock = shared.mock;
        Ok(history::report_with(
            &state.cache,
            &provider,
            def,
            key,
            &operation,
            &vin,
            force.unwrap_or(false),
            std::time::Instant::now(),
            |url| {
                if mock {
                    std::thread::sleep(std::time::Duration::from_millis(700));
                    history::mock_parsed(&operation, &vin)
                } else {
                    history::interpret(def, &operation, history::fetch(url))
                }
            },
        ))
    })
    .await
}

#[tauri::command]
pub fn license_info(state: State<'_, LicenseState>) -> LicenseInfo {
    state.info()
}

#[tauri::command]
pub async fn license_activate(app: AppHandle, key: String) -> CmdResult<LicenseInfo> {
    blocking(move || app.state::<LicenseState>().activate(&key)).await
}

#[tauri::command]
pub async fn license_deactivate(app: AppHandle) -> CmdResult<LicenseInfo> {
    blocking(move || app.state::<LicenseState>().deactivate()).await
}
