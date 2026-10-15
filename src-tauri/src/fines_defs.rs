//! Up-to-date fines provider definitions without a new release (docs/07, §8).
//!
//! The binary embeds `providers/fines.json`. Once a day the app downloads the copy on
//! `main` together with its detached Ed25519 signature (`fines.json.sig`, base64) and uses
//! it only if
//! - the signature verifies with the public key embedded in the binary
//!   (`providers/fines.pub`; while it is empty, downloading is off),
//! - its `updated` date is newer than the definitions in use (no rollback to older files),
//! - it parses; and every provider whose page or check host is not on the compiled-in
//!   [`ALLOWED_DOMAINS`] list is dropped, so even a signed file cannot send plates elsewhere.
//!
//! A verified download is cached in the app cache dir and re-verified on every start.
//! Only GitHub is contacted; no personal data is sent.

use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, SystemTime};

use base64::Engine;
use serde::Serialize;

use crate::fines::{self, url_host, Definitions, Provider};

pub const REMOTE_URL: &str =
    "https://raw.githubusercontent.com/vahidzekic/Chitach/main/src-tauri/providers/fines.json";
pub const REMOTE_SIG_URL: &str =
    "https://raw.githubusercontent.com/vahidzekic/Chitach/main/src-tauri/providers/fines.json.sig";
/// Base64 Ed25519 public key (raw 32 bytes); `#` lines are comments. Empty = no downloads.
pub const PUBLIC_KEY: &str = include_str!("../providers/fines.pub");

/// How often the remote definitions are fetched.
pub const REFRESH_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// Size limit of a downloaded definitions file.
pub const MAX_SIZE: u64 = 256 * 1024;
const TIMEOUT: Duration = Duration::from_secs(15);

const CACHE_JSON: &str = "fines.json";
const CACHE_SIG: &str = "fines.json.sig";
const CACHE_STAMP: &str = "fines.checked";

/// Domains (and their subdomains) a provider may point to. Compiled in on purpose: a
/// definitions file can only choose among these.
pub const ALLOWED_DOMAINS: &[&str] = &[
    "parking-servis.co.rs",
    "parkingns.rs",
    "parkingservisnp.rs",
    "nisparking.rs",
    "eparking.rs",
    "jkpsumadija.rs",
    "park4all.rs",
    "kjpzlatibor.rs",
    "mojekazne.com",
    "parkingcacak.co.rs",
    "sud.rs",
    "euprava.gov.rs",
    "amss.org.rs",
];

/// Is `host` one of [`ALLOWED_DOMAINS`] or a subdomain of one?
pub fn host_allowed(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    // A port or credentials in the authority are never needed.
    if host.contains([':', '@']) {
        return false;
    }
    ALLOWED_DOMAINS
        .iter()
        .any(|d| host == *d || host.strip_suffix(d).is_some_and(|rest| rest.ends_with('.')))
}

/// Do the provider's page and check point to allowed hosts only?
pub fn provider_allowed(p: &Provider) -> bool {
    let page = url_host(&p.expand(&p.url)).is_some_and(host_allowed);
    let check = p
        .check
        .as_ref()
        .is_none_or(|c| url_host(&p.expand(&c.url)).is_some_and(host_allowed));
    page && check
}

fn allowed_only(mut d: Definitions) -> Definitions {
    d.providers.retain(|p| {
        let ok = provider_allowed(p);
        if !ok {
            log::warn!("fines: provider {:?} points outside the allowlist", p.id);
        }
        ok
    });
    d
}

/// Public key from `fines.pub` text.
pub fn parse_public_key(text: &str) -> Option<[u8; 32]> {
    let b64: String = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    bytes.try_into().ok()
}

/// Verify a detached base64 Ed25519 signature of `data`.
pub fn verify(data: &[u8], signature_b64: &str, public_key: &[u8; 32]) -> bool {
    let Ok(sig) = base64::engine::general_purpose::STANDARD.decode(signature_b64.trim()) else {
        return false;
    };
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key)
        .verify(data, &sig)
        .is_ok()
}

/// Where the definitions in use come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// Built into this version.
    Embedded,
    /// Downloaded, signature verified.
    Remote,
}

/// `FinesDefinitions` of the API contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionsInfo {
    pub source: Source,
    pub updated: String,
}

/// Parse a verified file (or the embedded one) and apply the allowlist.
pub fn load(json: &str) -> Option<Definitions> {
    match fines::parse_definitions(json) {
        Ok(d) if !d.providers.is_empty() => Some(allowed_only(d)),
        Ok(_) => None,
        Err(e) => {
            log::warn!("fines definitions: {e}");
            None
        }
    }
}

/// Accept `candidate` (signed bytes) over the definitions currently in use?
pub fn accept(
    current_updated: &str,
    json: &[u8],
    signature_b64: &str,
    public_key: &[u8; 32],
) -> Option<Definitions> {
    if !verify(json, signature_b64, public_key) {
        log::warn!("fines definitions: bad signature");
        return None;
    }
    let d = load(std::str::from_utf8(json).ok()?)?;
    // ISO dates compare as strings. Equal = nothing new.
    (d.updated.as_str() > current_updated).then_some(d)
}

struct Current {
    defs: Definitions,
    source: Source,
}

/// Definitions in use (Tauri managed state).
pub struct FinesDefinitions {
    current: RwLock<Current>,
    cache_dir: Option<PathBuf>,
    public_key: Option<[u8; 32]>,
}

impl FinesDefinitions {
    /// Embedded definitions, replaced by a newer verified cached download if there is one.
    pub fn new(cache_dir: Option<PathBuf>) -> Self {
        Self::with_key(cache_dir, parse_public_key(PUBLIC_KEY))
    }

    pub fn with_key(cache_dir: Option<PathBuf>, public_key: Option<[u8; 32]>) -> Self {
        let embedded = load(fines::PROVIDERS_JSON).unwrap_or_else(|| Definitions {
            schema: 0,
            updated: String::new(),
            providers: Vec::new(),
        });
        let me = FinesDefinitions {
            current: RwLock::new(Current {
                defs: embedded,
                source: Source::Embedded,
            }),
            cache_dir,
            public_key,
        };
        me.load_cached();
        me
    }

    pub fn providers(&self) -> Vec<Provider> {
        self.read().defs.providers.clone()
    }

    pub fn info(&self) -> DefinitionsInfo {
        let c = self.read();
        DefinitionsInfo {
            source: c.source,
            updated: c.defs.updated.clone(),
        }
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Current> {
        self.current.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Use `json` if it is signed and newer. Returns whether it was taken.
    fn offer(&self, json: &[u8], sig: &str) -> bool {
        let Some(key) = &self.public_key else {
            return false;
        };
        let mut c = self.current.write().unwrap_or_else(|e| e.into_inner());
        match accept(&c.defs.updated, json, sig, key) {
            Some(defs) => {
                log::info!("fines definitions: using {} (signed)", defs.updated);
                *c = Current {
                    defs,
                    source: Source::Remote,
                };
                true
            }
            None => false,
        }
    }

    fn load_cached(&self) {
        let Some(dir) = &self.cache_dir else { return };
        if let (Ok(json), Ok(sig)) = (
            std::fs::read(dir.join(CACHE_JSON)),
            std::fs::read_to_string(dir.join(CACHE_SIG)),
        ) {
            self.offer(&json, &sig);
        }
    }

    /// Is a download due (never checked, or longer ago than [`REFRESH_INTERVAL`])?
    pub fn refresh_due(&self, now: SystemTime) -> bool {
        if self.public_key.is_none() {
            return false;
        }
        let Some(dir) = &self.cache_dir else {
            return true;
        };
        let last = std::fs::metadata(dir.join(CACHE_STAMP)).and_then(|m| m.modified());
        last.map_or(true, |t| {
            now.duration_since(t)
                .map_or(true, |age| age >= REFRESH_INTERVAL)
        })
    }

    /// Offer downloaded bytes; a taken file is written to the cache. Always records the
    /// attempt so the next one waits for [`REFRESH_INTERVAL`].
    pub fn apply_download(&self, json: &[u8], sig: &str) -> bool {
        let taken = self.offer(json, sig);
        if let Some(dir) = &self.cache_dir {
            if let Err(e) = write_cache(dir, taken.then_some((json, sig))) {
                log::warn!("fines definitions cache: {e}");
            }
        }
        taken
    }

    /// Download and apply if due (blocking). Network errors are only logged.
    pub fn refresh(&self) {
        if !self.refresh_due(SystemTime::now()) {
            return;
        }
        match (download(REMOTE_URL), download(REMOTE_SIG_URL)) {
            (Ok(json), Ok(sig)) => {
                self.apply_download(&json, &String::from_utf8_lossy(&sig));
            }
            (Err(e), _) | (_, Err(e)) => {
                log::info!("fines definitions download: {e}");
                // Retry at the next interval, not on every start.
                if let Some(dir) = &self.cache_dir {
                    let _ = write_cache(dir, None);
                }
            }
        }
    }
}

/// Background refresh: at start and then hourly (a download happens at most daily).
pub fn spawn_refresh(app: tauri::AppHandle) {
    use tauri::Manager;
    std::thread::spawn(move || loop {
        let enabled = app
            .try_state::<crate::state::Shared>()
            .is_some_and(|s| s.settings().fines.remote_definitions);
        if enabled {
            if let Some(defs) = app.try_state::<FinesDefinitions>() {
                defs.refresh();
            }
        }
        std::thread::sleep(Duration::from_secs(60 * 60));
    });
}

fn write_cache(dir: &Path, file: Option<(&[u8], &str)>) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    if let Some((json, sig)) = file {
        // Signature last: a half-written pair just fails verification.
        std::fs::write(dir.join(CACHE_JSON), json)?;
        std::fs::write(dir.join(CACHE_SIG), sig)?;
    }
    std::fs::write(dir.join(CACHE_STAMP), b"")
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .into();
    let mut rsp = agent
        .get(url)
        .header("User-Agent", fines::USER_AGENT)
        .call()
        .map_err(|e| e.to_string())?;
    rsp.body_mut()
        .with_config()
        .limit(MAX_SIZE)
        .read_to_vec()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    fn keypair() -> (Ed25519KeyPair, [u8; 32]) {
        // Fixed test seed; never used for real definitions.
        let kp = Ed25519KeyPair::from_seed_unchecked(&[7u8; 32]).unwrap();
        let pk: [u8; 32] = kp.public_key().as_ref().try_into().unwrap();
        (kp, pk)
    }

    fn sign(kp: &Ed25519KeyPair, data: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(kp.sign(data).as_ref())
    }

    fn defs_json(updated: &str, url: &str) -> String {
        format!(
            r#"{{"schema":1,"updated":"{updated}","providers":[
              {{"id":"nis-parking","category":"parking","name":"Niš","cities":["NI"],"kind":"link","url":"{url}"}},
              {{"id":"rnk","category":"person","name":"RNK","kind":"link","url":"https://rnk.sipres.sud.rs/"}}
            ]}}"#
        )
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "chitach-fines-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn allowlist() {
        assert!(host_allowed("portal.parkingns.rs"));
        assert!(host_allowed("parkingns.rs"));
        assert!(host_allowed("WWW.Parking-Servis.co.rs"));
        assert!(host_allowed("rnk.sipres.sud.rs"));
        assert!(!host_allowed("evilparkingns.rs"));
        assert!(!host_allowed("parkingns.rs.evil.com"));
        assert!(!host_allowed("portal.parkingns.rs:8443"));
        assert!(!host_allowed("x@portal.parkingns.rs"));
        assert!(!host_allowed("example.com"));
    }

    #[test]
    fn embedded_definitions_are_all_allowed() {
        let all = fines::providers();
        let kept = load(fines::PROVIDERS_JSON).unwrap();
        assert_eq!(kept.providers.len(), all.len());
        assert!(!kept.updated.is_empty());
    }

    #[test]
    fn signature_verification() {
        let (kp, pk) = keypair();
        let data = defs_json("2099-01-01", "https://www.nisparking.rs/sr/");
        let sig = sign(&kp, data.as_bytes());
        assert!(verify(data.as_bytes(), &sig, &pk));
        assert!(verify(data.as_bytes(), &format!("{sig}\n"), &pk));
        let mut tampered = data.clone().into_bytes();
        tampered[10] ^= 1;
        assert!(!verify(&tampered, &sig, &pk));
        assert!(!verify(data.as_bytes(), "not base64!", &pk));
        let other = Ed25519KeyPair::from_seed_unchecked(&[8u8; 32]).unwrap();
        assert!(!verify(
            data.as_bytes(),
            &sign(&other, data.as_bytes()),
            &pk
        ));
    }

    #[test]
    fn public_key_file() {
        let (_, pk) = keypair();
        let b64 = base64::engine::general_purpose::STANDARD.encode(pk);
        assert_eq!(parse_public_key(&format!("# comment\n{b64}\n")), Some(pk));
        assert_eq!(parse_public_key("# no key yet\n"), None);
        assert_eq!(parse_public_key("AAAA"), None);
    }

    #[test]
    fn only_newer_signed_allowed_files_are_accepted() {
        let (kp, pk) = keypair();
        let newer = defs_json("2099-01-01", "https://www.nisparking.rs/sr/");
        let d = accept(
            "2026-10-15",
            newer.as_bytes(),
            &sign(&kp, newer.as_bytes()),
            &pk,
        )
        .unwrap();
        assert_eq!(d.updated, "2099-01-01");
        assert_eq!(d.providers.len(), 2);
        // Same date or older: rejected (no rollback).
        assert!(accept(
            "2099-01-01",
            newer.as_bytes(),
            &sign(&kp, newer.as_bytes()),
            &pk
        )
        .is_none());
        let older = defs_json("2020-01-01", "https://www.nisparking.rs/sr/");
        assert!(accept(
            "2026-10-15",
            older.as_bytes(),
            &sign(&kp, older.as_bytes()),
            &pk
        )
        .is_none());
        // Unsigned: rejected.
        assert!(accept("2026-10-15", newer.as_bytes(), "", &pk).is_none());
        // Signed, but a provider points outside the allowlist: that provider is dropped.
        let evil = defs_json("2099-01-01", "https://collector.example.com/?p={plate}");
        let d = accept(
            "2026-10-15",
            evil.as_bytes(),
            &sign(&kp, evil.as_bytes()),
            &pk,
        )
        .unwrap();
        assert_eq!(
            d.providers
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>(),
            ["rnk"]
        );
    }

    #[test]
    fn check_host_must_be_allowed_too() {
        let json = r#"{"schema":1,"updated":"2099-01-01","providers":[
          {"id":"ns","category":"parking","name":"NS","cities":["NS"],"kind":"json_get",
           "url":"https://portal.parkingns.rs/",
           "check":{"url":"https://evil.example/x?p={plate}","parser":"jsonArray"}}]}"#;
        assert!(load(json).unwrap().providers.is_empty());
    }

    #[test]
    fn download_is_cached_and_reloaded() {
        let dir = temp_dir("cache");
        let (kp, pk) = keypair();
        let defs = FinesDefinitions::with_key(Some(dir.clone()), Some(pk));
        assert_eq!(defs.info().source, Source::Embedded);
        assert!(defs.refresh_due(SystemTime::now()));

        // A bad download changes nothing but still records the attempt.
        assert!(!defs.apply_download(b"{}", "AAAA"));
        assert_eq!(defs.info().source, Source::Embedded);
        assert!(!defs.refresh_due(SystemTime::now()));
        assert!(defs.refresh_due(SystemTime::now() + REFRESH_INTERVAL));

        let json = defs_json("2099-01-01", "https://www.nisparking.rs/sr/");
        assert!(defs.apply_download(json.as_bytes(), &sign(&kp, json.as_bytes())));
        assert_eq!(
            defs.info(),
            DefinitionsInfo {
                source: Source::Remote,
                updated: "2099-01-01".into()
            }
        );
        assert_eq!(defs.providers().len(), 2);

        // Next start: the cached file is verified and used again.
        let again = FinesDefinitions::with_key(Some(dir.clone()), Some(pk));
        assert_eq!(again.info().source, Source::Remote);
        // ... but not with another key.
        let other_pk: [u8; 32] = Ed25519KeyPair::from_seed_unchecked(&[9u8; 32])
            .unwrap()
            .public_key()
            .as_ref()
            .try_into()
            .unwrap();
        let other = FinesDefinitions::with_key(Some(dir.clone()), Some(other_pk));
        assert_eq!(other.info().source, Source::Embedded);
        // No key: no downloads at all.
        let nokey = FinesDefinitions::with_key(Some(dir.clone()), None);
        assert!(!nokey.refresh_due(SystemTime::now() + REFRESH_INTERVAL));
        assert_eq!(nokey.info().source, Source::Embedded);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
