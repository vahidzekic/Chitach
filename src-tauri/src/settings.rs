//! User settings, stored as JSON in the app config directory.

use std::path::{Path, PathBuf};

use celik_sign::pkcs11::{CardVendor, ModulePath};
use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[serde(rename = "sr-Latn")]
    SrLatn,
    #[serde(rename = "sr-Cyrl")]
    SrCyrl,
    #[serde(rename = "en")]
    #[default]
    En,
    #[serde(rename = "ru")]
    Ru,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    #[default]
    Reader,
    Smartbox,
    XmlSign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AutoSave {
    #[default]
    Off,
    Save,
    SaveAndOpen,
}

/// PKCS#11 module path per vendor; empty string = default path for the OS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Pkcs11Paths {
    pub mup: String,
    pub pks: String,
    pub posta: String,
    pub halcom: String,
    pub esmart: String,
}

pub const ALL_VENDORS: [CardVendor; 5] = [
    CardVendor::Mup,
    CardVendor::Pks,
    CardVendor::Posta,
    CardVendor::Halcom,
    CardVendor::Esmart,
];

impl Pkcs11Paths {
    pub fn get(&self, vendor: CardVendor) -> &str {
        match vendor {
            CardVendor::Mup => &self.mup,
            CardVendor::Pks => &self.pks,
            CardVendor::Posta => &self.posta,
            CardVendor::Halcom => &self.halcom,
            CardVendor::Esmart => &self.esmart,
        }
    }

    /// Module paths for all vendors, substituting OS defaults for empty entries.
    pub fn module_paths(&self, default_path: impl Fn(CardVendor) -> String) -> Vec<ModulePath> {
        ALL_VENDORS
            .iter()
            .filter_map(|&vendor| {
                let p = self.get(vendor).trim();
                let path = if p.is_empty() {
                    default_path(vendor)
                } else {
                    p.to_string()
                };
                (!path.is_empty()).then_some(ModulePath { vendor, path })
            })
            .collect()
    }
}

/// Fines check ("Provera kazni") preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FinesSettings {
    /// Run the automatic check when a vehicle card is read.
    pub auto_check: bool,
    /// The user agreed that the plate is sent to the automatic-check portals.
    pub consent_given: bool,
    /// Automatic-check providers that may be used; `None` = all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_providers: Option<Vec<String>>,
    /// Download newer signed provider definitions from GitHub (at most daily).
    pub remote_definitions: bool,
}

impl Default for FinesSettings {
    fn default() -> Self {
        FinesSettings {
            auto_check: false,
            consent_given: false,
            enabled_providers: None,
            remote_definitions: true,
        }
    }
}

impl FinesSettings {
    pub fn provider_enabled(&self, id: &str) -> bool {
        self.enabled_providers
            .as_ref()
            .is_none_or(|ids| ids.iter().any(|x| x == id))
    }
}

/// Vehicle history ("Istorija vozila") preferences. API keys are not stored here but in
/// `secrets.json` (see `history.rs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct HistorySettings {
    /// The user agreed that the VIN is sent to the configured history APIs.
    pub consent_given: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub language: Language,
    pub theme: Theme,
    pub mode: Mode,
    pub pdf_cyrillic_labels: bool,
    pub auto_save: AutoSave,
    pub auto_save_dir: String,
    pub run_in_background: bool,
    pub check_updates: bool,
    pub last_dir: String,
    pub pkcs11_paths: Pkcs11Paths,
    pub fines: FinesSettings,
    pub history: HistorySettings,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: Language::En,
            theme: Theme::System,
            mode: Mode::Reader,
            pdf_cyrillic_labels: false,
            auto_save: AutoSave::Off,
            auto_save_dir: String::new(),
            run_in_background: false,
            check_updates: true,
            last_dir: String::new(),
            pkcs11_paths: Pkcs11Paths::default(),
            fines: FinesSettings::default(),
            history: HistorySettings::default(),
        }
    }
}

/// Default UI language for an OS locale (e.g. `sr-RS`, `sr_RS@latin`, `en-US`).
pub fn language_for_locale(locale: Option<&str>) -> Language {
    match locale {
        Some(l) if l.to_ascii_lowercase().starts_with("sr") => Language::SrLatn,
        _ => Language::En,
    }
}

impl Settings {
    /// Defaults for a fresh install, using the OS locale.
    pub fn initial() -> Self {
        Settings {
            language: language_for_locale(sys_locale::get_locale().as_deref()),
            ..Settings::default()
        }
    }

    /// Load settings from `path`; missing or broken file -> initial defaults.
    pub fn load(path: Option<&Path>) -> Self {
        let Some(path) = path else {
            return Settings::initial();
        };
        match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(s) => s,
                Err(e) => {
                    log::warn!("invalid settings file {}: {e}", path.display());
                    Settings::initial()
                }
            },
            Err(_) => Settings::initial(),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    }
}

/// Settings file path in the app config dir.
pub fn settings_path(config_dir: Option<PathBuf>) -> Option<PathBuf> {
    config_dir.map(|d| d.join(FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let s = Settings::default();
        assert_eq!(s.theme, Theme::System);
        assert_eq!(s.mode, Mode::Reader);
        assert_eq!(s.auto_save, AutoSave::Off);
        assert!(s.check_updates);
        assert!(!s.run_in_background);
    }

    #[test]
    fn locale_language() {
        assert_eq!(language_for_locale(Some("sr-RS")), Language::SrLatn);
        assert_eq!(language_for_locale(Some("sr_RS@latin")), Language::SrLatn);
        assert_eq!(language_for_locale(Some("SR")), Language::SrLatn);
        assert_eq!(language_for_locale(Some("en-US")), Language::En);
        assert_eq!(language_for_locale(Some("de-DE")), Language::En);
        assert_eq!(language_for_locale(None), Language::En);
    }

    #[test]
    fn serializes_camel_case() {
        let mut s = Settings {
            language: Language::SrCyrl,
            mode: Mode::XmlSign,
            auto_save: AutoSave::SaveAndOpen,
            ..Default::default()
        };
        s.pkcs11_paths.mup = "/x.so".into();
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["language"], "sr-Cyrl");
        assert_eq!(v["theme"], "system");
        assert_eq!(v["mode"], "xmlSign");
        assert_eq!(v["autoSave"], "saveAndOpen");
        assert_eq!(v["pdfCyrillicLabels"], false);
        assert_eq!(v["autoSaveDir"], "");
        assert_eq!(v["runInBackground"], false);
        assert_eq!(v["checkUpdates"], true);
        assert_eq!(v["lastDir"], "");
        assert_eq!(v["pkcs11Paths"]["mup"], "/x.so");
        assert_eq!(v["pkcs11Paths"]["esmart"], "");
        let back: Settings = serde_json::from_value(v).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn partial_json_uses_defaults() {
        let s: Settings =
            serde_json::from_str(r#"{"theme":"dark","pkcs11Paths":{"halcom":"h"}}"#).unwrap();
        assert_eq!(s.theme, Theme::Dark);
        assert_eq!(s.mode, Mode::Reader);
        assert!(s.check_updates);
        assert_eq!(s.pkcs11_paths.halcom, "h");
        assert_eq!(s.pkcs11_paths.mup, "");
        assert_eq!(s.fines, FinesSettings::default());
        assert!(!s.history.consent_given);
    }

    #[test]
    fn fines_settings() {
        let s = Settings::default();
        assert!(!s.fines.auto_check);
        assert!(!s.fines.consent_given);
        assert!(s.fines.provider_enabled("parkingns"));
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["fines"]["autoCheck"], false);
        assert_eq!(v["fines"]["consentGiven"], false);
        assert!(v["fines"].get("enabledProviders").is_none());
        assert_eq!(v["fines"]["remoteDefinitions"], true);
        let s: Settings =
            serde_json::from_str(r#"{"fines":{"autoCheck":true,"enabledProviders":["a"]}}"#)
                .unwrap();
        assert!(s.fines.auto_check);
        assert!(!s.fines.consent_given);
        assert!(s.fines.remote_definitions, "missing = on");
        assert!(s.fines.provider_enabled("a"));
        assert!(!s.fines.provider_enabled("parkingns"));
    }

    #[test]
    fn invalid_values_rejected() {
        assert!(serde_json::from_str::<Settings>(r#"{"theme":"blue"}"#).is_err());
    }

    #[test]
    fn module_paths_use_defaults_for_empty() {
        let p = Pkcs11Paths {
            pks: " /custom.so ".into(),
            ..Default::default()
        };
        let paths = p.module_paths(|v| {
            if v == CardVendor::Halcom {
                String::new()
            } else {
                format!("default-{v:?}")
            }
        });
        assert_eq!(paths.len(), 4);
        assert_eq!(paths[0].path, "default-Mup");
        assert_eq!(paths[1].path, "/custom.so");
        assert!(paths.iter().all(|m| m.vendor != CardVendor::Halcom));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("chitach-settings-{}", std::process::id()));
        let path = dir.join(FILE_NAME);
        let s = Settings {
            last_dir: "/home/x".into(),
            ..Default::default()
        };
        s.save(&path).unwrap();
        assert_eq!(Settings::load(Some(&path)), s);
        std::fs::write(&path, "garbage").unwrap();
        assert_eq!(Settings::load(Some(&path)).last_dir, "");
        let _ = std::fs::remove_dir_all(dir);
    }
}
