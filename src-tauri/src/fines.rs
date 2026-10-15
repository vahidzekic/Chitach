//! Vehicle fines check ("Provera kazni"), see docs/07-istrazivanje-provera-kazni.md.
//!
//! Provider definitions are declarative JSON (`providers/fines.json`, embedded in the
//! binary). Two kinds exist:
//! - `link`: the UI opens the official portal in the system browser (optionally with the
//!   plate in the URL); nothing is sent by the app.
//! - `json_get`: an opt-in automatic check. Only the plate is sent, with a single GET to
//!   the URL in `check.url`; the response is interpreted by the parser named in
//!   `check.parser`. Turning a `link` provider into an automatic one only needs a JSON
//!   edit (and, for a new response format, a new parser here).
//!
//! Multi-city portals (e.g. ParkingNS, which serves Novi Sad, Novi Pazar, …) are modelled
//! as one provider per city sharing a `portal` id; the city-specific page/operator code is
//! `route` and is substituted for `{route}` (and the provider's first city code for
//! `{city}`) in both `url` and `check.url`.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use celik_core::Document;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{error::CmdResult, settings::FinesSettings};

/// Embedded provider definitions.
pub const PROVIDERS_JSON: &str = include_str!("../providers/fines.json");

/// How long a check result is reused.
pub const CACHE_TTL: Duration = Duration::from_secs(30 * 60);
/// Minimum age of a cached result before an explicit re-check (or a retry after a
/// network error) may hit the portal again.
pub const MIN_REFRESH: Duration = Duration::from_secs(60);
/// Timeout of one automatic check.
pub const TIMEOUT: Duration = Duration::from_secs(12);
pub const USER_AGENT: &str = concat!(
    "Chitach/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/vahidzekic/Chitach)"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    /// Parking tickets, bound to the plate.
    Parking,
    /// Fines bound to a person (owner), not the vehicle.
    Person,
    /// Toll.
    Toll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Link,
    JsonGet,
    /// Kinds added by newer definitions are ignored.
    #[serde(other)]
    Unknown,
}

/// Response parser of a `json_get` provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Parser {
    /// JSON array of tickets; empty array = no tickets.
    JsonArray,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckDef {
    /// GET URL template with a `{plate}` placeholder.
    pub url: String,
    pub parser: Parser,
}

/// One provider from `fines.json`. The UI translates the name and the note with the keys
/// `fines.provider.<id>.name` / `.note` and falls back to `name`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub category: Category,
    pub name: String,
    /// Shared portal of a multi-city system (e.g. `parkingns`), informative.
    #[serde(default)]
    pub portal: Option<String>,
    /// City-specific route/operator code on a shared portal (e.g. `ppkNP`), used for `{route}`.
    #[serde(default)]
    pub route: Option<String>,
    /// Plate city codes (Latin, e.g. `BG`, `ČA`) served by this provider.
    #[serde(default)]
    pub cities: Vec<String>,
    pub kind: Kind,
    /// Page opened for the user; may contain `{plate}`.
    pub url: String,
    /// Automatic check (`json_get` only).
    #[serde(default)]
    pub check: Option<CheckDef>,
}

#[derive(Debug, Deserialize)]
struct ProvidersFile {
    #[serde(default)]
    schema: u32,
    providers: Vec<Provider>,
}

/// Host part of an `http(s)` URL.
pub fn url_host(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next()?;
    (!host.is_empty()).then_some(host)
}

impl Provider {
    /// Substitute the provider's own placeholders (`{route}`, `{city}`) in a template;
    /// `{plate}` is left for [`resolve_url`].
    pub fn expand(&self, template: &str) -> String {
        let route = self
            .route
            .as_deref()
            .map(encode_component)
            .unwrap_or_default();
        let city = self
            .cities
            .first()
            .map(|c| encode_component(c))
            .unwrap_or_default();
        template.replace("{route}", &route).replace("{city}", &city)
    }

    /// Page for the user, for a normalised plate.
    pub fn page_url(&self, plate: &str) -> String {
        resolve_url(&self.expand(&self.url), plate)
    }

    /// Automatic check URL, for a normalised plate.
    pub fn check_url(&self, plate: &str) -> Option<String> {
        self.check
            .as_ref()
            .map(|c| resolve_url(&self.expand(&c.url), plate))
    }

    /// Definitions that are complete and safe to use.
    fn is_valid(&self) -> bool {
        let uses_route = self.url.contains("{route}")
            || self
                .check
                .as_ref()
                .is_some_and(|c| c.url.contains("{route}"));
        if self.id.is_empty()
            || url_host(&self.expand(&self.url)).is_none()
            || (uses_route && self.route.as_deref().unwrap_or("").is_empty())
        {
            return false;
        }
        match self.kind {
            Kind::Link => true,
            // Automatic checks only over https, and only with the plate in the URL.
            Kind::JsonGet => self.check.as_ref().is_some_and(|c| {
                c.url.starts_with("https://")
                    && url_host(&c.url).is_some()
                    && c.url.contains("{plate}")
                    && c.parser != Parser::Unknown
            }),
            Kind::Unknown => false,
        }
    }

    pub fn is_auto(&self) -> bool {
        self.kind == Kind::JsonGet
    }
}

/// Parse a definitions file, skipping unknown/invalid providers.
pub fn parse_providers(json: &str) -> Result<Vec<Provider>, serde_json::Error> {
    let file: ProvidersFile = serde_json::from_str(json)?;
    if file.schema > 1 {
        log::warn!("fines.json schema {} is newer than supported", file.schema);
    }
    Ok(file
        .providers
        .into_iter()
        .filter(|p| {
            let ok = p.is_valid();
            if !ok {
                log::warn!("fines: ignoring provider {:?}", p.id);
            }
            ok
        })
        .collect())
}

/// Embedded providers.
pub fn providers() -> Vec<Provider> {
    parse_providers(PROVIDERS_JSON).unwrap_or_else(|e| {
        log::error!("fines.json: {e}");
        Vec::new()
    })
}

// ---------- plates ----------

fn cyrillic_to_latin(c: char) -> Option<&'static str> {
    Some(match c {
        'А' => "A",
        'Б' => "B",
        'В' => "V",
        'Г' => "G",
        'Д' => "D",
        'Ђ' => "Đ",
        'Е' => "E",
        'Ж' => "Ž",
        'З' => "Z",
        'И' => "I",
        'Ј' => "J",
        'К' => "K",
        'Л' => "L",
        'Љ' => "LJ",
        'М' => "M",
        'Н' => "N",
        'Њ' => "NJ",
        'О' => "O",
        'П' => "P",
        'Р' => "R",
        'С' => "S",
        'Т' => "T",
        'Ћ' => "Ć",
        'У' => "U",
        'Ф' => "F",
        'Х' => "H",
        'Ц' => "C",
        'Ч' => "Č",
        'Џ' => "DŽ",
        'Ш' => "Š",
        _ => return None,
    })
}

/// Normalise a plate: uppercase, no spaces/dashes/dots, Cyrillic transliterated to
/// Latin. Serbian Latin letters (Č, Ć, Š, Ž, Đ) are kept.
pub fn normalize_plate(plate: &str) -> String {
    let mut out = String::with_capacity(plate.len());
    for c in plate.chars().flat_map(char::to_uppercase) {
        if !c.is_alphanumeric() {
            continue;
        }
        match cyrillic_to_latin(c) {
            Some(l) => out.push_str(l),
            None => out.push(c),
        }
    }
    out
}

/// City code (the two leading letters) of a normalised plate, e.g. `BG`, `ČA`.
pub fn city_code(normalized: &str) -> Option<String> {
    let letters: String = normalized
        .chars()
        .take_while(|c| c.is_alphabetic())
        .collect();
    (letters.chars().count() == 2).then_some(letters)
}

/// Mask a plate for logs (`BG1***B`).
pub fn mask_plate(plate: &str) -> String {
    let chars: Vec<char> = plate.chars().collect();
    if chars.len() <= 4 {
        return "***".into();
    }
    let mut s: String = chars[..3].iter().collect();
    s.push_str("***");
    s.push(chars[chars.len() - 1]);
    s
}

/// Percent-encode a URL component (RFC 3986 unreserved characters are kept).
pub fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Replace `{plate}` in a URL template with the encoded plate.
pub fn resolve_url(template: &str, plate: &str) -> String {
    template.replace("{plate}", &encode_component(plate))
}

// ---------- providers for a vehicle ----------

/// Provider as shown in the UI (`FinesProvider` in the API contract).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    pub id: String,
    pub name: String,
    pub category: Category,
    pub cities: Vec<String>,
    pub kind: Kind,
    /// Page to open (plate filled in where supported).
    pub url: String,
    /// The plate is part of `url`.
    pub plate_in_url: bool,
    /// Host the plate is sent to by an automatic check (`json_get` only).
    pub check_domain: Option<String>,
    /// Serves the city of the plate.
    pub matches_city: bool,
}

/// `FinesInfo` of the API contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinesInfo {
    pub plate: String,
    pub normalized_plate: String,
    pub city_code: Option<String>,
    pub providers: Vec<ProviderView>,
}

/// Providers for a plate; those serving the plate's city come first (stable order otherwise).
pub fn providers_for(plate: &str, providers: &[Provider]) -> FinesInfo {
    let normalized = normalize_plate(plate);
    let city = city_code(&normalized);
    let mut views: Vec<ProviderView> = providers
        .iter()
        .map(|p| ProviderView {
            id: p.id.clone(),
            name: p.name.clone(),
            category: p.category,
            cities: p.cities.clone(),
            kind: p.kind,
            url: p.page_url(&normalized),
            plate_in_url: p.url.contains("{plate}"),
            check_domain: p
                .check
                .as_ref()
                .filter(|_| p.is_auto())
                .and_then(|c| url_host(&p.expand(&c.url)).map(str::to_string)),
            matches_city: city
                .as_deref()
                .is_some_and(|c| p.cities.iter().any(|x| x == c)),
        })
        .collect();
    views.sort_by_key(|v| !v.matches_city);
    FinesInfo {
        plate: plate.to_string(),
        normalized_plate: normalized,
        city_code: city,
        providers: views,
    }
}

/// Plate of the current vehicle document.
pub fn document_plate(doc: Option<&Document>) -> CmdResult<String> {
    match doc {
        Some(Document::Vehicle(v)) => Ok(v.registration_number_of_vehicle.clone()),
        _ => Err("error.notVehicleCard".into()),
    }
}

// ---------- automatic check ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckStatus {
    /// Checked, nothing found.
    Clean,
    /// Checked, tickets found (see `items`).
    Found,
    /// Portal refused automation (403/429, HTML, captcha) – check by hand.
    Manual,
    /// Network error, timeout or server error.
    Unavailable,
    /// Unexpected response format.
    ParseError,
}

/// One ticket. Known fields are extracted heuristically; all scalar fields of the
/// response item are kept in `fields`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FineItem {
    pub date: Option<String>,
    pub amount: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub fields: BTreeMap<String, String>,
}

/// `FinesCheckResult` of the API contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub provider_id: String,
    pub status: CheckStatus,
    pub items: Vec<FineItem>,
    /// Technical detail (HTTP status, error text).
    pub detail: Option<String>,
    /// Unix time of the check in milliseconds.
    pub checked_at: i64,
    /// Served from the in-memory cache.
    pub cached: bool,
}

/// Outcome of parsing a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub status: CheckStatus,
    pub items: Vec<FineItem>,
    pub detail: Option<String>,
}

impl Parsed {
    fn new(status: CheckStatus, detail: Option<String>) -> Self {
        Parsed {
            status,
            items: Vec::new(),
            detail,
        }
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
        _ => None,
    }
}

/// Flatten scalars (nested objects one level deep as `a.b`).
fn flatten(obj: &serde_json::Map<String, Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (k, v) in obj {
        match v {
            Value::Object(inner) => {
                for (k2, v2) in inner {
                    if let Some(s) = scalar(v2) {
                        out.insert(format!("{k}.{k2}"), s);
                    }
                }
            }
            other => {
                if let Some(s) = scalar(other) {
                    out.insert(k.clone(), s);
                }
            }
        }
    }
    out
}

fn pick(fields: &BTreeMap<String, String>, needles: &[&str]) -> Option<String> {
    for needle in needles {
        for (k, v) in fields {
            if k.to_lowercase().contains(needle) {
                return Some(v.clone());
            }
        }
    }
    None
}

fn item_from(v: &Value) -> FineItem {
    match v {
        Value::Object(obj) => {
            let fields = flatten(obj);
            FineItem {
                date: pick(&fields, &["date", "datum", "vreme", "time", "created"]),
                amount: pick(
                    &fields,
                    &["amount", "iznos", "price", "cena", "total", "sum", "dug"],
                ),
                location: pick(
                    &fields,
                    &[
                        "location", "lokacija", "address", "adresa", "street", "ulica", "zone",
                        "zona", "mesto",
                    ],
                ),
                description: pick(
                    &fields,
                    &[
                        "description",
                        "opis",
                        "reason",
                        "razlog",
                        "napomena",
                        "note",
                    ],
                ),
                fields,
            }
        }
        other => FineItem {
            description: scalar(other),
            ..Default::default()
        },
    }
}

fn looks_like_html(body: &str, content_type: Option<&str>) -> bool {
    let start = body.trim_start().get(..64).unwrap_or(body.trim_start());
    content_type.is_some_and(|c| c.to_ascii_lowercase().contains("html"))
        || start.starts_with('<')
        || start.to_ascii_lowercase().contains("<!doctype")
}

/// `jsonArray` parser: `[]` = clean, `[..]` = tickets. An object wrapping a single
/// array (e.g. `{"data": [...]}`) is accepted too. HTML (captcha, login page) = manual.
pub fn parse_json_array(content_type: Option<&str>, body: &str) -> Parsed {
    if looks_like_html(body, content_type) {
        return Parsed::new(CheckStatus::Manual, Some("HTML response".into()));
    }
    let value: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return Parsed::new(CheckStatus::ParseError, Some(e.to_string())),
    };
    let array = match &value {
        Value::Array(a) => Some(a),
        Value::Object(o) => {
            let arrays: Vec<&Vec<Value>> = o.values().filter_map(Value::as_array).collect();
            (arrays.len() == 1).then(|| arrays[0])
        }
        _ => None,
    };
    let Some(array) = array else {
        return Parsed::new(
            CheckStatus::ParseError,
            Some("unexpected JSON shape".into()),
        );
    };
    if array.is_empty() {
        return Parsed::new(CheckStatus::Clean, None);
    }
    Parsed {
        status: CheckStatus::Found,
        items: array.iter().map(item_from).collect(),
        detail: None,
    }
}

/// Result of one HTTP request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    Body {
        content_type: Option<String>,
        body: String,
    },
    /// HTTP error status.
    Status(u16),
    /// Network error / timeout.
    Network(String),
}

/// Interpret a fetched response with the provider's parser.
pub fn interpret(parser: Parser, fetched: Fetched) -> Parsed {
    match fetched {
        Fetched::Status(code @ (401 | 403 | 429)) => {
            Parsed::new(CheckStatus::Manual, Some(format!("HTTP {code}")))
        }
        Fetched::Status(code) => {
            Parsed::new(CheckStatus::Unavailable, Some(format!("HTTP {code}")))
        }
        Fetched::Network(e) => Parsed::new(CheckStatus::Unavailable, Some(e)),
        Fetched::Body { content_type, body } => match parser {
            Parser::JsonArray => parse_json_array(content_type.as_deref(), &body),
            Parser::Unknown => Parsed::new(CheckStatus::Manual, None),
        },
    }
}

/// One GET request (blocking). Sends nothing but the URL (which carries the plate).
pub fn fetch(url: &str) -> Fetched {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .max_redirects(3)
        .build()
        .into();
    match agent
        .get(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .call()
    {
        Ok(mut rsp) => {
            let content_type = rsp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            match rsp.body_mut().read_to_string() {
                Ok(body) => Fetched::Body { content_type, body },
                Err(e) => Fetched::Network(e.to_string()),
            }
        }
        Err(ureq::Error::StatusCode(code)) => Fetched::Status(code),
        Err(e) => Fetched::Network(e.to_string()),
    }
}

/// Fake result for mock mode: deterministic by plate (even digit sum = two tickets).
pub fn mock_parsed(plate: &str) -> Parsed {
    let sum: u32 = plate.chars().filter_map(|c| c.to_digit(10)).sum();
    if sum % 2 == 1 {
        return Parsed::new(CheckStatus::Clean, None);
    }
    let body = r#"[
      {"datum":"03.10.2026. 10:42","iznos":"2400.00","lokacija":"Bulevar oslobođenja 12, zona 1","opis":"ePPK – posebna parking karta","brojNaloga":"PPK-2026-118204"},
      {"datum":"21.09.2026. 16:05","iznos":"1200.00","lokacija":"Zmaj Jovina 4, zona 0","opis":"ePPK – prekoračenje vremena","brojNaloga":"PPK-2026-109377"}
    ]"#;
    parse_json_array(Some("application/json"), body)
}

// ---------- cache ----------

type CacheKey = (String, String);

/// In-memory cache of check results keyed by (provider, plate). Never written to disk.
#[derive(Debug, Default)]
pub struct FinesCache {
    map: Mutex<HashMap<CacheKey, (Instant, CheckResult)>>,
}

impl FinesCache {
    /// Cached result, if still usable. Errors (unavailable) and forced re-checks reuse a
    /// result only for [`MIN_REFRESH`], anything else for [`CACHE_TTL`].
    pub fn get(
        &self,
        provider: &str,
        plate: &str,
        now: Instant,
        force: bool,
    ) -> Option<CheckResult> {
        let map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let (at, r) = map.get(&(provider.to_string(), plate.to_string()))?;
        let age = now.saturating_duration_since(*at);
        let max = if force || r.status == CheckStatus::Unavailable {
            MIN_REFRESH
        } else {
            CACHE_TTL
        };
        (age < max).then(|| CheckResult {
            cached: true,
            ..r.clone()
        })
    }

    pub fn put(&self, plate: &str, now: Instant, result: CheckResult) {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        map.retain(|_, (at, _)| now.saturating_duration_since(*at) < CACHE_TTL);
        map.insert(
            (result.provider_id.clone(), plate.to_string()),
            (now, result),
        );
    }

    pub fn len(&self) -> usize {
        self.map.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Is an automatic check of `provider` allowed by the settings?
pub fn check_allowed(settings: &FinesSettings, provider: &Provider) -> CmdResult<()> {
    if !provider.is_auto() {
        return Err("error.finesNotAutomatic".into());
    }
    if !settings.consent_given {
        return Err("error.finesConsent".into());
    }
    if !settings.provider_enabled(&provider.id) {
        return Err("error.finesProviderDisabled".into());
    }
    Ok(())
}

/// Run (or reuse) a check. `run` performs the request for the resolved URL.
pub fn check_with(
    cache: &FinesCache,
    provider: &Provider,
    plate: &str,
    force: bool,
    now: Instant,
    run: impl FnOnce(&str) -> Parsed,
) -> CheckResult {
    let normalized = normalize_plate(plate);
    if let Some(r) = cache.get(&provider.id, &normalized, now, force) {
        return r;
    }
    let parsed = match provider.check_url(&normalized) {
        Some(url) if !normalized.is_empty() => run(&url),
        _ => Parsed::new(CheckStatus::Manual, None),
    };
    log::info!(
        "fines: {} {} -> {:?}",
        provider.id,
        mask_plate(&normalized),
        parsed.status
    );
    let result = CheckResult {
        provider_id: provider.id.clone(),
        status: parsed.status,
        items: parsed.items,
        detail: parsed.detail,
        checked_at: chrono::Utc::now().timestamp_millis(),
        cached: false,
    };
    cache.put(&normalized, now, result.clone());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_providers_are_valid() {
        let file: ProvidersFile = serde_json::from_str(PROVIDERS_JSON).unwrap();
        let all = file.providers.len();
        let ps = providers();
        assert_eq!(ps.len(), all, "every embedded provider must be valid");
        let ids: std::collections::HashSet<_> = ps.iter().map(|p| &p.id).collect();
        assert_eq!(ids.len(), ps.len(), "ids must be unique");
        let ns = ps.iter().find(|p| p.id == "parkingns").unwrap();
        assert!(ns.is_auto());
        assert_eq!(ns.check.as_ref().unwrap().parser, Parser::JsonArray);
        for p in &ps {
            for c in &p.cities {
                assert_eq!(normalize_plate(c), *c, "city codes are normalised");
            }
        }
        assert!(ps
            .iter()
            .any(|p| p.id == "np-parking" && p.kind == Kind::Link));
        assert!(ps
            .iter()
            .any(|p| p.id == "rnk" && p.category == Category::Person));
    }

    #[test]
    fn invalid_and_unknown_providers_are_skipped() {
        let json = r#"{"schema":1,"providers":[
          {"id":"a","category":"parking","name":"A","kind":"link","url":"https://a.rs/"},
          {"id":"b","category":"parking","name":"B","kind":"html_form","url":"https://b.rs/"},
          {"id":"c","category":"parking","name":"C","kind":"json_get","url":"https://c.rs/",
           "check":{"url":"http://c.rs/x?p={plate}","parser":"jsonArray"}},
          {"id":"d","category":"parking","name":"D","kind":"json_get","url":"https://d.rs/",
           "check":{"url":"https://d.rs/x","parser":"jsonArray"}},
          {"id":"e","category":"parking","name":"E","kind":"json_get","url":"https://e.rs/",
           "check":{"url":"https://e.rs/x?p={plate}","parser":"somethingNew"}},
          {"id":"f","category":"toll","name":"F","kind":"link","url":"ftp://f.rs/"}
        ]}"#;
        let ps = parse_providers(json).unwrap();
        assert_eq!(ps.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["a"]);
    }

    #[test]
    fn plate_normalisation() {
        assert_eq!(normalize_plate("BG 123-AB"), "BG123AB");
        assert_eq!(normalize_plate(" ns 045 – xy "), "NS045XY");
        assert_eq!(normalize_plate("БГ123АБ"), "BG123AB");
        assert_eq!(normalize_plate("ча 12-шђ"), "ČA12ŠĐ");
        assert_eq!(normalize_plate("Čа 1 ćž"), "ČA1ĆŽ");
        assert_eq!(normalize_plate("НИ 1 ЏЉ"), "NI1DŽLJ");
        assert_eq!(normalize_plate("kg.123.ab"), "KG123AB");
        assert_eq!(normalize_plate(""), "");
    }

    #[test]
    fn city_extraction() {
        assert_eq!(city_code("BG123AB").as_deref(), Some("BG"));
        assert_eq!(city_code("ČA12ŠĐ").as_deref(), Some("ČA"));
        assert_eq!(
            city_code(&normalize_plate("ЗР 1 АА")).as_deref(),
            Some("ZR")
        );
        assert_eq!(city_code("P12345"), None);
        assert_eq!(city_code("ABC123"), None);
        assert_eq!(city_code("123"), None);
        assert_eq!(city_code(""), None);
    }

    #[test]
    fn url_templating() {
        assert_eq!(
            resolve_url("https://x.rs/check?platePr={plate}", "NS123AB"),
            "https://x.rs/check?platePr=NS123AB"
        );
        assert_eq!(
            resolve_url("https://x.rs/?p={plate}", "ČA12Š"),
            "https://x.rs/?p=%C4%8CA12%C5%A0"
        );
        assert_eq!(resolve_url("https://x.rs/a b&", "X"), "https://x.rs/a b&");
        assert_eq!(encode_component("a/b?&="), "a%2Fb%3F%26%3D");
        assert_eq!(
            url_host("https://portal.parkingns.rs/portal/x?y"),
            Some("portal.parkingns.rs")
        );
        assert_eq!(
            url_host("http://status.parkingcacak.co.rs/"),
            Some("status.parkingcacak.co.rs")
        );
        assert_eq!(url_host("ftp://x"), None);
    }

    #[test]
    fn providers_sorted_by_city() {
        let info = providers_for("ns 123-ab", &providers());
        assert_eq!(info.normalized_plate, "NS123AB");
        assert_eq!(info.city_code.as_deref(), Some("NS"));
        assert_eq!(info.providers[0].id, "parkingns");
        assert!(info.providers[0].matches_city);
        assert!(info.providers[1..].iter().all(|p| !p.matches_city));
        assert_eq!(
            info.providers[0].check_domain.as_deref(),
            Some("portal.parkingns.rs")
        );
        // Stable order otherwise: Belgrade is the first of the rest.
        assert_eq!(info.providers[1].id, "bg-parking");

        let info = providers_for("ČA 12-AB", &providers());
        assert_eq!(info.providers[0].id, "ca-parking");
        let info = providers_for("P 12345", &providers());
        assert!(info.city_code.is_none());
        assert!(info.providers.iter().all(|p| !p.matches_city));
        assert!(info
            .providers
            .iter()
            .all(|p| p.kind == Kind::JsonGet || p.check_domain.is_none()));
    }

    #[test]
    fn plate_in_url_is_resolved() {
        let p = parse_providers(
            r#"{"providers":[{"id":"x","category":"parking","name":"X","kind":"link","url":"https://x.rs/?plate={plate}"}]}"#,
        )
        .unwrap();
        let info = providers_for("bg 1-ab", &p);
        assert_eq!(info.providers[0].url, "https://x.rs/?plate=BG1AB");
        assert!(info.providers[0].plate_in_url);
    }

    #[test]
    fn multi_city_routes() {
        let p = parse_providers(
            r#"{"providers":[
              {"id":"np","category":"parking","name":"NP","portal":"parkingns","route":"ppkNP",
               "cities":["NP"],"kind":"json_get","url":"https://portal.parkingns.rs/{route}",
               "check":{"url":"https://portal.parkingns.rs/portal/auth/checkPPK?platePr={plate}&op={route}&c={city}","parser":"jsonArray"}},
              {"id":"bad","category":"parking","name":"B","cities":["XX"],"kind":"link","url":"https://x.rs/{route}"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(p.len(), 1, "a {{route}} template needs a route");
        assert_eq!(p[0].page_url("NP1AB"), "https://portal.parkingns.rs/ppkNP");
        assert_eq!(
            p[0].check_url("NP1AB").unwrap(),
            "https://portal.parkingns.rs/portal/auth/checkPPK?platePr=NP1AB&op=ppkNP&c=NP"
        );
        let info = providers_for("NP 1-AB", &p);
        assert!(info.providers[0].matches_city);
        assert_eq!(
            info.providers[0].check_domain.as_deref(),
            Some("portal.parkingns.rs")
        );
    }

    const EMPTY: &str = "[]";
    const FOUND: &str = r#"[
      {"id": 77, "plateNumber": "NS123AB", "issueDate": "2026-10-01T09:15:00",
       "amount": 2400, "zone": {"name": "Zona 1"}, "street": "Zmaj Jovina", "paid": false},
      {"id": 78, "createdAt": "2026-10-02", "price": "1200.00", "napomena": "ePPK"}
    ]"#;
    const HTML: &str = "<!DOCTYPE html><html><head><title>Just a moment...</title></head></html>";

    #[test]
    fn parkingns_empty_is_clean() {
        let p = parse_json_array(Some("application/json"), EMPTY);
        assert_eq!(p.status, CheckStatus::Clean);
        assert!(p.items.is_empty());
        let p = parse_json_array(None, " [ ] \n");
        assert_eq!(p.status, CheckStatus::Clean);
    }

    #[test]
    fn parkingns_items_are_found() {
        let p = parse_json_array(Some("application/json;charset=UTF-8"), FOUND);
        assert_eq!(p.status, CheckStatus::Found);
        assert_eq!(p.items.len(), 2);
        let a = &p.items[0];
        assert_eq!(a.date.as_deref(), Some("2026-10-01T09:15:00"));
        assert_eq!(a.amount.as_deref(), Some("2400"));
        assert_eq!(a.location.as_deref(), Some("Zmaj Jovina"));
        assert_eq!(
            a.fields.get("zone.name").map(String::as_str),
            Some("Zona 1")
        );
        assert_eq!(a.fields.get("paid").map(String::as_str), Some("false"));
        let b = &p.items[1];
        assert_eq!(b.date.as_deref(), Some("2026-10-02"));
        assert_eq!(b.amount.as_deref(), Some("1200.00"));
        assert_eq!(b.description.as_deref(), Some("ePPK"));
        assert_eq!(b.location, None);
        // Wrapped array and non-object items.
        let p = parse_json_array(None, r#"{"data":["PPK-1"]}"#);
        assert_eq!(p.status, CheckStatus::Found);
        assert_eq!(p.items[0].description.as_deref(), Some("PPK-1"));
    }

    #[test]
    fn parkingns_html_is_manual() {
        assert_eq!(
            parse_json_array(Some("text/html"), HTML).status,
            CheckStatus::Manual
        );
        assert_eq!(parse_json_array(None, HTML).status, CheckStatus::Manual);
        assert_eq!(
            parse_json_array(Some("text/html; charset=utf-8"), "[]").status,
            CheckStatus::Manual
        );
    }

    #[test]
    fn unexpected_responses() {
        assert_eq!(
            parse_json_array(None, "{\"a\":1}").status,
            CheckStatus::ParseError
        );
        assert_eq!(
            parse_json_array(None, "not json").status,
            CheckStatus::ParseError
        );
        assert_eq!(parse_json_array(None, "42").status, CheckStatus::ParseError);
        let s = |f| interpret(Parser::JsonArray, f).status;
        assert_eq!(s(Fetched::Status(403)), CheckStatus::Manual);
        assert_eq!(s(Fetched::Status(429)), CheckStatus::Manual);
        assert_eq!(s(Fetched::Status(503)), CheckStatus::Unavailable);
        assert_eq!(
            s(Fetched::Network("timeout".into())),
            CheckStatus::Unavailable
        );
        assert_eq!(
            s(Fetched::Body {
                content_type: Some("application/json".into()),
                body: EMPTY.into()
            }),
            CheckStatus::Clean
        );
    }

    fn ns() -> Provider {
        providers()
            .into_iter()
            .find(|p| p.id == "parkingns")
            .unwrap()
    }

    #[test]
    fn cache_reuses_results() {
        let cache = FinesCache::default();
        let p = ns();
        let t0 = Instant::now();
        let calls = std::cell::Cell::new(0);
        let run = |url: &str| {
            calls.set(calls.get() + 1);
            assert_eq!(
                url,
                "https://portal.parkingns.rs/portal/auth/checkPPK?platePr=NS123AB"
            );
            parse_json_array(None, FOUND)
        };
        let r = check_with(&cache, &p, "ns 123-ab", false, t0, run);
        assert_eq!(r.status, CheckStatus::Found);
        assert!(!r.cached);
        // Same plate in another spelling, within TTL: cached, no request.
        let r = check_with(
            &cache,
            &p,
            "NS123AB",
            false,
            t0 + Duration::from_secs(600),
            run,
        );
        assert!(r.cached);
        assert_eq!(r.items.len(), 2);
        assert_eq!(calls.get(), 1);
        // Forced re-check within MIN_REFRESH is still served from cache.
        let r = check_with(
            &cache,
            &p,
            "NS123AB",
            true,
            t0 + Duration::from_secs(30),
            run,
        );
        assert!(r.cached);
        assert_eq!(calls.get(), 1);
        // Forced after MIN_REFRESH: new request.
        let r = check_with(
            &cache,
            &p,
            "NS123AB",
            true,
            t0 + Duration::from_secs(90),
            run,
        );
        assert!(!r.cached);
        assert_eq!(calls.get(), 2);
        // Expired after TTL.
        let later = t0 + Duration::from_secs(90) + CACHE_TTL;
        assert!(cache.get("parkingns", "NS123AB", later, false).is_none());
        // Other plate: separate entry.
        check_with(&cache, &p, "NS999ZZ", false, t0, |_| {
            parse_json_array(None, EMPTY)
        });
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn cache_retries_errors_after_min_refresh() {
        let cache = FinesCache::default();
        let p = ns();
        let t0 = Instant::now();
        let down = |_: &str| interpret(Parser::JsonArray, Fetched::Network("dns".into()));
        let r = check_with(&cache, &p, "NS1AA", false, t0, down);
        assert_eq!(r.status, CheckStatus::Unavailable);
        assert!(cache
            .get("parkingns", "NS1AA", t0 + Duration::from_secs(10), false)
            .is_some());
        assert!(cache
            .get("parkingns", "NS1AA", t0 + MIN_REFRESH, false)
            .is_none());
    }

    #[test]
    fn consent_and_enabled_are_required() {
        let p = ns();
        let mut s = FinesSettings::default();
        assert_eq!(check_allowed(&s, &p).unwrap_err(), "error.finesConsent");
        s.consent_given = true;
        assert!(check_allowed(&s, &p).is_ok());
        s.enabled_providers = Some(vec![]);
        assert_eq!(
            check_allowed(&s, &p).unwrap_err(),
            "error.finesProviderDisabled"
        );
        s.enabled_providers = Some(vec!["parkingns".into()]);
        assert!(check_allowed(&s, &p).is_ok());
        let link = providers()
            .into_iter()
            .find(|p| p.id == "bg-parking")
            .unwrap();
        assert_eq!(
            check_allowed(&s, &link).unwrap_err(),
            "error.finesNotAutomatic"
        );
    }

    #[test]
    fn mock_results() {
        assert_eq!(mock_parsed("BG123AB").status, CheckStatus::Found);
        assert_eq!(mock_parsed("BG123AB").items.len(), 2);
        assert_eq!(mock_parsed("NS1AB").status, CheckStatus::Clean);
    }

    #[test]
    fn masking() {
        assert_eq!(mask_plate("BG123AB"), "BG1***B");
        assert_eq!(mask_plate("AB1"), "***");
    }

    #[test]
    fn serializes_camel_case() {
        let r = CheckResult {
            provider_id: "x".into(),
            status: CheckStatus::ParseError,
            items: vec![FineItem::default()],
            detail: None,
            checked_at: 1,
            cached: false,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["providerId"], "x");
        assert_eq!(v["status"], "parseError");
        assert_eq!(v["checkedAt"], 1);
        let info = providers_for("BG1AB", &providers());
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["normalizedPlate"], "BG1AB");
        assert_eq!(v["cityCode"], "BG");
        assert_eq!(v["providers"][0]["kind"], "link");
        assert_eq!(v["providers"][0]["matchesCity"], true);
        assert_eq!(v["providers"][0]["category"], "parking");
    }
}
