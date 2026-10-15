//! Vehicle fines check ("Provera kazni"), see docs/07-istrazivanje-provera-kazni.md.
//!
//! Provider definitions are declarative JSON (`providers/fines.json`, embedded in the
//! binary; newer signed copies are downloaded by [`crate::fines_defs`]). Four kinds exist:
//! - `link`: the UI opens the official portal in the system browser (optionally with the
//!   plate in the URL); nothing is sent by the app.
//! - `json_get`: an opt-in automatic check. Only the plate is sent, with a single GET to
//!   the URL in `check.url`; the response is interpreted by the parser named in
//!   `check.parser`. Turning a `link` provider into an automatic one only needs a JSON
//!   edit (and, for a new response format, a new parser here).
//! - `html_form`: an opt-in automatic check of a portal that answers a plain HTML form:
//!   one GET or POST (`check.method`, form fields in `check.form`), the answer page is
//!   classified by text markers (`check.found` / `check.clean`, parser `htmlMarkers`).
//! - `json_post`: an opt-in automatic check of a JSON API that takes a POST with a JSON
//!   body (`check.json`, string values may contain `{plate}`), answered like `json_get`
//!   (parser `jsonArray`); `check.emptyIsClean` makes an empty answer mean "no tickets".
//!
//! A provider whose `kind` or `check` this version does not understand is still shown,
//! as a `link` to its page.
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
    HtmlForm,
    JsonPost,
    /// Kinds added by newer definitions are ignored.
    #[serde(other)]
    Unknown,
}

/// Response parser of a `json_get` provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Parser {
    /// JSON array of tickets; empty array = no tickets.
    #[default]
    JsonArray,
    /// HTML answer page classified by `found` / `clean` text markers.
    HtmlMarkers,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Method {
    #[default]
    Get,
    /// `application/x-www-form-urlencoded` body from `form`.
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckDef {
    /// URL template; `{plate}` here or in a `form` value.
    pub url: String,
    pub parser: Parser,
    #[serde(default)]
    pub method: Method,
    /// Form fields (name -> value template), sent as the query of a GET or the body of a POST.
    #[serde(default)]
    pub form: BTreeMap<String, String>,
    /// `htmlMarkers`: text that appears on the answer page only when tickets were found.
    #[serde(default)]
    pub found: Vec<String>,
    /// `htmlMarkers`: text that appears on the answer page only when nothing was found.
    #[serde(default)]
    pub clean: Vec<String>,
    /// `json_post`: JSON object sent as the body; `{plate}`, `{route}`, `{city}` are
    /// substituted in its string values.
    #[serde(default)]
    pub json: serde_json::Map<String, Value>,
    /// An empty answer means "no tickets" (the portal says so by sending nothing).
    #[serde(default)]
    pub empty_is_clean: bool,
}

impl CheckDef {
    fn has_plate(&self) -> bool {
        self.url.contains("{plate}")
            || self.form.values().any(|v| v.contains("{plate}"))
            || self
                .json
                .values()
                .any(|v| v.as_str().is_some_and(|s| s.contains("{plate}")))
    }
}

/// One automatic-check request, resolved for a plate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Request {
    pub method: Method,
    pub url: String,
    /// Form fields (query of a GET, body of a POST), values not encoded.
    pub form: Vec<(String, String)>,
    /// JSON body of a POST (`json_post`); sent instead of `form`.
    pub json: Option<Value>,
    pub parser: Parser,
    pub found: Vec<String>,
    pub clean: Vec<String>,
    pub empty_is_clean: bool,
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
    /// Automatic check (`json_get`, `html_form`, `json_post`).
    #[serde(default)]
    pub check: Option<CheckDef>,
}

#[derive(Debug, Deserialize)]
struct ProvidersFile {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    updated: String,
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

    /// Automatic check URL (without form fields), for a normalised plate.
    pub fn check_url(&self, plate: &str) -> Option<String> {
        self.check
            .as_ref()
            .map(|c| resolve_url(&self.expand(&c.url), plate))
    }

    /// Automatic check request, for a normalised plate.
    pub fn check_request(&self, plate: &str) -> Option<Request> {
        let c = self.check.as_ref()?;
        Some(Request {
            method: c.method,
            url: resolve_url(&self.expand(&c.url), plate),
            form: c
                .form
                .iter()
                .map(|(k, v)| (k.clone(), self.expand(v).replace("{plate}", plate)))
                .collect(),
            json: (self.kind == Kind::JsonPost).then(|| {
                Value::Object(
                    c.json
                        .iter()
                        .map(|(k, v)| {
                            let v = match v {
                                Value::String(s) => {
                                    Value::String(self.expand_raw(s).replace("{plate}", plate))
                                }
                                other => other.clone(),
                            };
                            (k.clone(), v)
                        })
                        .collect(),
                )
            }),
            parser: c.parser,
            found: c.found.clone(),
            clean: c.clean.clone(),
            empty_is_clean: c.empty_is_clean,
        })
    }

    /// Like [`Provider::expand`], without URL encoding (for JSON bodies).
    fn expand_raw(&self, template: &str) -> String {
        let route = self.route.as_deref().unwrap_or_default();
        let city = self.cities.first().map(String::as_str).unwrap_or_default();
        template.replace("{route}", route).replace("{city}", city)
    }

    fn uses_route(&self) -> bool {
        self.url.contains("{route}")
            || self.check.as_ref().is_some_and(|c| {
                c.url.contains("{route}")
                    || c.form.values().any(|v| v.contains("{route}"))
                    || c.json
                        .values()
                        .any(|v| v.as_str().is_some_and(|s| s.contains("{route}")))
            })
    }

    /// Is the user page usable (a `link` can be shown)?
    fn page_is_valid(&self) -> bool {
        !self.id.is_empty()
            && url_host(&self.expand(&self.url)).is_some()
            && !(self.uses_route() && self.route.as_deref().unwrap_or("").is_empty())
    }

    /// Is the automatic check complete and safe to use?
    fn check_is_valid(&self) -> bool {
        // Automatic checks only over https, and only with the plate in the request.
        let Some(c) = self.check.as_ref() else {
            return false;
        };
        let base = c.url.starts_with("https://") && url_host(&c.url).is_some() && c.has_plate();
        base && match self.kind {
            Kind::JsonGet => c.parser == Parser::JsonArray && c.method == Method::Get,
            Kind::JsonPost => c.parser == Parser::JsonArray && !c.json.is_empty(),
            // Both marker lists are required: a page that matches neither is never "clean".
            Kind::HtmlForm => {
                c.parser == Parser::HtmlMarkers
                    && !c.found.iter().all(|m| m.trim().is_empty())
                    && !c.clean.iter().all(|m| m.trim().is_empty())
            }
            Kind::Link | Kind::Unknown => false,
        }
    }

    pub fn is_auto(&self) -> bool {
        matches!(self.kind, Kind::JsonGet | Kind::HtmlForm | Kind::JsonPost)
    }

    /// Validate; an automatic check this version cannot run degrades to a `link`.
    fn sanitized(mut self) -> Option<Provider> {
        if !self.page_is_valid() {
            return None;
        }
        if self.kind != Kind::Link && !self.check_is_valid() {
            log::warn!("fines: provider {:?} shown as a link only", self.id);
            self.kind = Kind::Link;
        }
        if self.kind == Kind::Link {
            self.check = None;
        }
        Some(self)
    }
}

/// Parse a definitions file, skipping invalid providers.
pub fn parse_providers(json: &str) -> Result<Vec<Provider>, serde_json::Error> {
    parse_definitions(json).map(|d| d.providers)
}

/// A parsed definitions file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definitions {
    pub schema: u32,
    /// `updated` of the file (ISO date), used to pick the newest definitions.
    pub updated: String,
    pub providers: Vec<Provider>,
}

/// Highest definitions schema this version understands fully.
pub const SCHEMA: u32 = 1;

/// Parse a definitions file, skipping invalid providers.
pub fn parse_definitions(json: &str) -> Result<Definitions, serde_json::Error> {
    let file: ProvidersFile = serde_json::from_str(json)?;
    if file.schema > SCHEMA {
        log::warn!("fines.json schema {} is newer than supported", file.schema);
    }
    let providers = file
        .providers
        .into_iter()
        .filter_map(|p| {
            let id = p.id.clone();
            let p = p.sanitized();
            if p.is_none() {
                log::warn!("fines: ignoring provider {id:?}");
            }
            p
        })
        .collect();
    Ok(Definitions {
        schema: file.schema,
        updated: file.updated,
        providers,
    })
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
    /// Has an automatic check (`json_get`, `html_form`, `json_post`).
    pub auto: bool,
    /// Page to open (plate filled in where supported).
    pub url: String,
    /// The plate is part of `url`.
    pub plate_in_url: bool,
    /// Host the plate is sent to by an automatic check.
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
    /// Which provider definitions are in use (set by the command).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definitions: Option<crate::fines_defs::DefinitionsInfo>,
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
            auto: p.is_auto(),
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
        definitions: None,
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
                    &[
                        "amount", "iznos", "zauplatu", "price", "cena", "total", "sum", "dug",
                    ],
                ),
                location: pick(
                    &fields,
                    &[
                        "location",
                        "lokacija",
                        "address",
                        "adresa",
                        "street",
                        "ulica",
                        "zone",
                        "zona",
                        "mesto",
                        "nazivgrada",
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
                        "nazivstatusa",
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
/// array (e.g. `{"data": [...]}`) is accepted too, also when the whole answer is JSON
/// encoded once more as a string (mojekazne: `"{\"table\":[...]}"`). HTML (captcha, login
/// page) = manual. Rows that only report an error (`{"response":"ERROR_MESSAGE"}`) are
/// an unexpected answer, never tickets and never "clean".
pub fn parse_json_array(content_type: Option<&str>, body: &str) -> Parsed {
    if looks_like_html(body, content_type) {
        return Parsed::new(CheckStatus::Manual, Some("HTML response".into()));
    }
    let mut value: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return Parsed::new(CheckStatus::ParseError, Some(e.to_string())),
    };
    if let Value::String(inner) = &value {
        value = match serde_json::from_str(inner) {
            Ok(v) => v,
            Err(e) => return Parsed::new(CheckStatus::ParseError, Some(e.to_string())),
        };
    }
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
    let error_row = |v: &Value| {
        v.get("response")
            .and_then(Value::as_str)
            .is_some_and(|r| r.to_ascii_uppercase().contains("ERROR"))
    };
    if array.iter().all(error_row) {
        return Parsed::new(CheckStatus::ParseError, Some("error row".into()));
    }
    Parsed {
        status: CheckStatus::Found,
        items: array.iter().map(item_from).collect(),
        detail: None,
    }
}

// ---------- htmlMarkers parser ----------

/// Text for marker matching: Cyrillic transliterated to Latin, lowercase, whitespace
/// collapsed, so `Nema izdatih karata` also matches `Нема издатих карата`.
pub fn normalize_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = true;
    for c in s.chars().flat_map(char::to_uppercase) {
        if c.is_whitespace() {
            if !space {
                out.push(' ');
                space = true;
            }
            continue;
        }
        space = false;
        match cyrillic_to_latin(c) {
            Some(l) => out.push_str(l),
            None => out.push(c),
        }
    }
    out.trim_end().to_lowercase()
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest.find(';').filter(|&e| e <= 10);
        let decoded = end.and_then(|e| {
            let name = &rest[1..e];
            let c = match name {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                "nbsp" => ' ',
                _ => {
                    let n = name.strip_prefix('#')?;
                    let code = match n.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => n.parse().ok()?,
                    };
                    char::from_u32(code)?
                }
            };
            Some((c, e))
        });
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Drop `<script>`/`<style>` blocks and comments, replace tags by spaces, decode entities.
pub fn html_text(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    while i < html.len() {
        let Some(off) = lower[i..].find('<') else {
            out.push_str(&html[i..]);
            break;
        };
        out.push_str(&html[i..i + off]);
        let start = i + off;
        let skip_to = if lower[start..].starts_with("<!--") {
            lower[start..].find("-->").map(|e| start + e + 3)
        } else if lower[start..].starts_with("<script") || lower[start..].starts_with("<style") {
            let close = if lower[start..].starts_with("<script") {
                "</script"
            } else {
                "</style"
            };
            lower[start..]
                .find(close)
                .and_then(|e| lower[start + e..].find('>').map(|g| start + e + g + 1))
        } else {
            lower[start..].find('>').map(|e| start + e + 1)
        };
        out.push(' ');
        i = skip_to.unwrap_or(html.len());
    }
    decode_entities(&out)
}

/// Inner HTML of every `<tag ...>…</tag>` in `html` (not nested-aware, fine for cells/rows).
fn elements<'a>(html: &'a str, lower: &str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}");
    let close = format!("</{tag}");
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(off) = lower[i..].find(&open) {
        let start = i + off;
        // `<td` must not match `<tdx`.
        let after = lower[start + open.len()..].chars().next();
        if !matches!(after, Some('>' | ' ' | '\t' | '\n' | '\r' | '/')) {
            i = start + open.len();
            continue;
        }
        let Some(gt) = lower[start..].find('>') else {
            break;
        };
        let body = start + gt + 1;
        let end = lower[body..].find(&close).map_or(lower.len(), |e| body + e);
        out.push(&html[body..end]);
        i = end.max(body);
    }
    out
}

fn cell_text(html: &str) -> String {
    html_text(html)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Rows of the HTML tables as JSON objects (header cells as keys, else `col1`, `col2`, …).
fn table_items(html: &str) -> Vec<FineItem> {
    let lower = html.to_ascii_lowercase();
    let mut items = Vec::new();
    for table in elements(html, &lower, "table") {
        let tl = table.to_ascii_lowercase();
        let mut header: Vec<String> = Vec::new();
        for row in elements(table, &tl, "tr") {
            let rl = row.to_ascii_lowercase();
            let th: Vec<String> = elements(row, &rl, "th")
                .into_iter()
                .map(cell_text)
                .collect();
            if !th.is_empty() {
                header = th;
                continue;
            }
            let td: Vec<String> = elements(row, &rl, "td")
                .into_iter()
                .map(cell_text)
                .collect();
            if td.iter().all(String::is_empty) {
                continue;
            }
            let mut obj = serde_json::Map::new();
            for (n, v) in td.into_iter().enumerate() {
                let key = header
                    .get(n)
                    .filter(|h| !h.is_empty())
                    .cloned()
                    .unwrap_or_else(|| format!("col{}", n + 1));
                obj.insert(key, Value::String(v));
            }
            items.push(item_from(&Value::Object(obj)));
        }
    }
    items
}

/// Bot protection widgets: the user has to check by hand.
const CAPTCHA_MARKERS: [&str; 5] = [
    "g-recaptcha",
    "cf-turnstile",
    "h-captcha",
    "challenge-platform",
    "captcha",
];

/// Some portals answer an AJAX call with JSON that carries the HTML fragment in a string
/// field (Niš: `{"success":true,"data":"<div>…</div>"}`). Joins every string value of such
/// a body so the markers see the fragment; anything that is not JSON is returned as is.
fn html_of(body: &str) -> String {
    fn strings(v: &Value, out: &mut String) {
        match v {
            Value::String(s) => {
                out.push_str(s);
                out.push('\n');
            }
            Value::Array(a) => a.iter().for_each(|v| strings(v, out)),
            Value::Object(o) => o.values().for_each(|v| strings(v, out)),
            _ => {}
        }
    }
    let trimmed = body.trim_start_matches('\u{feff}').trim_start();
    if !trimmed.starts_with(['{', '[']) {
        return body.to_string();
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(v) => {
            let mut out = String::new();
            strings(&v, &mut out);
            out
        }
        Err(_) => body.to_string(),
    }
}

/// `htmlMarkers` parser. Order matters: a captcha means manual; a `found` marker wins
/// over a `clean` one (a false "no fines" is worse than a false alarm); a page with
/// neither marker is a parse error, never "clean".
pub fn parse_html_markers(body: &str, found: &[String], clean: &[String]) -> Parsed {
    let html = html_of(body);
    let body = html.as_str();
    let lower = body.to_ascii_lowercase();
    if CAPTCHA_MARKERS.iter().any(|m| lower.contains(m)) {
        return Parsed::new(CheckStatus::Manual, Some("captcha".into()));
    }
    let text = normalize_text(&html_text(body));
    let has = |markers: &[String]| {
        markers
            .iter()
            .map(|m| normalize_text(m))
            .any(|m| !m.is_empty() && text.contains(&m))
    };
    if has(found) {
        let mut items = table_items(body);
        if items.is_empty() {
            items.push(FineItem::default());
        }
        return Parsed {
            status: CheckStatus::Found,
            items,
            detail: None,
        };
    }
    if has(clean) {
        return Parsed::new(CheckStatus::Clean, None);
    }
    Parsed::new(CheckStatus::ParseError, Some("no known marker".into()))
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

/// Interpret a fetched response with the request's parser.
pub fn interpret(req: &Request, fetched: Fetched) -> Parsed {
    match fetched {
        Fetched::Status(code @ (401 | 403 | 429)) => {
            Parsed::new(CheckStatus::Manual, Some(format!("HTTP {code}")))
        }
        Fetched::Status(code) => {
            Parsed::new(CheckStatus::Unavailable, Some(format!("HTTP {code}")))
        }
        Fetched::Network(e) => Parsed::new(CheckStatus::Unavailable, Some(e)),
        Fetched::Body { content_type, body } => match req.parser {
            // An empty answer, or an empty JSON string (`""`).
            Parser::JsonArray if req.empty_is_clean && matches!(body.trim(), "" | "\"\"") => {
                Parsed::new(CheckStatus::Clean, None)
            }
            Parser::JsonArray => parse_json_array(content_type.as_deref(), &body),
            Parser::HtmlMarkers => parse_html_markers(&body, &req.found, &req.clean),
            Parser::Unknown => Parsed::new(CheckStatus::Manual, None),
        },
    }
}

impl Request {
    /// URL with the form fields as query (GET) or as is (POST, JSON body).
    pub fn full_url(&self) -> String {
        if self.method == Method::Post || self.json.is_some() || self.form.is_empty() {
            return self.url.clone();
        }
        let sep = if self.url.contains('?') { '&' } else { '?' };
        format!("{}{sep}{}", self.url, self.encoded_form())
    }

    /// `application/x-www-form-urlencoded` form fields.
    pub fn encoded_form(&self) -> String {
        self.form
            .iter()
            .map(|(k, v)| format!("{}={}", encode_component(k), encode_component(v)))
            .collect::<Vec<_>>()
            .join("&")
    }
}

/// One request (blocking). Sends nothing but the request itself (which carries the plate).
pub fn fetch(req: &Request) -> Fetched {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .max_redirects(3)
        .build()
        .into();
    let accept = match req.parser {
        Parser::HtmlMarkers => "text/html",
        _ => "application/json",
    };
    let url = req.full_url();
    let response = match (req.method, &req.json) {
        (_, Some(json)) => agent
            .post(&url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", accept)
            .content_type("application/json")
            .send(json.to_string()),
        (Method::Get, None) => agent
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", accept)
            .call(),
        (Method::Post, None) => agent
            .post(&url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", accept)
            .content_type("application/x-www-form-urlencoded")
            .send(req.encoded_form()),
    };
    match response {
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

/// Run (or reuse) a check. `run` performs the resolved request.
pub fn check_with(
    cache: &FinesCache,
    provider: &Provider,
    plate: &str,
    force: bool,
    now: Instant,
    run: impl FnOnce(&Request) -> Parsed,
) -> CheckResult {
    let normalized = normalize_plate(plate);
    if let Some(r) = cache.get(&provider.id, &normalized, now, force) {
        return r;
    }
    let parsed = match provider.check_request(&normalized) {
        Some(req) if !normalized.is_empty() => run(&req),
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
        let np = ps.iter().find(|p| p.id == "np-parking").unwrap();
        assert!(np.is_auto());
        assert_eq!(np.page_url("NP123AB"), "https://portal.parkingns.rs/ppkNP");
        assert_eq!(
            np.check_url("NP123AB").unwrap(),
            "https://portal.parkingns.rs/portal/auth/checkPPKNP?platePr=NP123AB"
        );
        assert!(ps
            .iter()
            .any(|p| p.id == "rnk" && p.category == Category::Person));
    }

    #[test]
    fn invalid_providers_are_skipped_and_unknown_checks_become_links() {
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
        // `f` has no usable page; the others keep their page as a plain link.
        assert_eq!(
            ps.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            ["a", "b", "c", "d", "e"]
        );
        assert!(ps.iter().all(|p| p.kind == Kind::Link && p.check.is_none()));
    }

    #[test]
    fn html_form_definitions() {
        let json = r#"{"schema":1,"providers":[
          {"id":"ok","category":"parking","name":"A","cities":["NI"],"kind":"html_form","url":"https://a.rs/",
           "check":{"url":"https://a.rs/provera","method":"post","form":{"tablica":"{plate}","grad":"{city}"},
                    "parser":"htmlMarkers","found":["Broj naloga"],"clean":["Nema izdatih"]}},
          {"id":"noclean","category":"parking","name":"B","kind":"html_form","url":"https://b.rs/",
           "check":{"url":"https://b.rs/x?t={plate}","parser":"htmlMarkers","found":["x"]}},
          {"id":"noplate","category":"parking","name":"C","kind":"html_form","url":"https://c.rs/",
           "check":{"url":"https://c.rs/x","method":"post","form":{"t":"BG1"},
                    "parser":"htmlMarkers","found":["x"],"clean":["y"]}},
          {"id":"jsonpost","category":"parking","name":"D","kind":"json_get","url":"https://d.rs/",
           "check":{"url":"https://d.rs/x?p={plate}","method":"post","parser":"jsonArray"}}
        ]}"#;
        let ps = parse_providers(json).unwrap();
        let auto: Vec<_> = ps
            .iter()
            .filter(|p| p.is_auto())
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(auto, ["ok"]);
        assert_eq!(ps.len(), 4);
        let req = ps[0].check_request("NIČ12AB").unwrap();
        assert_eq!(req.method, Method::Post);
        assert_eq!(req.full_url(), "https://a.rs/provera");
        assert_eq!(req.encoded_form(), "grad=NI&tablica=NI%C4%8C12AB");
        let info = providers_for("NI 1-AB", &ps);
        assert!(info.providers[0].auto);
        assert_eq!(info.providers[0].check_domain.as_deref(), Some("a.rs"));
        // GET with form fields: fields go to the query.
        let get = Request {
            method: Method::Get,
            url: "https://x.rs/p?a=1".into(),
            form: vec![("t".into(), "BG 1".into())],
            parser: Parser::HtmlMarkers,
            ..Default::default()
        };
        assert_eq!(get.full_url(), "https://x.rs/p?a=1&t=BG%201");
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
            .all(|p| p.auto || p.check_domain.is_none()));
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

    #[test]
    fn parkingns_order_fields() {
        // Field names used by the ParkingNS portal (proveraPPK / proveraPPKNP pages).
        let body = r#"[{"idfirme": 5, "nazivGrada": "Novi Pazar", "brojNaloga": "123/2026",
            "regOznaka": "NP123AB", "statusNaloga": 1, "nazivStatusa": "Neplaćen",
            "zaUplatu": 1500}]"#;
        let r = parse_json_array(Some("application/json"), body);
        assert_eq!(r.status, CheckStatus::Found);
        let it = &r.items[0];
        assert_eq!(it.amount.as_deref(), Some("1500"));
        assert_eq!(it.location.as_deref(), Some("Novi Pazar"));
        assert_eq!(it.description.as_deref(), Some("Neplaćen"));
        assert_eq!(it.fields["brojNaloga"], "123/2026");
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
        let req = ns().check_request("NS1AB").unwrap();
        let s = |f| interpret(&req, f).status;
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
        let run = |req: &Request| {
            calls.set(calls.get() + 1);
            assert_eq!(
                req.full_url(),
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
        let down = |r: &Request| interpret(r, Fetched::Network("dns".into()));
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

    // Synthetic answer pages in the shape of typical municipal eDPK forms (no real portal
    // page could be recorded from the development environment; see docs/07 §12).
    const HTML_FOUND: &str = r#"<html><head><style>td{color:red}</style>
      <script>var msg = "Nema izdatih karata";</script></head><body>
      <h2>Rezultat pretrage za NI123AB</h2>
      <table class="t"><thead><tr><th>Broj naloga</th><th>Datum</th><th>Iznos</th><th>Lokacija</th></tr></thead>
      <tbody><tr><td>EDPK-2026-0042</td><td>12.10.2026. 09:14</td><td>1.800,00&nbsp;RSD</td><td>Obrenovićeva &amp; Vožda Karađorđa</td></tr>
      <tr><td>EDPK-2026-0051</td><td>13.10.2026.</td><td>900,00 RSD</td><td>Zona 2</td></tr></tbody></table>
      </body></html>"#;
    const HTML_CLEAN_CYR: &str = r#"<html><body><div class="alert">Нема издатих карата за возило НИ123АБ.</div>
      <!-- Broj naloga --></body></html>"#;

    fn html_req() -> Request {
        Request {
            method: Method::Post,
            url: "https://a.rs/".into(),
            form: vec![],
            parser: Parser::HtmlMarkers,
            found: vec!["Broj naloga".into()],
            clean: vec!["Nema izdatih karata".into()],
            ..Default::default()
        }
    }

    #[test]
    fn html_markers_found_with_table() {
        let r = html_req();
        let p = parse_html_markers(HTML_FOUND, &r.found, &r.clean);
        assert_eq!(p.status, CheckStatus::Found, "scripts/styles are not text");
        assert_eq!(p.items.len(), 2);
        let a = &p.items[0];
        assert_eq!(a.date.as_deref(), Some("12.10.2026. 09:14"));
        assert_eq!(a.amount.as_deref(), Some("1.800,00 RSD"));
        assert_eq!(
            a.location.as_deref(),
            Some("Obrenovićeva & Vožda Karađorđa")
        );
        assert_eq!(
            a.fields.get("Broj naloga").map(String::as_str),
            Some("EDPK-2026-0042")
        );
    }

    #[test]
    fn html_markers_clean_in_cyrillic_and_comments_ignored() {
        let r = html_req();
        let p = parse_html_markers(HTML_CLEAN_CYR, &r.found, &r.clean);
        assert_eq!(p.status, CheckStatus::Clean);
        assert_eq!(
            normalize_text(" Нема  ИЗДАТИХ\n карата "),
            "nema izdatih karata"
        );
    }

    #[test]
    fn json_post_request_and_answers() {
        let json = r#"{"schema":1,"providers":[
          {"id":"kg","category":"parking","name":"KG","cities":["KG"],"kind":"json_post",
           "url":"https://kg.example.rs/",
           "check":{"url":"https://api.example.rs/API/kazne/","parser":"jsonArray",
                    "json":{"id":9009,"regOzn":"{plate}","grad":"{city}"},"emptyIsClean":true}},
          {"id":"nobody","category":"parking","name":"X","kind":"json_post","url":"https://x.rs/",
           "check":{"url":"https://x.rs/api","parser":"jsonArray"}}
        ]}"#;
        let ps = parse_providers(json).unwrap();
        assert_eq!(ps[0].kind, Kind::JsonPost);
        // Without a body (and so without the plate) it is only a link.
        assert_eq!(ps[1].kind, Kind::Link);
        let req = ps[0].check_request("KG12ČĆ").unwrap();
        assert_eq!(req.full_url(), "https://api.example.rs/API/kazne/");
        assert_eq!(
            req.json,
            Some(serde_json::json!({"id": 9009, "regOzn": "KG12ČĆ", "grad": "KG"}))
        );
        let body = |b: &str| Fetched::Body {
            content_type: Some("application/json".into()),
            body: b.into(),
        };
        // Empty answer (or an empty JSON string): no tickets, but only when the definition says so.
        assert_eq!(interpret(&req, body("")).status, CheckStatus::Clean);
        assert_eq!(interpret(&req, body("\"\"")).status, CheckStatus::Clean);
        let strict = Request {
            empty_is_clean: false,
            ..req.clone()
        };
        assert_eq!(interpret(&strict, body("")).status, CheckStatus::ParseError);
        // The answer JSON encoded once more as a string.
        let p = interpret(
            &req,
            body(r#""{\"table\":[{\"datum\":\"01.10.2026.\",\"iznos\":\"1000\"}]}""#),
        );
        assert_eq!(p.status, CheckStatus::Found);
        assert_eq!(p.items.len(), 1);
        assert_eq!(
            interpret(&req, body(r#""{\"table\":[]}""#)).status,
            CheckStatus::Clean
        );
        // A row that only reports an error is neither tickets nor "clean".
        assert_eq!(
            interpret(
                &req,
                body(r#""{\"table\":[{\"response\":\"ERROR_MESSAGE\"}]}""#)
            )
            .status,
            CheckStatus::ParseError
        );
    }

    #[test]
    fn html_markers_inside_json() {
        // Shape of the Niš `checkmydk` answer: BOM, JSON, HTML with \u-escaped Cyrillic.
        let found = vec!["Broj dnevne karte".to_string()];
        let clean = vec!["ne postoji dugovanje".to_string()];
        let body = "\u{feff}{\"success\":true,\"message\":null,\"data\":\"<div>\\u0437\\u0430 \\u043d\\u0430\\u0432\\u0435\\u0434\\u0435\\u043d\\u043e \\u0432\\u043e\\u0437\\u0438\\u043b\\u043e \\u043d\\u0435 \\u043f\\u043e\\u0441\\u0442\\u043e\\u0458\\u0438 \\u0434\\u0443\\u0433\\u043e\\u0432\\u0430\\u045a\\u0435<br \\/><\\/div>\"}";
        assert_eq!(
            parse_html_markers(body, &found, &clean).status,
            CheckStatus::Clean
        );
        let body = r#"{"data":"<table><tr><th>Broj dnevne karte</th><th>Iznos</th></tr><tr><td>DK-1</td><td>1.800,00</td></tr></table>"}"#;
        let p = parse_html_markers(body, &found, &clean);
        assert_eq!(p.status, CheckStatus::Found);
        assert_eq!(p.items.len(), 1);
        // `data: null` (no fragment): neither marker, never "clean".
        let body = r#"{"success":true,"data":null}"#;
        assert_eq!(
            parse_html_markers(body, &found, &clean).status,
            CheckStatus::ParseError
        );
    }

    #[test]
    fn html_markers_never_guess_clean() {
        let r = html_req();
        // Neither marker (e.g. the form was shown again): not "clean".
        let p = parse_html_markers("<form><input name=tablica></form>", &r.found, &r.clean);
        assert_eq!(p.status, CheckStatus::ParseError);
        // Captcha widget: manual.
        let p = parse_html_markers(
            r#"<div class="cf-turnstile" data-sitekey="x"></div>Nema izdatih karata"#,
            &r.found,
            &r.clean,
        );
        assert_eq!(p.status, CheckStatus::Manual);
        // Found marker without a table: one item, still "found".
        let p = parse_html_markers("<p>Broj naloga: 77</p>", &r.found, &r.clean);
        assert_eq!(p.status, CheckStatus::Found);
        assert_eq!(p.items.len(), 1);
        // Both markers: found wins.
        let p = parse_html_markers(
            "<p>Nema izdatih karata starijih od 60 dana.</p><table><tr><td>Broj naloga 1</td></tr></table>",
            &r.found,
            &r.clean,
        );
        assert_eq!(p.status, CheckStatus::Found);
        // Through `interpret`, HTTP errors behave as for JSON.
        assert_eq!(
            interpret(&r, Fetched::Status(429)).status,
            CheckStatus::Manual
        );
        assert_eq!(
            interpret(
                &r,
                Fetched::Body {
                    content_type: Some("text/html".into()),
                    body: HTML_FOUND.into()
                }
            )
            .status,
            CheckStatus::Found
        );
    }

    #[test]
    fn html_text_helpers() {
        assert_eq!(
            html_text("a<b>b</b>&#268;&#x0160;&amp;&unknown;&")
                .split_whitespace()
                .collect::<Vec<_>>(),
            ["a", "b", "ČŠ&&unknown;&"]
        );
        assert_eq!(html_text("x<script>if(a<b){}</script>y").trim(), "x y");
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
