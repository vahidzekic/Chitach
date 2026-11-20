//! Connecting to the card in a reader.
//!
//! Desktop: PC/SC (the reader name is the PC/SC reader name).
//! Phones: USB CCID readers through `tauri-plugin-ccid` (the reader name is the
//! label the plugin reports; [`usb_readers`] keeps the label -> USB device map).

use celik_core::CardTransport;
#[cfg(desktop)]
use tauri::AppHandle;

/// A connected card, whatever the transport.
pub type Card = Box<dyn CardTransport + Send + Sync>;

#[cfg(desktop)]
pub fn init(_app: &AppHandle) {}

#[cfg(desktop)]
pub fn connect(reader: &str) -> celik_core::Result<Card> {
    Ok(Box::new(celik_core::reader::PcscCard::connect(reader)?))
}

#[cfg(mobile)]
pub use usb::{connect, init, request_permission, usb_readers};

#[cfg(mobile)]
mod usb {
    use std::{
        collections::BTreeMap,
        sync::{Mutex, OnceLock},
    };

    use celik_core::{CardError, CardTransport};
    use tauri::AppHandle;
    use tauri_plugin_ccid::{CcidExt, UsbReader};

    use super::Card;

    static APP: OnceLock<AppHandle> = OnceLock::new();
    /// Reader label -> USB device id, from the last [`usb_readers`] call.
    static IDS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

    pub fn init(app: &AppHandle) {
        let _ = APP.set(app.clone());
    }

    fn app() -> celik_core::Result<&'static AppHandle> {
        APP.get().ok_or(CardError::NoReader)
    }

    fn map_err(e: tauri_plugin_ccid::Error) -> CardError {
        match e {
            tauri_plugin_ccid::Error::Unsupported => CardError::NoReader,
            tauri_plugin_ccid::Error::Plugin(m) => match m.as_str() {
                "no card" => CardError::NoCard,
                "no reader" | "no USB permission" | "cannot open reader" => CardError::NoReader,
                _ => CardError::Pcsc(m),
            },
        }
    }

    /// Connected USB readers; remembers their ids for [`connect`].
    pub fn usb_readers() -> celik_core::Result<Vec<UsbReader>> {
        let list = app()?.ccid().readers().map_err(map_err)?;
        let mut ids = IDS.lock().unwrap_or_else(|p| p.into_inner());
        ids.clear();
        for r in &list {
            ids.insert(r.label.clone(), r.id.clone());
        }
        Ok(list)
    }

    /// Ask the user (system dialog) to allow the app to use a reader.
    pub fn request_permission(id: &str) {
        if let Ok(app) = app() {
            if let Err(e) = app.ccid().request_permission(id) {
                log::warn!("USB permission request: {e}");
            }
        }
    }

    struct UsbCard {
        id: String,
        atr: Vec<u8>,
    }

    impl CardTransport for UsbCard {
        fn atr(&self) -> celik_core::Result<Vec<u8>> {
            Ok(self.atr.clone())
        }

        fn transmit(&self, apdu: &[u8]) -> celik_core::Result<Vec<u8>> {
            let ccid = app()?.ccid();
            celik_core::card::transmit_with_status_handling(apdu, |a| {
                ccid.transmit(&self.id, a).map_err(map_err)
            })
        }
    }

    pub fn connect(reader: &str) -> celik_core::Result<Card> {
        let id = IDS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(reader)
            .cloned()
            .ok_or(CardError::NoReader)?;
        let atr = app()?.ccid().connect(&id).map_err(map_err)?;
        Ok(Box::new(UsbCard { id, atr }))
    }
}
