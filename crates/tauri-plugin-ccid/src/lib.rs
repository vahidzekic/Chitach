//! USB CCID smart-card readers on Android.
//!
//! Phones have no PC/SC service, so the Kotlin side of this plugin
//! (`android/src/main/java/CcidPlugin.kt`) talks to the reader directly over the
//! Android USB host API using the CCID protocol: list readers, ask for USB
//! permission, power the card on (ATR) and exchange APDUs.
//!
//! The Rust side only forwards calls. Everything here blocks, so call it from a
//! worker thread, never from the main thread.
//!
//! On other platforms the plugin registers nothing and every call returns
//! [`Error::Unsupported`].

use serde::{Deserialize, Serialize};
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "rs.chitach.ccid";

/// A USB smart-card reader seen by the phone.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbReader {
    /// Android USB device name (`/dev/bus/usb/...`), used to address the reader.
    pub id: String,
    /// Human readable name (manufacturer and product).
    pub label: String,
    /// The user allowed the app to use this reader.
    pub permission: bool,
    /// A card is inserted (only known once permission is granted).
    pub card_present: bool,
    /// The reader only speaks TPDU (T=1 blocks), which is not supported yet.
    #[serde(default)]
    pub tpdu_only: bool,
}

#[derive(Debug)]
pub enum Error {
    /// No USB reader support on this platform.
    Unsupported,
    /// Error reported by the Kotlin side.
    Plugin(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported => {
                f.write_str("USB card readers are not supported on this platform")
            }
            Error::Plugin(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Deserialize)]
struct ReadersResponse {
    readers: Vec<UsbReader>,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Deserialize)]
struct HexResponse {
    hex: String,
}

/// Access to USB readers; get it with [`CcidExt::ccid`].
pub struct Ccid<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "android"))]
    _marker: std::marker::PhantomData<fn() -> R>,
}

#[cfg(target_os = "android")]
impl<R: Runtime> Ccid<R> {
    fn call<T: serde::de::DeserializeOwned>(
        &self,
        command: &str,
        payload: serde_json::Value,
    ) -> Result<T> {
        self.handle
            .run_mobile_plugin(command, payload)
            .map_err(|e| Error::Plugin(e.to_string()))
    }

    /// Connected CCID readers.
    pub fn readers(&self) -> Result<Vec<UsbReader>> {
        self.call::<ReadersResponse>("readers", serde_json::json!({}))
            .map(|r| r.readers)
    }

    /// Show the system dialog asking to use the reader. The answer shows up in
    /// [`UsbReader::permission`] on a later [`Ccid::readers`] call.
    pub fn request_permission(&self, id: &str) -> Result<()> {
        self.call::<serde_json::Value>("requestPermission", serde_json::json!({ "id": id }))
            .map(|_| ())
    }

    /// Power the card on and return its ATR.
    pub fn connect(&self, id: &str) -> Result<Vec<u8>> {
        let r: HexResponse = self.call("connect", serde_json::json!({ "id": id }))?;
        from_hex(&r.hex)
    }

    /// Send one APDU and return the response including SW1 SW2.
    pub fn transmit(&self, id: &str, apdu: &[u8]) -> Result<Vec<u8>> {
        let r: HexResponse = self.call(
            "transmit",
            serde_json::json!({ "id": id, "apdu": to_hex(apdu) }),
        )?;
        from_hex(&r.hex)
    }
}

#[cfg(not(target_os = "android"))]
impl<R: Runtime> Ccid<R> {
    pub fn readers(&self) -> Result<Vec<UsbReader>> {
        Err(Error::Unsupported)
    }
    pub fn request_permission(&self, _id: &str) -> Result<()> {
        Err(Error::Unsupported)
    }
    pub fn connect(&self, _id: &str) -> Result<Vec<u8>> {
        Err(Error::Unsupported)
    }
    pub fn transmit(&self, _id: &str, _apdu: &[u8]) -> Result<Vec<u8>> {
        Err(Error::Unsupported)
    }
}

/// Uppercase hex without separators (the format the Kotlin side uses).
pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

pub fn from_hex(s: &str) -> Result<Vec<u8>> {
    let s = s.trim();
    if !s.len().is_multiple_of(2) {
        return Err(Error::Plugin(format!("bad hex from plugin: {s}")));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| Error::Plugin(format!("bad hex from plugin: {s}")))
        })
        .collect()
}

/// Gives `AppHandle`, `App` and windows access to the plugin.
pub trait CcidExt<R: Runtime> {
    fn ccid(&self) -> &Ccid<R>;
}

impl<R: Runtime, T: Manager<R>> CcidExt<R> for T {
    fn ccid(&self) -> &Ccid<R> {
        self.state::<Ccid<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("ccid")
        .setup(|app, _api| {
            #[cfg(target_os = "android")]
            let ccid = Ccid {
                handle: _api.register_android_plugin(PLUGIN_IDENTIFIER, "CcidPlugin")?,
            };
            #[cfg(not(target_os = "android"))]
            let ccid = Ccid::<R> {
                _marker: std::marker::PhantomData,
            };
            app.manage(ccid);
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let bytes = [0x00, 0xA4, 0x04, 0x0C, 0xFF];
        assert_eq!(to_hex(&bytes), "00A4040CFF");
        assert_eq!(from_hex("00a4040cff").unwrap(), bytes);
        assert!(from_hex("ABC").is_err());
        assert!(from_hex("ZZ").is_err());
    }

    #[test]
    fn reader_json() {
        let r: ReadersResponse = serde_json::from_str(
            r#"{"readers":[{"id":"/dev/bus/usb/001/002","label":"ACS ACR39U","permission":true,"cardPresent":false}]}"#,
        )
        .unwrap();
        assert_eq!(r.readers[0].label, "ACS ACR39U");
        assert!(!r.readers[0].tpdu_only);
    }
}
