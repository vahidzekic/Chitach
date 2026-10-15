//! Vehicle history ("Istorija vozila"), see docs/08-istrazivanje-istorija-vozila.md.
//!
//! Three parts:
//! - a local VIN check (format, WMI → manufacturer/country, model-year code, North-American
//!   check digit) compared with the data on the card. Nothing is sent anywhere;
//! - declarative provider definitions (`providers/history.json`, embedded): `link` providers
//!   are opened in the system browser (VIN/plate in the URL where supported, otherwise copied
//!   to the clipboard by the UI);
//! - `api` providers: an opt-in report through a commercial API with the **user's own** key
//!   (BYOK). The key is never part of the binary or of `settings.json`; it is stored in
//!   `secrets.json` next to the settings (owner-only permissions on Unix).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use celik_core::{Document, VehicleDocument};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};

use crate::{
    error::CmdResult,
    fines::{self, Fetched},
    settings::HistorySettings,
};

/// Embedded provider definitions.
pub const PROVIDERS_JSON: &str = include_str!("../providers/history.json");
/// Embedded WMI table.
pub const WMI_JSON: &str = include_str!("../providers/wmi.json");
/// API secrets file in the app config dir.
pub const SECRETS_FILE: &str = "secrets.json";

/// How long an API result is reused (every API call may be charged).
pub const CACHE_TTL: Duration = Duration::from_secs(30 * 60);
/// Minimum age of a cached result before an explicit re-check hits the API again.
pub const MIN_REFRESH: Duration = Duration::from_secs(60);

// ---------- VIN ----------

/// ISO 3780 order of the second WMI character (and of model-year codes).
const ORDER: &str = "ABCDEFGHJKLMNPRSTUVWXYZ1234567890";

fn order_index(c: char) -> Option<usize> {
    ORDER.find(c)
}

/// Normalise a VIN: uppercase, no spaces, dashes or dots.
pub fn normalize_vin(vin: &str) -> String {
    vin.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VinFormat {
    /// 17 characters, no I/O/Q.
    Valid,
    /// Something else (older vehicles may have shorter numbers).
    Invalid,
    Empty,
}

pub fn vin_format(vin: &str) -> VinFormat {
    if vin.is_empty() {
        VinFormat::Empty
    } else if vin.len() == 17
        && vin
            .chars()
            .all(|c| c.is_ascii_digit() || (c.is_ascii_uppercase() && !"IOQ".contains(c)))
    {
        VinFormat::Valid
    } else {
        VinFormat::Invalid
    }
}

fn transliterate(c: char) -> Option<u32> {
    if let Some(d) = c.to_digit(10) {
        return Some(d);
    }
    Some(match c {
        'A' | 'J' => 1,
        'B' | 'K' | 'S' => 2,
        'C' | 'L' | 'T' => 3,
        'D' | 'M' | 'U' => 4,
        'E' | 'N' | 'V' => 5,
        'F' | 'W' => 6,
        'G' | 'P' | 'X' => 7,
        'H' | 'Y' => 8,
        'R' | 'Z' => 9,
        _ => return None,
    })
}

/// Check digit (VIN position 9) as computed from the other characters (North America).
pub fn check_digit(vin: &str) -> Option<char> {
    const WEIGHTS: [u32; 17] = [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];
    if vin.len() != 17 {
        return None;
    }
    let mut sum = 0;
    for (c, w) in vin.chars().zip(WEIGHTS) {
        sum += transliterate(c)? * w;
    }
    let r = sum % 11;
    Some(if r == 10 {
        'X'
    } else {
        char::from_digit(r, 10)?
    })
}

/// Model year for the VIN position-10 code, choosing the 30-year cycle closest to (but not
/// after) `current_year + 1`.
pub fn model_year(code: char, current_year: i32) -> Option<i32> {
    // Letters A..Y without I, O, Q, U, Z = 1980..2000, digits 1..9 = 2001..2009.
    const CODES: &str = "ABCDEFGHJKLMNPRSTVWXY123456789";
    let base = 1980 + CODES.find(code)? as i32;
    let mut year = base;
    while year + 30 <= current_year + 1 {
        year += 30;
    }
    Some(year)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Region {
    from: String,
    to: String,
    country: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manufacturer {
    pub wmi: String,
    pub make: String,
    /// Normalised make names on the card that match this WMI.
    #[serde(default)]
    pub names: Vec<String>,
    /// Country override (assembly plant outside the WMI region).
    #[serde(default)]
    pub country: Option<String>,
    /// The manufacturer encodes the model year in position 10.
    #[serde(default)]
    pub model_year: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WmiTable {
    regions: Vec<Region>,
    manufacturers: Vec<Manufacturer>,
}

impl WmiTable {
    pub fn embedded() -> Self {
        serde_json::from_str(WMI_JSON).unwrap_or_else(|e| {
            log::error!("wmi.json: {e}");
            WmiTable::default()
        })
    }

    /// Country (ISO 3166-1 alpha-2) of the WMI region.
    pub fn region(&self, vin: &str) -> Option<String> {
        let mut it = vin.chars();
        let (a, b) = (it.next()?, order_index(it.next()?)?);
        self.regions
            .iter()
            .find(|r| {
                let (mut f, mut t) = (r.from.chars(), r.to.chars());
                f.next() == Some(a)
                    && t.next() == Some(a)
                    && f.next()
                        .and_then(order_index)
                        .zip(t.next().and_then(order_index))
                        .is_some_and(|(lo, hi)| (lo..=hi).contains(&b))
            })
            .map(|r| r.country.clone())
    }

    pub fn manufacturer(&self, vin: &str) -> Option<&Manufacturer> {
        let wmi = vin.get(..3)?;
        self.manufacturers.iter().find(|m| m.wmi == wmi)
    }
}

/// `VinDecoded` of the API contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VinDecoded {
    pub wmi: String,
    pub vds: String,
    pub vis: String,
    pub make: Option<String>,
    /// ISO 3166-1 alpha-2.
    pub country: Option<String>,
    /// WMI region is North America (check digit and model year are mandatory there).
    pub north_america: bool,
    /// Model year from position 10, only when the manufacturer is known to encode it.
    pub model_year: Option<i32>,
    /// `Some(true/false)` for North-American VINs, `None` otherwise.
    pub check_digit_ok: Option<bool>,
}

pub fn decode(vin: &str, table: &WmiTable, current_year: i32) -> Option<VinDecoded> {
    if vin_format(vin) != VinFormat::Valid {
        return None;
    }
    let man = table.manufacturer(vin);
    let north_america = vin.starts_with(['1', '2', '3', '4', '5']);
    let year_char = vin.chars().nth(9)?;
    Some(VinDecoded {
        wmi: vin[..3].to_string(),
        vds: vin[3..9].to_string(),
        vis: vin[9..].to_string(),
        make: man.map(|m| m.make.clone()),
        country: man
            .and_then(|m| m.country.clone())
            .or_else(|| table.region(vin)),
        north_america,
        model_year: (north_america || man.is_some_and(|m| m.model_year))
            .then(|| model_year(year_char, current_year))
            .flatten(),
        check_digit_ok: north_america.then(|| check_digit(vin) == vin.chars().nth(8)),
    })
}

/// Make on the card, normalised for comparison (uppercase Latin without diacritics).
pub fn normalize_make(make: &str) -> String {
    fines::normalize_plate(make)
        .chars()
        .map(|c| match c {
            'Č' | 'Ć' => 'C',
            'Š' => 'S',
            'Ž' => 'Z',
            'Đ' => 'D',
            'Ë' | 'É' | 'È' => 'E',
            'Ö' => 'O',
            'Ü' => 'U',
            'Ä' => 'A',
            c => c,
        })
        .collect()
}

/// Year from a card date (`15.01.2020`, `2020`, …): the last plausible 4-digit number.
pub fn year_of(s: &str) -> Option<i32> {
    s.split(|c: char| !c.is_ascii_digit())
        .filter(|p| p.len() == 4)
        .filter_map(|p| p.parse::<i32>().ok())
        .rfind(|y| (1900..=2100).contains(y))
}

// ---------- checks ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Ok,
    Warning,
    Info,
    Unknown,
}

/// One consistency check. The UI translates `history.check.<id>.<variant>` with `args`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: &'static str,
    pub level: Level,
    pub variant: &'static str,
    pub args: Vec<String>,
}

fn check(id: &'static str, level: Level, variant: &'static str, args: Vec<String>) -> Check {
    Check {
        id,
        level,
        variant,
        args,
    }
}

/// Consistency checks between the VIN and the card. Never claims more than "consistent".
pub fn checks(
    v: &VehicleDocument,
    vin: &str,
    decoded: Option<&VinDecoded>,
    table: &WmiTable,
    current_year: i32,
) -> Vec<Check> {
    let mut out = Vec::new();
    match vin_format(vin) {
        VinFormat::Empty => {
            out.push(check("vin", Level::Unknown, "empty", vec![]));
            return out;
        }
        VinFormat::Invalid => out.push(check(
            "vin",
            Level::Warning,
            "invalid",
            vec![vin.len().to_string()],
        )),
        VinFormat::Valid => out.push(check("vin", Level::Ok, "valid", vec![])),
    }

    if let Some(d) = decoded {
        let card_make = normalize_make(&v.vehicle_make);
        match table.manufacturer(vin) {
            None => out.push(check(
                "make",
                Level::Unknown,
                "unknownWmi",
                vec![d.wmi.clone()],
            )),
            Some(_) if card_make.is_empty() => {}
            Some(m) => {
                let matches = m.names.iter().any(|n| {
                    !n.is_empty() && (card_make.contains(n.as_str()) || n.contains(&card_make))
                });
                out.push(if matches {
                    check("make", Level::Ok, "match", vec![m.make.clone()])
                } else {
                    check(
                        "make",
                        Level::Warning,
                        "mismatch",
                        vec![m.make.clone(), v.vehicle_make.clone()],
                    )
                });
            }
        }

        let produced = year_of(&v.year_of_production);
        match (d.model_year, produced) {
            (Some(my), Some(p)) if (my - p).abs() <= 1 => {
                out.push(check("modelYear", Level::Ok, "match", vec![my.to_string()]))
            }
            (Some(my), Some(p)) => out.push(check(
                "modelYear",
                Level::Warning,
                "mismatch",
                vec![my.to_string(), p.to_string()],
            )),
            (Some(my), None) => out.push(check(
                "modelYear",
                Level::Info,
                "only",
                vec![my.to_string()],
            )),
            (None, _) => out.push(check("modelYear", Level::Unknown, "notEncoded", vec![])),
        }

        match d.check_digit_ok {
            Some(true) => out.push(check("checkDigit", Level::Ok, "valid", vec![])),
            Some(false) => out.push(check("checkDigit", Level::Warning, "invalid", vec![])),
            None => {}
        }
    }

    let produced = year_of(&v.year_of_production);
    let first = year_of(&v.date_of_first_registration);
    match (produced, first) {
        (Some(p), Some(f)) if f < p => out.push(check(
            "firstRegistration",
            Level::Warning,
            "beforeProduction",
            vec![f.to_string(), p.to_string()],
        )),
        (Some(p), Some(f)) if f - p >= 2 => out.push(check(
            "firstRegistration",
            Level::Info,
            "late",
            vec![(f - p).to_string()],
        )),
        (Some(_), Some(_)) => out.push(check("firstRegistration", Level::Ok, "new", vec![])),
        _ => {}
    }
    if let Some(p) = produced {
        let age = current_year - p;
        if age >= 0 {
            out.push(check("age", Level::Info, "years", vec![age.to_string()]));
        }
    }
    out
}

// ---------- providers ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    /// Serbian sources (history in Serbia).
    Domestic,
    /// Commercial reports covering Europe.
    Foreign,
    /// Sources useful for vehicles imported from North America.
    Usa,
    /// Report through an API with the user's own key.
    Api,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Link,
    Api,
    #[serde(other)]
    Unknown,
}

/// What the provider is searched by; copied to the clipboard when not in the URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Key {
    #[default]
    Vin,
    Plate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Service {
    Vincario,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiDef {
    pub service: Service,
    /// https base URL of the API.
    pub base: String,
    /// Operations offered in the UI, in order.
    pub operations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub category: Category,
    pub name: String,
    pub kind: Kind,
    #[serde(default)]
    pub key: Key,
    /// Indicative price of one report (informative, as published).
    #[serde(default)]
    pub price: Option<String>,
    /// Page for the user; may contain `{vin}` / `{plate}`.
    pub url: String,
    /// Second page (e.g. the iOS app of a mobile-only service).
    #[serde(default)]
    pub alt_url: Option<String>,
    /// Affiliate/partner link: the UI must say so.
    #[serde(default)]
    pub partner: bool,
    #[serde(default)]
    pub api: Option<ApiDef>,
}

#[derive(Debug, Deserialize)]
struct ProvidersFile {
    #[serde(default)]
    schema: u32,
    providers: Vec<Provider>,
}

fn https_url(url: &str) -> bool {
    url.starts_with("https://") && fines::url_host(url).is_some()
}

impl Provider {
    fn is_valid(&self) -> bool {
        if self.id.is_empty()
            || !https_url(&self.url)
            || self.alt_url.as_deref().is_some_and(|u| !https_url(u))
        {
            return false;
        }
        match self.kind {
            Kind::Link => true,
            Kind::Api => self.api.as_ref().is_some_and(|a| {
                a.service != Service::Unknown && https_url(&a.base) && !a.operations.is_empty()
            }),
            Kind::Unknown => false,
        }
    }

    fn resolve(&self, template: &str, vin: &str, plate: &str) -> String {
        template
            .replace("{vin}", &fines::encode_component(vin))
            .replace("{plate}", &fines::encode_component(plate))
    }
}

pub fn parse_providers(json: &str) -> Result<Vec<Provider>, serde_json::Error> {
    let file: ProvidersFile = serde_json::from_str(json)?;
    if file.schema > 1 {
        log::warn!(
            "history.json schema {} is newer than supported",
            file.schema
        );
    }
    Ok(file
        .providers
        .into_iter()
        .filter(|p| {
            let ok = p.is_valid();
            if !ok {
                log::warn!("history: ignoring provider {:?}", p.id);
            }
            ok
        })
        .collect())
}

pub fn providers() -> Vec<Provider> {
    parse_providers(PROVIDERS_JSON).unwrap_or_else(|e| {
        log::error!("history.json: {e}");
        Vec::new()
    })
}

/// `HistoryProvider` of the API contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    pub id: String,
    pub name: String,
    pub category: Category,
    pub kind: Kind,
    pub key: Key,
    pub price: Option<String>,
    pub url: String,
    pub alt_url: Option<String>,
    /// The search key (VIN or plate) is part of `url`.
    pub key_in_url: bool,
    pub partner: bool,
    /// `api` only: operations, the host the VIN is sent to, and whether a key is stored.
    pub operations: Vec<String>,
    pub api_domain: Option<String>,
    pub configured: bool,
}

/// `HistoryInfo` of the API contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryInfo {
    pub vin: String,
    pub normalized_vin: String,
    pub vin_format: VinFormat,
    pub plate: String,
    pub normalized_plate: String,
    pub make: String,
    pub model: String,
    pub year_of_production: String,
    pub date_of_first_registration: String,
    pub decoded: Option<VinDecoded>,
    pub checks: Vec<Check>,
    pub providers: Vec<ProviderView>,
}

/// Everything the panel needs for a vehicle (`v = None`: no vehicle card, providers only).
pub fn info_for(
    v: Option<&VehicleDocument>,
    providers: &[Provider],
    table: &WmiTable,
    secrets: &Secrets,
    current_year: i32,
) -> HistoryInfo {
    let empty = VehicleDocument::default();
    let doc = v.unwrap_or(&empty);
    let vin = normalize_vin(&doc.vehicle_id_number);
    let plate = fines::normalize_plate(&doc.registration_number_of_vehicle);
    let decoded = decode(&vin, table, current_year);
    let checks = if v.is_some() {
        checks(doc, &vin, decoded.as_ref(), table, current_year)
    } else {
        Vec::new()
    };
    HistoryInfo {
        vin: doc.vehicle_id_number.clone(),
        normalized_vin: vin.clone(),
        vin_format: vin_format(&vin),
        plate: doc.registration_number_of_vehicle.clone(),
        normalized_plate: plate.clone(),
        make: doc.vehicle_make.clone(),
        model: doc.commercial_description.clone(),
        year_of_production: doc.year_of_production.clone(),
        date_of_first_registration: doc.date_of_first_registration.clone(),
        decoded,
        checks,
        providers: providers
            .iter()
            .map(|p| {
                let key_tag = match p.key {
                    Key::Vin => "{vin}",
                    Key::Plate => "{plate}",
                };
                ProviderView {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    category: p.category,
                    kind: p.kind,
                    key: p.key,
                    price: p.price.clone(),
                    url: p.resolve(&p.url, &vin, &plate),
                    alt_url: p.alt_url.as_deref().map(|u| p.resolve(u, &vin, &plate)),
                    key_in_url: p.url.contains(key_tag),
                    partner: p.partner,
                    operations: p
                        .api
                        .as_ref()
                        .map(|a| a.operations.clone())
                        .unwrap_or_default(),
                    api_domain: p
                        .api
                        .as_ref()
                        .and_then(|a| fines::url_host(&a.base).map(str::to_string)),
                    configured: p.kind == Kind::Api && secrets.get(&p.id).is_some(),
                }
            })
            .collect(),
    }
}

pub fn document_vehicle(doc: Option<&Document>) -> CmdResult<&VehicleDocument> {
    match doc {
        Some(Document::Vehicle(v)) => Ok(v),
        _ => Err("error.notVehicleCard".into()),
    }
}

// ---------- secrets (BYOK) ----------

/// API credentials of one provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApiKey {
    pub api_key: String,
    #[serde(default)]
    pub secret: String,
}

/// Stored API credentials (`secrets.json`), keyed by provider id. Never sent to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Secrets {
    #[serde(default)]
    pub keys: BTreeMap<String, ApiKey>,
}

impl Secrets {
    pub fn get(&self, id: &str) -> Option<&ApiKey> {
        self.keys.get(id).filter(|k| !k.api_key.is_empty())
    }

    pub fn load(path: Option<&Path>) -> Self {
        let Some(path) = path else {
            return Secrets::default();
        };
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                log::warn!("invalid {}: {e}", path.display());
                Secrets::default()
            }),
            Err(_) => Secrets::default(),
        }
    }

    /// Write with owner-only permissions (Unix); removes the file when empty.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if self.keys.is_empty() {
            return match std::fs::remove_file(path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
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
}

/// `secrets.json` path next to `settings.json`.
pub fn secrets_path(settings_path: Option<&Path>) -> Option<PathBuf> {
    settings_path
        .and_then(|p| p.parent())
        .map(|d| d.join(SECRETS_FILE))
}

// ---------- API report ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ApiStatus {
    Ok,
    /// Key rejected (HTTP 401/403 or an auth error from the API).
    Unauthorized,
    /// The API answered with an error (no credit, unknown VIN, …).
    Error,
    Unavailable,
    ParseError,
}

/// `HistoryApiResult` of the API contract. `fields` are the provider's own labels and
/// values, in order; Chitach does not interpret them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResult {
    pub provider_id: String,
    pub operation: String,
    pub status: ApiStatus,
    pub fields: Vec<(String, String)>,
    /// Price of this call as reported by the API.
    pub price: Option<String>,
    pub detail: Option<String>,
    pub checked_at: i64,
    pub cached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiParsed {
    pub status: ApiStatus,
    pub fields: Vec<(String, String)>,
    pub price: Option<String>,
    pub detail: Option<String>,
}

impl ApiParsed {
    fn new(status: ApiStatus, detail: Option<String>) -> Self {
        ApiParsed {
            status,
            fields: Vec::new(),
            price: None,
            detail,
        }
    }
}

/// Vincario control sum: first 10 hex chars of SHA-1 of `VIN|id|apiKey|secret`.
pub fn vincario_control_sum(vin: &str, id: &str, key: &ApiKey) -> String {
    let input = format!("{vin}|{id}|{}|{}", key.api_key, key.secret);
    let digest = Sha1::digest(input.as_bytes());
    digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()[..10]
        .to_string()
}

/// Request URL of an API operation. Contains the API key: never log it.
pub fn api_url(def: &ApiDef, op: &str, vin: &str, key: &ApiKey) -> Option<String> {
    if !def.operations.iter().any(|o| o == op) {
        return None;
    }
    match def.service {
        Service::Vincario => Some(format!(
            "{}/{}/{}/{}/{}.json",
            def.base.trim_end_matches('/'),
            fines::encode_component(&key.api_key),
            vincario_control_sum(vin, op, key),
            op,
            vin
        )),
        Service::Unknown => None,
    }
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => {
            let s = s.trim();
            (!s.is_empty()).then(|| s.to_string())
        }
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().filter_map(scalar).collect();
            (!parts.is_empty() && parts.len() == a.len()).then(|| parts.join(", "))
        }
        _ => None,
    }
}

/// Flatten a JSON value into `a.b.c` labels (arrays of objects as `a[0].b`, at most 10 items).
fn flatten_into(prefix: &str, v: &Value, out: &mut Vec<(String, String)>) {
    match v {
        Value::Object(o) => {
            for (k, v) in o {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_into(&p, v, out);
            }
        }
        Value::Array(a) if scalar(v).is_none() => {
            for (i, v) in a.iter().take(10).enumerate() {
                flatten_into(&format!("{prefix}[{i}]"), v, out);
            }
        }
        other => {
            if let Some(s) = scalar(other) {
                out.push((prefix.to_string(), s));
            }
        }
    }
}

/// Parse a Vincario response. `decode` is a `[{label, value}]` list; other operations are
/// flattened as they come. `price`/`price_currency`/`balance` are metadata.
pub fn parse_vincario(op: &str, body: &str) -> ApiParsed {
    let value: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return ApiParsed::new(ApiStatus::ParseError, Some(e.to_string())),
    };
    let Value::Object(obj) = &value else {
        return ApiParsed::new(ApiStatus::ParseError, Some("unexpected JSON shape".into()));
    };
    if obj
        .get("error")
        .is_some_and(|e| e.as_bool() == Some(true) || e.is_string())
    {
        let msg = obj
            .get("message")
            .and_then(scalar)
            .or_else(|| obj.get("error").and_then(Value::as_str).map(str::to_string));
        let auth = msg.as_deref().is_some_and(|m| {
            let m = m.to_lowercase();
            m.contains("key") || m.contains("auth") || m.contains("control") || m.contains("secret")
        });
        return ApiParsed::new(
            if auth {
                ApiStatus::Unauthorized
            } else {
                ApiStatus::Error
            },
            msg,
        );
    }
    let price = obj.get("price").and_then(scalar).map(|p| {
        match obj.get("price_currency").and_then(scalar) {
            Some(c) => format!("{p} {c}"),
            None => p,
        }
    });
    let mut fields = Vec::new();
    if op == "decode" {
        let Some(items) = obj.get("decode").and_then(Value::as_array) else {
            return ApiParsed::new(ApiStatus::ParseError, Some("missing \"decode\"".into()));
        };
        for item in items {
            let label = item.get("label").and_then(scalar);
            let value = item.get("value").and_then(scalar);
            if let (Some(l), Some(v)) = (label, value) {
                fields.push((l, v));
            }
        }
    } else {
        for (k, v) in obj {
            if !matches!(k.as_str(), "price" | "price_currency" | "balance") {
                flatten_into(k, v, &mut fields);
            }
        }
    }
    ApiParsed {
        status: ApiStatus::Ok,
        fields,
        price,
        detail: None,
    }
}

/// One GET of an API URL (blocking), with the HTTP client of the fines check.
pub fn fetch(url: &str) -> Fetched {
    fines::fetch(&fines::Request {
        method: fines::Method::Get,
        url: url.to_string(),
        form: Vec::new(),
        parser: fines::Parser::JsonArray,
        ..Default::default()
    })
}

pub fn interpret(def: &ApiDef, op: &str, fetched: Fetched) -> ApiParsed {
    match fetched {
        Fetched::Status(code @ (401 | 403)) => {
            ApiParsed::new(ApiStatus::Unauthorized, Some(format!("HTTP {code}")))
        }
        Fetched::Status(code @ (400 | 402 | 404 | 422 | 429)) => {
            ApiParsed::new(ApiStatus::Error, Some(format!("HTTP {code}")))
        }
        Fetched::Status(code) => {
            ApiParsed::new(ApiStatus::Unavailable, Some(format!("HTTP {code}")))
        }
        Fetched::Network(e) => ApiParsed::new(ApiStatus::Unavailable, Some(e)),
        Fetched::Body { body, .. } => match def.service {
            Service::Vincario => parse_vincario(op, &body),
            Service::Unknown => ApiParsed::new(ApiStatus::ParseError, None),
        },
    }
}

/// Fake answers for mock mode.
pub fn mock_parsed(op: &str, vin: &str) -> ApiParsed {
    let body = match op {
        "decode" => format!(
            r#"{{"price":0.25,"price_currency":"EUR","balance":{{"API Decode":19}},"decode":[
              {{"label":"VIN","value":"{vin}"}},{{"label":"Make","value":"Škoda"}},
              {{"label":"Model","value":"Octavia"}},{{"label":"Model Year","value":2020}},
              {{"label":"Body","value":"Combi"}},{{"label":"Engine Displacement (ccm)","value":1968}},
              {{"label":"Engine Power (kW)","value":110}},{{"label":"Fuel Type - Primary","value":"Diesel"}},
              {{"label":"Transmission","value":"Manual"}},{{"label":"Drive","value":"Front-wheel drive"}},
              {{"label":"Plant Country","value":"Czech Republic"}}]}}"#
        ),
        "stolen-check" => r#"{"price":0.49,"price_currency":"EUR","stolen":{"status":"not reported as stolen","sources":"EU police databases (sample)"}}"#.to_string(),
        _ => r#"{"price":0.99,"price_currency":"EUR","vehicle":{"make":"Škoda","model":"Octavia","model_year":2020},"period":{"from":"2026-04-15","to":"2026-10-15"},"prices":{"below":12900,"mean":14650,"above":16400,"currency":"EUR"},"records":[{"price":14200,"odometer":118000},{"price":15100,"odometer":96000}]}"#.to_string(),
    };
    parse_vincario(op, &body)
}

type CacheKey = (String, String, String);

/// In-memory cache of API results keyed by (provider, operation, VIN). Never written to disk.
#[derive(Debug, Default)]
pub struct HistoryCache {
    map: Mutex<HashMap<CacheKey, (Instant, ApiResult)>>,
}

impl HistoryCache {
    pub fn get(&self, key: &CacheKey, now: Instant, force: bool) -> Option<ApiResult> {
        let map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let (at, r) = map.get(key)?;
        let age = now.saturating_duration_since(*at);
        let max = if force || r.status != ApiStatus::Ok {
            MIN_REFRESH
        } else {
            CACHE_TTL
        };
        (age < max).then(|| ApiResult {
            cached: true,
            ..r.clone()
        })
    }

    pub fn put(&self, key: CacheKey, now: Instant, result: ApiResult) {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        map.retain(|_, (at, _)| now.saturating_duration_since(*at) < CACHE_TTL);
        map.insert(key, (now, result));
    }

    /// Forget results of a provider (its key changed).
    pub fn clear_provider(&self, id: &str) {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        map.retain(|(p, _, _), _| p != id);
    }
}

/// Managed state: API key store and result cache.
#[derive(Debug, Default)]
pub struct HistoryState {
    pub cache: HistoryCache,
    secrets: Mutex<Secrets>,
    path: Option<PathBuf>,
}

impl HistoryState {
    pub fn new(path: Option<PathBuf>) -> Self {
        HistoryState {
            cache: HistoryCache::default(),
            secrets: Mutex::new(Secrets::load(path.as_deref())),
            path,
        }
    }

    pub fn secrets(&self) -> std::sync::MutexGuard<'_, Secrets> {
        self.secrets.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Store (`Some`) or remove (`None`) the key of a provider.
    pub fn set_key(&self, id: &str, key: Option<ApiKey>) -> CmdResult<()> {
        let mut s = self.secrets();
        let mut next = s.clone();
        match key {
            Some(k) => {
                next.keys.insert(id.to_string(), k);
            }
            None => {
                next.keys.remove(id);
            }
        }
        if let Some(p) = &self.path {
            next.save(p)
                .map_err(|e| crate::error::with_detail("error.saveSettings", e))?;
        }
        *s = next;
        self.cache.clear_provider(id);
        Ok(())
    }
}

/// Is an API report allowed? Returns the provider's API definition and key.
pub fn api_allowed<'a>(
    settings: &HistorySettings,
    provider: &'a Provider,
    op: &str,
    secrets: &'a Secrets,
) -> CmdResult<(&'a ApiDef, &'a ApiKey)> {
    let def = provider
        .api
        .as_ref()
        .filter(|_| provider.kind == Kind::Api)
        .ok_or("error.historyNotApi")?;
    if !def.operations.iter().any(|o| o == op) {
        return Err(format!("error.historyUnknownOperation|{op}"));
    }
    if !settings.consent_given {
        return Err("error.historyConsent".into());
    }
    let key = secrets.get(&provider.id).ok_or("error.historyNoKey")?;
    Ok((def, key))
}

/// Run (or reuse) an API call. `run` performs the request for the URL.
#[allow(clippy::too_many_arguments)]
pub fn report_with(
    cache: &HistoryCache,
    provider: &Provider,
    def: &ApiDef,
    key: &ApiKey,
    op: &str,
    vin: &str,
    force: bool,
    now: Instant,
    run: impl FnOnce(&str) -> ApiParsed,
) -> ApiResult {
    let vin = normalize_vin(vin);
    let cache_key = (provider.id.clone(), op.to_string(), vin.clone());
    if let Some(r) = cache.get(&cache_key, now, force) {
        return r;
    }
    let parsed = match (vin_format(&vin), api_url(def, op, &vin, key)) {
        (VinFormat::Valid, Some(url)) => run(&url),
        _ => ApiParsed::new(ApiStatus::Error, Some("invalid VIN".into())),
    };
    log::info!(
        "history: {} {} {}… -> {:?}",
        provider.id,
        op,
        vin.get(..3).unwrap_or(""),
        parsed.status
    );
    let result = ApiResult {
        provider_id: provider.id.clone(),
        operation: op.to_string(),
        status: parsed.status,
        fields: parsed.fields,
        price: parsed.price,
        detail: parsed.detail,
        checked_at: chrono::Utc::now().timestamp_millis(),
        cached: false,
    };
    cache.put(cache_key, now, result.clone());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> WmiTable {
        WmiTable::embedded()
    }

    fn skoda() -> VehicleDocument {
        VehicleDocument {
            vehicle_make: "ŠKODA".into(),
            commercial_description: "OCTAVIA".into(),
            vehicle_id_number: "TMBJJ7NE0L0123456".into(),
            year_of_production: "2019".into(),
            date_of_first_registration: "15.01.2020".into(),
            registration_number_of_vehicle: "BG1234AB".into(),
            ..Default::default()
        }
    }

    fn level(cs: &[Check], id: &str) -> Option<(Level, &'static str)> {
        cs.iter().find(|c| c.id == id).map(|c| (c.level, c.variant))
    }

    #[test]
    fn embedded_definitions_are_valid() {
        let file: ProvidersFile = serde_json::from_str(PROVIDERS_JSON).unwrap();
        let ps = providers();
        assert_eq!(
            ps.len(),
            file.providers.len(),
            "every provider must be valid"
        );
        let ids: std::collections::HashSet<_> = ps.iter().map(|p| &p.id).collect();
        assert_eq!(ids.len(), ps.len());
        assert!(ps
            .iter()
            .any(|p| p.id == "uos-carcheck" && p.key == Key::Plate));
        let v = ps.iter().find(|p| p.id == "vincario").unwrap();
        assert_eq!(v.api.as_ref().unwrap().service, Service::Vincario);
        let t = table();
        assert!(t.regions.len() > 40 && t.manufacturers.len() > 80);
        let mut wmis: Vec<_> = t.manufacturers.iter().map(|m| m.wmi.as_str()).collect();
        wmis.sort();
        wmis.dedup();
        assert_eq!(wmis.len(), t.manufacturers.len(), "WMIs must be unique");
        for m in &t.manufacturers {
            assert_eq!(m.wmi.len(), 3);
            for n in &m.names {
                assert_eq!(&normalize_make(n), n, "names are normalised");
            }
        }
        for r in &t.regions {
            assert!(
                order_index(r.from.chars().nth(1).unwrap())
                    <= order_index(r.to.chars().nth(1).unwrap())
            );
        }
    }

    #[test]
    fn invalid_providers_are_skipped() {
        let json = r#"{"providers":[
          {"id":"a","category":"foreign","name":"A","kind":"link","url":"https://a.com/?v={vin}"},
          {"id":"b","category":"foreign","name":"B","kind":"link","url":"http://b.com/"},
          {"id":"c","category":"api","name":"C","kind":"api","url":"https://c.com/"},
          {"id":"d","category":"api","name":"D","kind":"api","url":"https://d.com/",
           "api":{"service":"other","base":"https://api.d.com","operations":["x"]}},
          {"id":"e","category":"api","name":"E","kind":"api","url":"https://e.com/",
           "api":{"service":"vincario","base":"http://api.e.com","operations":["decode"]}},
          {"id":"f","category":"foreign","name":"F","kind":"pdf","url":"https://f.com/"}
        ]}"#;
        let ps = parse_providers(json).unwrap();
        assert_eq!(ps.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["a"]);
    }

    #[test]
    fn vin_basics() {
        assert_eq!(normalize_vin(" tmb-jj7ne0 l0123456 "), "TMBJJ7NE0L0123456");
        assert_eq!(vin_format("TMBJJ7NE0L0123456"), VinFormat::Valid);
        assert_eq!(vin_format("TMBJJ7NE0L012345"), VinFormat::Invalid);
        assert_eq!(vin_format("TMBJJ7NE0L012345O"), VinFormat::Invalid);
        assert_eq!(vin_format(""), VinFormat::Empty);
        // Well-known valid North-American VINs.
        assert_eq!(check_digit("1M8GDM9AXKP042788"), Some('X'));
        assert_eq!(check_digit("11111111111111111"), Some('1'));
        assert_eq!(check_digit("5YJ3E1EA2KF317000"), Some('2'));
    }

    #[test]
    fn model_years() {
        assert_eq!(model_year('L', 2026), Some(2020));
        assert_eq!(model_year('A', 2026), Some(2010));
        assert_eq!(model_year('Y', 2026), Some(2000));
        assert_eq!(model_year('Y', 2029), Some(2030));
        assert_eq!(model_year('9', 2026), Some(2009));
        assert_eq!(model_year('T', 2026), Some(2026));
        assert_eq!(model_year('V', 2026), Some(2027));
        assert_eq!(model_year('W', 2026), Some(1998));
        assert_eq!(model_year('U', 2026), None);
        assert_eq!(model_year('0', 2026), None);
    }

    #[test]
    fn regions_and_manufacturers() {
        let t = table();
        assert_eq!(t.region("VX1A").as_deref(), Some("RS"));
        assert_eq!(t.region("V1AA").as_deref(), Some("RS"));
        assert_eq!(t.region("V3AA").as_deref(), Some("HR"));
        assert_eq!(t.region("TMB").as_deref(), Some("CZ"));
        assert_eq!(t.region("WVW").as_deref(), Some("DE"));
        assert_eq!(t.region("ZFA").as_deref(), Some("IT"));
        assert_eq!(t.region("ZZZ").as_deref(), Some("SI"));
        assert_eq!(t.region("QAA"), None);
        assert_eq!(t.region("UU1").as_deref(), Some("RO"));
        assert_eq!(t.region("U5Y").as_deref(), Some("SK"));
        assert_eq!(t.manufacturer("TMBJJ7NE0L0123456").unwrap().make, "Škoda");
        assert!(t.manufacturer("QQQ").is_none());
    }

    #[test]
    fn decoding() {
        let t = table();
        let d = decode("TMBJJ7NE0L0123456", &t, 2026).unwrap();
        assert_eq!(d.wmi, "TMB");
        assert_eq!(d.vds, "JJ7NE0");
        assert_eq!(d.vis, "L0123456");
        assert_eq!(d.make.as_deref(), Some("Škoda"));
        assert_eq!(d.country.as_deref(), Some("CZ"));
        assert_eq!(d.model_year, Some(2020));
        assert!(!d.north_america);
        assert_eq!(d.check_digit_ok, None);
        // BMW does not encode the model year in Europe.
        let d = decode("WBA8E9C50GK123456", &t, 2026).unwrap();
        assert_eq!(d.model_year, None);
        // Tesla Berlin: country override.
        assert_eq!(
            decode("XP7YGCEK1PB123456", &t, 2026)
                .unwrap()
                .country
                .as_deref(),
            Some("DE")
        );
        let d = decode("5YJ3E1EA2KF317000", &t, 2026).unwrap();
        assert!(d.north_america);
        assert_eq!(d.check_digit_ok, Some(true));
        assert_eq!(d.model_year, Some(2019));
        assert_eq!(
            decode("5YJ3E1EA8KF317000", &t, 2026)
                .unwrap()
                .check_digit_ok,
            Some(false)
        );
        assert!(decode("VX1234567890ABCDE", &t, 2026).is_some());
        assert!(decode("SHORT", &t, 2026).is_none());
    }

    #[test]
    fn makes_and_years() {
        assert_eq!(normalize_make("Škoda"), "SKODA");
        assert_eq!(normalize_make("MERCEDES-BENZ"), "MERCEDESBENZ");
        assert_eq!(normalize_make("Citroën"), "CITROEN");
        assert_eq!(normalize_make("ШКОДА"), "SKODA");
        assert_eq!(year_of("15.01.2020"), Some(2020));
        assert_eq!(year_of("2019"), Some(2019));
        assert_eq!(year_of("15.01.2020."), Some(2020));
        assert_eq!(year_of(""), None);
        assert_eq!(year_of("12345"), None);
    }

    #[test]
    fn consistent_vehicle() {
        let t = table();
        let v = skoda();
        let vin = normalize_vin(&v.vehicle_id_number);
        let d = decode(&vin, &t, 2026);
        let cs = checks(&v, &vin, d.as_ref(), &t, 2026);
        assert_eq!(level(&cs, "vin"), Some((Level::Ok, "valid")));
        assert_eq!(level(&cs, "make"), Some((Level::Ok, "match")));
        assert_eq!(level(&cs, "modelYear"), Some((Level::Ok, "match")));
        assert_eq!(level(&cs, "firstRegistration"), Some((Level::Ok, "new")));
        assert_eq!(level(&cs, "age"), Some((Level::Info, "years")));
        assert!(level(&cs, "checkDigit").is_none());
        assert_eq!(cs.iter().find(|c| c.id == "age").unwrap().args, ["7"]);
    }

    #[test]
    fn inconsistent_vehicle() {
        let t = table();
        let v = VehicleDocument {
            vehicle_make: "VOLKSWAGEN".into(),
            year_of_production: "2012".into(),
            date_of_first_registration: "03.04.2018".into(),
            ..skoda()
        };
        let vin = normalize_vin(&v.vehicle_id_number);
        let d = decode(&vin, &t, 2026);
        let cs = checks(&v, &vin, d.as_ref(), &t, 2026);
        assert_eq!(level(&cs, "make"), Some((Level::Warning, "mismatch")));
        assert_eq!(level(&cs, "modelYear"), Some((Level::Warning, "mismatch")));
        assert_eq!(level(&cs, "firstRegistration"), Some((Level::Info, "late")));
        let v = VehicleDocument {
            year_of_production: "2021".into(),
            ..skoda()
        };
        let cs = checks(&v, &vin, decode(&vin, &t, 2026).as_ref(), &t, 2026);
        assert_eq!(
            level(&cs, "firstRegistration"),
            Some((Level::Warning, "beforeProduction"))
        );
        // Unknown WMI, invalid / empty VIN.
        let v = VehicleDocument {
            vehicle_id_number: "QQQ4567890ABCDEFG".replace('Q', "R"),
            ..skoda()
        };
        let vin = normalize_vin(&v.vehicle_id_number);
        let cs = checks(&v, &vin, decode(&vin, &t, 2026).as_ref(), &t, 2026);
        assert_eq!(level(&cs, "make"), Some((Level::Unknown, "unknownWmi")));
        assert_eq!(
            level(&cs, "modelYear"),
            Some((Level::Unknown, "notEncoded"))
        );
        let cs = checks(&skoda(), "ABC", None, &t, 2026);
        assert_eq!(level(&cs, "vin"), Some((Level::Warning, "invalid")));
        assert!(level(&cs, "make").is_none());
        let cs = checks(&skoda(), "", None, &t, 2026);
        assert_eq!(cs.len(), 1);
        // Zastava sample card: make matches VX1.
        let z = VehicleDocument {
            vehicle_make: "ZASTAVA".into(),
            vehicle_id_number: "VX1234567890ABCDE".into(),
            ..skoda()
        };
        let cs = checks(
            &z,
            "VX1234567890ABCDE",
            decode("VX1234567890ABCDE", &t, 2026).as_ref(),
            &t,
            2026,
        );
        assert_eq!(level(&cs, "make"), Some((Level::Ok, "match")));
    }

    #[test]
    fn info_resolves_urls() {
        let v = skoda();
        let mut secrets = Secrets::default();
        let info = info_for(Some(&v), &providers(), &table(), &secrets, 2026);
        assert_eq!(info.normalized_vin, "TMBJJ7NE0L0123456");
        assert_eq!(info.normalized_plate, "BG1234AB");
        let n = info
            .providers
            .iter()
            .find(|p| p.id == "nhtsa-recalls")
            .unwrap();
        assert_eq!(n.url, "https://www.nhtsa.gov/recalls?vin=TMBJJ7NE0L0123456");
        assert!(n.key_in_url);
        let u = info
            .providers
            .iter()
            .find(|p| p.id == "uos-carcheck")
            .unwrap();
        assert!(!u.key_in_url);
        assert!(u.alt_url.is_some());
        let api = info.providers.iter().find(|p| p.id == "vincario").unwrap();
        assert!(!api.configured);
        assert_eq!(api.api_domain.as_deref(), Some("api.vincario.com"));
        assert_eq!(api.operations[0], "decode");
        secrets.keys.insert(
            "vincario".into(),
            ApiKey {
                api_key: "k".into(),
                secret: "s".into(),
            },
        );
        let info = info_for(None, &providers(), &table(), &secrets, 2026);
        assert!(info.checks.is_empty());
        assert!(
            info.providers
                .iter()
                .find(|p| p.id == "vincario")
                .unwrap()
                .configured
        );
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["vinFormat"], "empty");
        assert_eq!(v["providers"][0]["category"], "domestic");
        assert_eq!(v["providers"][0]["key"], "plate");
        assert_eq!(v["providers"][0]["keyInUrl"], false);
    }

    fn vincario() -> (Provider, ApiDef, ApiKey) {
        let p = providers()
            .into_iter()
            .find(|p| p.id == "vincario")
            .unwrap();
        let def = p.api.clone().unwrap();
        let key = ApiKey {
            api_key: "testkey".into(),
            secret: "testsecret".into(),
        };
        (p, def, key)
    }

    #[test]
    fn vincario_urls() {
        let (_, def, key) = vincario();
        // sha1("TMBJJ7NE0L0123456|decode|testkey|testsecret")
        let sum = vincario_control_sum("TMBJJ7NE0L0123456", "decode", &key);
        assert_eq!(sum.len(), 10);
        assert!(sum.chars().all(|c| c.is_ascii_hexdigit()));
        let expected: String = Sha1::digest(b"TMBJJ7NE0L0123456|decode|testkey|testsecret")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(sum, expected[..10]);
        assert_eq!(
            api_url(&def, "decode", "TMBJJ7NE0L0123456", &key).unwrap(),
            format!("https://api.vincario.com/3.2/testkey/{sum}/decode/TMBJJ7NE0L0123456.json")
        );
        assert!(api_url(&def, "balance", "TMBJJ7NE0L0123456", &key).is_none());
    }

    #[test]
    fn vincario_responses() {
        let p = mock_parsed("decode", "TMBJJ7NE0L0123456");
        assert_eq!(p.status, ApiStatus::Ok);
        assert_eq!(p.price.as_deref(), Some("0.25 EUR"));
        assert_eq!(p.fields[1], ("Make".into(), "Škoda".into()));
        assert_eq!(p.fields[3], ("Model Year".into(), "2020".into()));
        let p = mock_parsed("vehicle-market-value", "X");
        assert!(p.fields.contains(&("prices.mean".into(), "14650".into())));
        assert!(p
            .fields
            .contains(&("records[1].odometer".into(), "96000".into())));
        assert!(p.fields.iter().all(|(k, _)| !k.starts_with("balance")));
        let p = parse_vincario(
            "decode",
            r#"{"error":true,"message":"Invalid control sum"}"#,
        );
        assert_eq!(p.status, ApiStatus::Unauthorized);
        let p = parse_vincario(
            "decode",
            r#"{"error":true,"message":"Insufficient balance"}"#,
        );
        assert_eq!(p.status, ApiStatus::Error);
        assert_eq!(p.detail.as_deref(), Some("Insufficient balance"));
        assert_eq!(parse_vincario("decode", "{}").status, ApiStatus::ParseError);
        assert_eq!(
            parse_vincario("decode", "<html>").status,
            ApiStatus::ParseError
        );
        let (_, def, _) = vincario();
        assert_eq!(
            interpret(&def, "decode", Fetched::Status(403)).status,
            ApiStatus::Unauthorized
        );
        assert_eq!(
            interpret(&def, "decode", Fetched::Status(402)).status,
            ApiStatus::Error
        );
        assert_eq!(
            interpret(&def, "decode", Fetched::Status(502)).status,
            ApiStatus::Unavailable
        );
    }

    #[test]
    fn consent_and_key_are_required() {
        let (p, _, key) = vincario();
        let mut s = HistorySettings::default();
        let mut secrets = Secrets::default();
        assert_eq!(
            api_allowed(&s, &p, "decode", &secrets).unwrap_err(),
            "error.historyConsent"
        );
        s.consent_given = true;
        assert_eq!(
            api_allowed(&s, &p, "decode", &secrets).unwrap_err(),
            "error.historyNoKey"
        );
        secrets.keys.insert("vincario".into(), key);
        assert!(api_allowed(&s, &p, "decode", &secrets).is_ok());
        assert!(api_allowed(&s, &p, "balance", &secrets)
            .unwrap_err()
            .starts_with("error.historyUnknownOperation"));
        let link = providers()
            .into_iter()
            .find(|p| p.id == "carvertical")
            .unwrap();
        assert_eq!(
            api_allowed(&s, &link, "decode", &secrets).unwrap_err(),
            "error.historyNotApi"
        );
    }

    #[test]
    fn cache_reuses_paid_calls() {
        let (p, def, key) = vincario();
        let cache = HistoryCache::default();
        let t0 = Instant::now();
        let calls = std::cell::Cell::new(0);
        let run = |url: &str| {
            calls.set(calls.get() + 1);
            assert!(url.ends_with("/decode/TMBJJ7NE0L0123456.json"));
            mock_parsed("decode", "TMBJJ7NE0L0123456")
        };
        let vin = "tmbjj7ne0l0123456";
        assert!(!report_with(&cache, &p, &def, &key, "decode", vin, false, t0, run).cached);
        let later = t0 + Duration::from_secs(600);
        assert!(report_with(&cache, &p, &def, &key, "decode", vin, false, later, run).cached);
        let soon = t0 + Duration::from_secs(30);
        assert!(report_with(&cache, &p, &def, &key, "decode", vin, true, soon, run).cached);
        assert_eq!(calls.get(), 1);
        let after = t0 + Duration::from_secs(90);
        assert!(!report_with(&cache, &p, &def, &key, "decode", vin, true, after, run).cached);
        assert_eq!(calls.get(), 2);
        cache.clear_provider("vincario");
        assert!(cache
            .get(
                &(
                    "vincario".into(),
                    "decode".into(),
                    "TMBJJ7NE0L0123456".into()
                ),
                after,
                false
            )
            .is_none());
        // Invalid VIN: no request.
        let r = report_with(
            &cache,
            &p,
            &def,
            &key,
            "decode",
            "ABC",
            false,
            t0,
            |_| unreachable!(),
        );
        assert_eq!(r.status, ApiStatus::Error);
    }

    #[test]
    fn secrets_roundtrip() {
        let dir = std::env::temp_dir().join(format!("chitach-secrets-{}", std::process::id()));
        let path = dir.join(SECRETS_FILE);
        let mut s = Secrets::default();
        s.keys.insert(
            "vincario".into(),
            ApiKey {
                api_key: "k".into(),
                secret: "s".into(),
            },
        );
        s.save(&path).unwrap();
        assert_eq!(Secrets::load(Some(&path)), s);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        s.keys.clear();
        s.save(&path).unwrap();
        assert!(!path.exists());
        assert_eq!(Secrets::load(Some(&path)), Secrets::default());
        assert_eq!(
            secrets_path(Some(&dir.join("settings.json"))),
            Some(dir.join(SECRETS_FILE))
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
