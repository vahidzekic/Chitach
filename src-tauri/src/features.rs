//! Paid (Pro) features: the one list of what Chitach Pro unlocks. Public, because the
//! free code (export, auto-save, tray) asks whether a feature is allowed; the licence
//! itself lives in the Pro module (`pro/`, see `pro_stub.rs`).

use serde::Serialize;

/// Error returned by gated commands.
pub const PRO_REQUIRED: &str = "error.proRequired";

/// Paid (Pro) features. One place that says what is paid; every gate names its feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Feature {
    /// JSON and Excel export (PDF stays free).
    DataExport,
    /// Automatic save when a card is read.
    AutoSave,
    /// Running in the system tray.
    Background,
    /// Changing the ID card PIN.
    Pin,
    /// Listing and saving the ID card certificates.
    Certificates,
    /// XML signing (PKCS#11).
    XmlSign,
    /// Smartbox emulation for ePorezi.
    Smartbox,
    /// Fines check.
    Fines,
    /// Vehicle history.
    History,
}

impl Feature {
    pub const ALL: [Feature; 9] = [
        Feature::DataExport,
        Feature::AutoSave,
        Feature::Background,
        Feature::Pin,
        Feature::Certificates,
        Feature::XmlSign,
        Feature::Smartbox,
        Feature::Fines,
        Feature::History,
    ];
}
