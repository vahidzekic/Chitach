//! "smartbox" mode: Smartbox (ePorezi) emulation server.
//!
//! Thin adapter over `celik_sign::smartbox`. The PIN is supplied by the
//! ePorezi web client in each request, so no PIN prompt is
//! needed here; if `celik_sign` later adds a PIN provider callback, wire it in
//! [`start_server`].

use std::sync::Mutex;

use celik_sign::{pkcs11::CardVendor, smartbox::SmartboxServer};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    error::{catch, sign_error, CmdResult},
    events::SMARTBOX_STATUS,
    signing::module_paths,
    state::{lock, Shared},
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartboxStatus {
    pub running: bool,
    pub address: Option<String>,
    pub vendors: Vec<CardVendor>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct SmartboxState {
    server: Mutex<Option<SmartboxServer>>,
    status: Mutex<SmartboxStatus>,
}

impl SmartboxState {
    pub fn status(&self) -> SmartboxStatus {
        lock(&self.status).clone()
    }

    pub fn stop(&self) {
        if let Some(s) = lock(&self.server).take() {
            s.stop();
        }
        *lock(&self.status) = SmartboxStatus::default();
    }
}

fn start_server(paths: &[celik_sign::pkcs11::ModulePath]) -> CmdResult<SmartboxServer> {
    catch("error.smartbox", || {
        SmartboxServer::start(paths).map_err(|e| sign_error("error.smartbox", &e))
    })
}

/// Start the server in a background thread and emit `smartbox-status`.
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let settings = app.state::<Shared>().settings().clone();
        let result = catch("error.smartbox", || Ok(module_paths(&settings)))
            .and_then(|paths| start_server(&paths));
        let state = app.state::<SmartboxState>();
        let status = match result {
            Ok(server) => {
                let status = SmartboxStatus {
                    running: true,
                    address: Some(server.address.clone()),
                    vendors: server.vendors.clone(),
                    error: None,
                };
                *lock(&state.server) = Some(server);
                status
            }
            Err(e) => {
                log::error!("smartbox: {e}");
                SmartboxStatus {
                    error: Some(e),
                    ..Default::default()
                }
            }
        };
        *lock(&state.status) = status.clone();
        if let Err(e) = app.emit(SMARTBOX_STATUS, status) {
            log::warn!("emit smartbox-status: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_json() {
        let s = SmartboxStatus {
            running: true,
            address: Some("ws://127.0.0.1:1".into()),
            vendors: vec![CardVendor::Mup],
            error: None,
        };
        let v = serde_json::to_value(s).unwrap();
        assert_eq!(v["running"], true);
        assert_eq!(v["vendors"][0], "mup");
        assert!(v["error"].is_null());
    }
}
