//! Chitach Pro licence: a one-time Lemon Squeezy license key that unlocks the fines check
//! ("Provera kazni") and the vehicle history ("Istorija vozila").
//!
//! Chitach is open source, so this is a gate for honest customers, not copy protection:
//! anyone can build a copy without it. The key is checked against the public Lemon Squeezy
//! License API (no API secret is needed or shipped); the app sends only the key and an
//! instance name. After activation the app works offline; the key is re-validated in the
//! background every [`REVALIDATE_AFTER`] and Pro stays on for [`OFFLINE_GRACE`] after the
//! last successful validation.
//!
//! The key is stored in `license.json` next to the settings (owner-only permissions on
//! Unix) and is never sent to the UI.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{with_detail, CmdResult};
use crate::fines::{encode_component, USER_AGENT};

/// Lemon Squeezy store that sells Chitach Pro (0 = not configured yet: keys are rejected).
pub const STORE_ID: u64 = 0;
/// Products whose keys unlock Pro (empty = any product of [`STORE_ID`]).
pub const PRODUCT_IDS: &[u64] = &[];
/// Checkout page opened by the "Buy" button (empty = no button).
pub const BUY_URL: &str = "";

const API: &str = "https://api.lemonsqueezy.com/v1/licenses";
const TIMEOUT: Duration = Duration::from_secs(15);
/// Licence file in the app config dir.
pub const LICENSE_FILE: &str = "license.json";
/// Background re-validation interval (ms).
pub const REVALIDATE_AFTER: i64 = 7 * 24 * 3600 * 1000;
/// Pro stays on this long after the last successful validation when offline (ms).
pub const OFFLINE_GRACE: i64 = 30 * 24 * 3600 * 1000;
/// Key accepted without network in mock mode (`CHITACH_MOCK=1`).
pub const MOCK_KEY: &str = "CHITACH-PRO-DEMO";
/// Error returned by gated commands.
pub const PRO_REQUIRED: &str = "error.proRequired";

/// Activated licence as stored in `license.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stored {
    pub key: String,
    pub instance_id: String,
    /// Unix ms.
    pub activated_at: i64,
    /// Last successful validation, Unix ms.
    pub validated_at: i64,
    #[serde(default)]
    pub email: Option<String>,
}

impl Stored {
    pub fn active(&self, now: i64) -> bool {
        now - self.validated_at <= OFFLINE_GRACE
    }
}

/// `LicenseInfo` of the API contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInfo {
    pub active: bool,
    /// A key is stored (it may be inactive when it could not be validated for too long).
    pub stored: bool,
    /// Last 4 characters of the stored key.
    pub key_hint: Option<String>,
    pub email: Option<String>,
    pub validated_at: Option<i64>,
    pub buy_url: Option<String>,
}

// ---------- Lemon Squeezy responses ----------

#[derive(Debug, Default, Deserialize)]
struct LsResponse {
    #[serde(default)]
    activated: Option<bool>,
    #[serde(default)]
    valid: Option<bool>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    instance: Option<LsInstance>,
    #[serde(default)]
    meta: Option<LsMeta>,
}

#[derive(Debug, Deserialize)]
struct LsInstance {
    id: String,
}

#[derive(Debug, Deserialize)]
struct LsMeta {
    store_id: u64,
    product_id: u64,
    #[serde(default)]
    customer_email: Option<String>,
}

/// One answer of the License API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    /// HTTP status and body (also for 4xx: Lemon Squeezy explains errors in JSON).
    Body(u16, String),
    Network(String),
}

fn product_ok(meta: &LsMeta) -> bool {
    STORE_ID != 0
        && meta.store_id == STORE_ID
        && (PRODUCT_IDS.is_empty() || PRODUCT_IDS.contains(&meta.product_id))
}

/// Result of an activation: the licence to store, or an `error.*` string.
pub fn interpret_activate(key: &str, fetched: Fetched, now: i64) -> CmdResult<Stored> {
    let (status, body) = match fetched {
        Fetched::Body(s, b) => (s, b),
        Fetched::Network(e) => return Err(with_detail("error.licenseNetwork", e)),
    };
    if status >= 500 {
        return Err(with_detail(
            "error.licenseNetwork",
            format!("HTTP {status}"),
        ));
    }
    let rsp: LsResponse = serde_json::from_str(&body)
        .map_err(|e| with_detail("error.licenseNetwork", format!("HTTP {status}: {e}")))?;
    if rsp.activated != Some(true) {
        let msg = rsp.error.unwrap_or_else(|| format!("HTTP {status}"));
        if msg.to_lowercase().contains("limit") {
            return Err(with_detail("error.licenseLimit", msg));
        }
        return Err(with_detail("error.licenseInvalid", msg));
    }
    let meta = rsp
        .meta
        .ok_or_else(|| with_detail("error.licenseInvalid", "no meta"))?;
    if !product_ok(&meta) {
        return Err(with_detail(
            "error.licenseWrongProduct",
            format!("store {} product {}", meta.store_id, meta.product_id),
        ));
    }
    let instance = rsp
        .instance
        .ok_or_else(|| with_detail("error.licenseInvalid", "no instance"))?;
    Ok(Stored {
        key: key.to_string(),
        instance_id: instance.id,
        activated_at: now,
        validated_at: now,
        email: meta.customer_email,
    })
}

/// Outcome of a background validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    Valid,
    /// The key was refunded, disabled, deactivated or belongs to another product.
    Invalid(String),
    /// Could not ask (offline, server error); keep the licence within the grace period.
    Unknown(String),
}

pub fn interpret_validate(fetched: Fetched) -> Validation {
    let (status, body) = match fetched {
        Fetched::Body(s, b) => (s, b),
        Fetched::Network(e) => return Validation::Unknown(e),
    };
    if status >= 500 || status == 429 {
        return Validation::Unknown(format!("HTTP {status}"));
    }
    let Ok(rsp) = serde_json::from_str::<LsResponse>(&body) else {
        return Validation::Unknown(format!("HTTP {status}: invalid JSON"));
    };
    match (rsp.valid, rsp.meta) {
        (Some(true), Some(meta)) if product_ok(&meta) => Validation::Valid,
        (Some(true), _) => Validation::Invalid("wrong product".into()),
        (Some(false), _) => Validation::Invalid(rsp.error.unwrap_or_else(|| "invalid".into())),
        (None, _) => Validation::Unknown(format!("HTTP {status}")),
    }
}

/// POST a form to the License API (blocking).
fn post(endpoint: &str, form: &[(&str, &str)]) -> Fetched {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();
    let body = form
        .iter()
        .map(|(k, v)| format!("{}={}", encode_component(k), encode_component(v)))
        .collect::<Vec<_>>()
        .join("&");
    match agent
        .post(format!("{API}/{endpoint}"))
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .content_type("application/x-www-form-urlencoded")
        .send(body)
    {
        Ok(mut rsp) => {
            let status = rsp.status().as_u16();
            match rsp.body_mut().read_to_string() {
                Ok(b) => Fetched::Body(status, b),
                Err(e) => Fetched::Network(e.to_string()),
            }
        }
        Err(e) => Fetched::Network(e.to_string()),
    }
}

fn instance_name() -> String {
    format!(
        "Chitach ({} {})",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Masked form of a key for the UI.
pub fn key_hint(key: &str) -> String {
    let tail: String = key
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("…{tail}")
}

// ---------- storage ----------

pub fn load(path: Option<&Path>) -> Option<Stored> {
    let bytes = std::fs::read(path?).ok()?;
    serde_json::from_slice(&bytes)
        .map_err(|e| log::warn!("invalid {LICENSE_FILE}: {e}"))
        .ok()
}

/// Write with owner-only permissions (Unix); `None` removes the file.
pub fn save(path: &Path, stored: Option<&Stored>) -> std::io::Result<()> {
    let Some(stored) = stored else {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(stored).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    {
        use std::io::Write;
        let mut f = opts.open(&tmp)?;
        f.write_all(&json)?;
    }
    std::fs::rename(&tmp, path)
}

/// `license.json` path next to `settings.json`.
pub fn license_path(settings_path: Option<&Path>) -> Option<PathBuf> {
    settings_path
        .and_then(|p| p.parent())
        .map(|d| d.join(LICENSE_FILE))
}

// ---------- state ----------

pub struct LicenseState {
    path: Option<PathBuf>,
    mock: bool,
    stored: Mutex<Option<Stored>>,
}

impl LicenseState {
    pub fn new(path: Option<PathBuf>, mock: bool) -> Self {
        let stored = load(path.as_deref());
        LicenseState {
            path,
            mock,
            stored: Mutex::new(stored),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Stored>> {
        self.stored.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set(&self, stored: Option<Stored>) -> CmdResult<()> {
        if let Some(p) = &self.path {
            save(p, stored.as_ref()).map_err(|e| with_detail("error.licenseSave", e))?;
        }
        *self.lock() = stored;
        Ok(())
    }

    pub fn info(&self) -> LicenseInfo {
        let stored = self.lock().clone();
        LicenseInfo {
            active: stored.as_ref().is_some_and(|s| s.active(now_ms())),
            stored: stored.is_some(),
            key_hint: stored.as_ref().map(|s| key_hint(&s.key)),
            email: stored.as_ref().and_then(|s| s.email.clone()),
            validated_at: stored.as_ref().map(|s| s.validated_at),
            buy_url: (!BUY_URL.is_empty()).then(|| BUY_URL.to_string()),
        }
    }

    /// Activate a key on this computer (blocking).
    pub fn activate(&self, key: &str) -> CmdResult<LicenseInfo> {
        let key = key.trim();
        if key.is_empty() {
            return Err("error.licenseNoKey".into());
        }
        let now = now_ms();
        let stored = if self.mock {
            std::thread::sleep(Duration::from_millis(600));
            if key != MOCK_KEY {
                return Err(with_detail(
                    "error.licenseInvalid",
                    "license_key not found.",
                ));
            }
            Stored {
                key: key.to_string(),
                instance_id: "mock".into(),
                activated_at: now,
                validated_at: now,
                email: Some("kupac@example.com".into()),
            }
        } else {
            let name = instance_name();
            let fetched = post(
                "activate",
                &[("license_key", key), ("instance_name", &name)],
            );
            match interpret_activate(key, fetched.clone(), now) {
                Err(e) if e.starts_with("error.licenseWrongProduct") => {
                    // Free the activation slot taken on the other product.
                    if let Fetched::Body(_, body) = &fetched {
                        if let Ok(LsResponse {
                            instance: Some(i), ..
                        }) = serde_json::from_str::<LsResponse>(body)
                        {
                            let _ = post(
                                "deactivate",
                                &[("license_key", key), ("instance_id", &i.id)],
                            );
                        }
                    }
                    return Err(e);
                }
                r => r?,
            }
        };
        self.set(Some(stored))?;
        Ok(self.info())
    }

    /// Release this computer's activation and forget the key (blocking). The local key is
    /// removed even when Lemon Squeezy cannot be reached.
    pub fn deactivate(&self) -> CmdResult<LicenseInfo> {
        let stored = self.lock().clone();
        if let (Some(s), false) = (&stored, self.mock) {
            if let Fetched::Network(e) = post(
                "deactivate",
                &[("license_key", &s.key), ("instance_id", &s.instance_id)],
            ) {
                log::warn!("licence deactivation: {e}");
            }
        }
        self.set(None)?;
        Ok(self.info())
    }

    /// Re-validate the stored key when it is due (or `force`); blocking.
    pub fn revalidate(&self, force: bool) {
        let Some(s) = self.lock().clone() else {
            return;
        };
        let now = now_ms();
        if self.mock || (!force && now - s.validated_at < REVALIDATE_AFTER) {
            return;
        }
        let fetched = post(
            "validate",
            &[("license_key", &s.key), ("instance_id", &s.instance_id)],
        );
        let result = match interpret_validate(fetched) {
            Validation::Valid => self.set(Some(Stored {
                validated_at: now,
                ..s
            })),
            Validation::Invalid(why) => {
                log::warn!("licence is no longer valid: {why}");
                self.set(None)
            }
            Validation::Unknown(why) => {
                log::info!("licence validation skipped: {why}");
                Ok(())
            }
        };
        if let Err(e) = result {
            log::warn!("licence: {e}");
        }
    }

    /// Gate for the Pro commands (blocking: may re-validate an expired grace period).
    pub fn require(&self) -> CmdResult<()> {
        let stored = self.lock().clone();
        match stored {
            None => Err(PRO_REQUIRED.into()),
            Some(s) if s.active(now_ms()) => Ok(()),
            Some(_) => {
                self.revalidate(true);
                if self.info().active {
                    Ok(())
                } else {
                    Err(PRO_REQUIRED.into())
                }
            }
        }
    }
}

/// Re-validate in the background at startup.
pub fn spawn_revalidate(app: tauri::AppHandle) {
    use tauri::Manager;
    std::thread::spawn(move || {
        if let Some(state) = app.try_state::<LicenseState>() {
            state.revalidate(false);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(s: u16, b: &str) -> Fetched {
        Fetched::Body(s, b.to_string())
    }

    #[test]
    fn activation_errors() {
        let e = interpret_activate(
            "K",
            body(
                404,
                r#"{"activated":false,"error":"license_key not found."}"#,
            ),
            0,
        )
        .unwrap_err();
        assert!(e.starts_with("error.licenseInvalid|"), "{e}");
        let e = interpret_activate(
            "K",
            body(
                400,
                r#"{"activated":false,"error":"This license key has reached the activation limit."}"#,
            ),
            0,
        )
        .unwrap_err();
        assert!(e.starts_with("error.licenseLimit|"), "{e}");
        let e = interpret_activate("K", Fetched::Network("dns".into()), 0).unwrap_err();
        assert_eq!(e, "error.licenseNetwork|dns");
        let e = interpret_activate("K", body(503, "oops"), 0).unwrap_err();
        assert!(e.starts_with("error.licenseNetwork|"), "{e}");
    }

    #[test]
    fn activation_checks_the_store() {
        // STORE_ID is not configured in the repository, so every real key is refused.
        let ok = r#"{"activated":true,"error":null,
            "license_key":{"id":1,"status":"active","key":"K"},
            "instance":{"id":"inst-1","name":"Chitach"},
            "meta":{"store_id":999,"product_id":5,"customer_email":"a@b.c"}}"#;
        let e = interpret_activate("K", body(200, ok), 0).unwrap_err();
        assert!(e.starts_with("error.licenseWrongProduct|"), "{e}");
    }

    #[test]
    fn validation() {
        assert_eq!(
            interpret_validate(body(
                404,
                r#"{"valid":false,"error":"license_key not found."}"#
            )),
            Validation::Invalid("license_key not found.".into())
        );
        assert!(matches!(
            interpret_validate(Fetched::Network("offline".into())),
            Validation::Unknown(_)
        ));
        assert!(matches!(
            interpret_validate(body(500, "")),
            Validation::Unknown(_)
        ));
        assert!(matches!(
            interpret_validate(body(
                200,
                r#"{"valid":true,"meta":{"store_id":999,"product_id":1}}"#
            )),
            Validation::Invalid(_)
        ));
    }

    #[test]
    fn grace_period() {
        let s = Stored {
            key: "ABCD-1234".into(),
            instance_id: "i".into(),
            activated_at: 0,
            validated_at: 1_000,
            email: None,
        };
        assert!(s.active(1_000 + OFFLINE_GRACE));
        assert!(!s.active(1_001 + OFFLINE_GRACE));
        assert_eq!(key_hint(&s.key), "…1234");
    }

    #[test]
    fn storage_roundtrip() {
        let dir = std::env::temp_dir().join(format!("chitach-license-{}", std::process::id()));
        let path = dir.join(LICENSE_FILE);
        let s = Stored {
            key: "K".into(),
            instance_id: "i".into(),
            activated_at: 1,
            validated_at: 2,
            email: Some("a@b.c".into()),
        };
        save(&path, Some(&s)).unwrap();
        assert_eq!(load(Some(&path)), Some(s));
        save(&path, None).unwrap();
        assert_eq!(load(Some(&path)), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mock_activation() {
        let st = LicenseState::new(None, true);
        assert_eq!(st.require(), Err(PRO_REQUIRED.to_string()));
        assert!(st.activate("wrong").is_err());
        let info = st.activate(MOCK_KEY).unwrap();
        assert!(info.active);
        assert!(st.require().is_ok());
        assert!(!st.deactivate().unwrap().stored);
    }
}
