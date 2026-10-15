//! Smartbox emulation: the local websocket server used by the ePorezi portal
//! (eporezi.purs.gov.rs) to sign tax returns with a token.
//!
//! Port of bas-celik (MIT, (c) Nikola Ubavić) `internal/smartbox`.
//!
//! Protocol: on connect the server sends `ON_OPEN_EVENT`; then the portal sends JSON messages
//! `{"operation": ..., "input": {...}}` (`GET_INFO`, `GET_PROVIDERS`, `GET_TERMINALS`,
//! `GET_CERTIFICATES`, `GET_SIGNED_XML`) and gets `{"operation", "status", "payload"}` replies.
//! The PIN is sent by the portal with `GET_CERTIFICATES` / `GET_SIGNED_XML`, so the server needs
//! no PIN entry UI. Any error closes the connection (like Bas Celik).

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde::Serialize;
use serde_json::{Map, Value};
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::Message as WsMessage;

use crate::pkcs11::{CardVendor, ModulePath, Modules, NamedCert, Pkcs11Session, Slot};
use crate::xmlsign::{atv_string, parse_cert, sign_xml, DigestSigner};
use crate::{hex_decode, hex_encode, Result, SignError};

/// Address the server binds to.
pub const BIND_HOST: &str = "127.0.0.1";
/// Ports tried in order (the portal probes the same list).
pub const DEFAULT_PORTS: [u16; 3] = [17165, 20806, 65097];
/// Origins (hosts) allowed to connect, besides same-host requests and requests without Origin.
pub const ALLOWED_ORIGINS: [&str; 2] = ["eporezi.purs.gov.rs", "test.purs.gov.rs"];

const ON_OPEN_EVENT: &str = "ON_OPEN_EVENT";
const OPERATION_GET_INFO: &str = "GET_INFO";
const OPERATION_GET_PROVIDERS: &str = "GET_PROVIDERS";
const OPERATION_GET_TERMINALS: &str = "GET_TERMINALS";
const OPERATION_GET_CERTIFICATES: &str = "GET_CERTIFICATES";
const OPERATION_GET_SIGNED_XML: &str = "GET_SIGNED_XML";

/// The token operations the Smartbox server needs (Go `PkcsModuleSession`); implemented by
/// [`Pkcs11Session`] and by mocks in tests.
pub trait TokenSession: Send {
    fn list_slots(&mut self) -> Result<Vec<Slot>>;
    fn open_session_and_login(&mut self, pin: &str, terminal_id: i64) -> Result<()>;
    fn certificates(&mut self) -> Result<Vec<NamedCert>>;
    fn sign_digest(&mut self, cert_id: &[u8], sha256_digest: &[u8]) -> Result<Vec<u8>>;
}

impl TokenSession for Pkcs11Session {
    fn list_slots(&mut self) -> Result<Vec<Slot>> {
        Pkcs11Session::list_slots(self)
    }

    fn open_session_and_login(&mut self, pin: &str, terminal_id: i64) -> Result<()> {
        if terminal_id < 0 {
            return Err(SignError::Pkcs11(format!("invalid slot id: {terminal_id}")));
        }
        self.login(terminal_id as u64, pin)
    }

    fn certificates(&mut self) -> Result<Vec<NamedCert>> {
        Pkcs11Session::certificates(self)
    }

    fn sign_digest(&mut self, cert_id: &[u8], sha256_digest: &[u8]) -> Result<Vec<u8>> {
        Pkcs11Session::sign_digest(self, cert_id, sha256_digest)
    }
}

pub type SharedTokenSession = Arc<Mutex<dyn TokenSession>>;
/// Creates a token session for a vendor (called on `GET_TERMINALS`).
pub type SessionProvider = Arc<dyn Fn(CardVendor) -> Result<SharedTokenSession> + Send + Sync>;

fn lock(m: &SharedTokenSession) -> std::sync::MutexGuard<'_, dyn TokenSession + 'static> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------------------
// Protocol types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Response_<P: Serialize> {
    operation: &'static str,
    status: i64,
    payload: P,
}

/// Serialises a response like Go's `json.Encoder` (with a trailing newline).
fn response<P: Serialize>(operation: &'static str, payload: P) -> Result<String> {
    let mut s = serde_json::to_string(&Response_ {
        operation,
        status: 0,
        payload,
    })
    .map_err(|e| SignError::Server(e.to_string()))?;
    s.push('\n');
    Ok(s)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OnOpenPayload {
    app_name: &'static str,
    app_version: &'static str,
    app_build: i64,
    os_name: &'static str,
    os_version: &'static str,
    os_arch: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GetInfoPayload {
    terminal_id: i64,
    provider_id: i64,
    certificate_id: String,
}

#[derive(Serialize)]
struct Provider {
    id: i64,
    name: &'static str,
}

#[derive(Serialize)]
struct GetProvidersPayload {
    providers: Vec<Provider>,
}

#[derive(Serialize)]
struct Terminal {
    id: String,
    name: String,
}

#[derive(Serialize)]
struct GetTerminalsPayload {
    terminals: Vec<Terminal>,
}

/// Certificate as presented to the portal: `alias` is the hex `CKA_ID`, `name` the subject CN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CertificateAlias {
    pub alias: String,
    pub name: String,
}

#[derive(Serialize)]
struct GetCertificatesPayload {
    certificates: Vec<CertificateAlias>,
}

#[derive(Serialize)]
struct GetSignedXmlPayload {
    xml: String,
}

fn go_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        os => os,
    }
}

fn go_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "x86" => "386",
        arch => arch,
    }
}

fn on_open() -> Result<String> {
    response(
        ON_OPEN_EVENT,
        OnOpenPayload {
            app_name: "SmartBox",
            app_version: "2.0.0",
            app_build: 1,
            os_name: go_os(),
            os_version: "1",
            os_arch: go_arch(),
        },
    )
}

// --- lenient JSON access mimicking Go's encoding/json --------------------------------------

fn server_err(msg: impl Into<String>) -> SignError {
    SignError::Server(msg.into())
}

/// Case-insensitive field lookup (Go matches struct fields case-insensitively).
fn get_ci<'a>(obj: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    obj.get(key).or_else(|| {
        obj.iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
            .next_back()
    })
}

static EMPTY: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();

fn empty_map() -> &'static Map<String, Value> {
    EMPTY.get_or_init(Map::new)
}

/// A struct-typed value: missing / null => all defaults, object => fields, else error.
fn as_struct<'a>(v: Option<&'a Value>, what: &str) -> Result<&'a Map<String, Value>> {
    match v {
        None | Some(Value::Null) => Ok(empty_map()),
        Some(Value::Object(o)) => Ok(o),
        Some(other) => Err(server_err(format!(
            "json: cannot unmarshal {other} into {what}"
        ))),
    }
}

fn input_of(msg: &Value) -> Result<&Map<String, Value>> {
    let obj = msg
        .as_object()
        .ok_or_else(|| server_err("json: message is not an object"))?;
    as_struct(get_ci(obj, "input"), "input")
}

fn str_field(obj: &Map<String, Value>, key: &str) -> Result<String> {
    match get_ci(obj, key) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(server_err(format!(
            "json: cannot unmarshal {other} into string field {key}"
        ))),
    }
}

fn int_field(obj: &Map<String, Value>, key: &str) -> Result<i64> {
    match get_ci(obj, key) {
        None | Some(Value::Null) => Ok(0),
        Some(v) => v
            .as_i64()
            .ok_or_else(|| server_err(format!("json: cannot unmarshal {v} into int field {key}"))),
    }
}

/// Go `stringOrInt`: accepts `3` and `"3"`.
fn string_or_int(v: Option<&Value>) -> Result<i64> {
    match v {
        None | Some(Value::Null) => Ok(0),
        Some(Value::Number(n)) => n
            .as_i64()
            .ok_or_else(|| server_err(format!("invalid value for stringOrInt: {n}"))),
        Some(Value::String(s)) => s
            .parse::<i64>()
            .map_err(|_| server_err(format!("invalid value for stringOrInt: {s:?}"))),
        Some(other) => Err(server_err(format!(
            "invalid value for stringOrInt: {other}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Certificates
// ---------------------------------------------------------------------------

/// Certificates currently valid; for MUP tokens only the ones with the non-repudiation
/// (content commitment) key usage (Go `GetValidCertificates`).
pub fn get_valid_certificates(certs: &[NamedCert], vendor: CardVendor) -> Vec<NamedCert> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    certs
        .iter()
        .filter(|nc| {
            let Ok(cert) = parse_cert(&nc.der) else {
                return false;
            };
            let validity = cert.validity();
            if now < validity.not_before.timestamp() || now > validity.not_after.timestamp() {
                return false;
            }
            if vendor == CardVendor::Mup {
                let non_repudiation = matches!(
                    cert.key_usage(),
                    Ok(Some(ku)) if ku.value.non_repudiation()
                );
                if !non_repudiation {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

fn subject_common_name(der: &[u8]) -> String {
    parse_cert(der)
        .ok()
        .and_then(|c| c.subject().iter_common_name().filter_map(atv_string).last())
        .unwrap_or_default()
}

/// Go `GetCertificateAliases`.
pub fn get_certificate_aliases(certs: &[NamedCert]) -> Vec<CertificateAlias> {
    certs
        .iter()
        .map(|c| CertificateAlias {
            alias: hex_encode(&c.id),
            name: subject_common_name(&c.der),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

/// State of one portal session (`sbSession`).
#[derive(Clone, Default)]
struct SmartboxSession {
    #[allow(dead_code)]
    id: String,
    module: Option<SharedTokenSession>,
    terminal_id: i64,
    vendor: i64,
    certificate_id: Option<Vec<u8>>,
}

impl SmartboxSession {
    /// Selected certificate (DER), if present on the token.
    fn certificate(&self) -> Option<NamedCert> {
        let module = self.module.as_ref()?;
        let id = self.certificate_id.as_deref().unwrap_or_default();
        let certs = lock(module).certificates().ok()?;
        certs.into_iter().rfind(|c| c.id == id)
    }
}

impl DigestSigner for SmartboxSession {
    fn sign_sha256_digest(&self, digest: &[u8]) -> Result<Vec<u8>> {
        let module = self
            .module
            .as_ref()
            .ok_or_else(|| server_err("nil module"))?;
        let id = self
            .certificate_id
            .as_ref()
            .ok_or_else(|| server_err("certificate ID not set"))?;
        if digest.len() != 32 {
            return Err(server_err("only sha256 supported"));
        }
        lock(module).sign_digest(id, digest)
    }
}

struct ServerState {
    sessions: Mutex<HashMap<String, SmartboxSession>>,
    loaded_vendors: Vec<CardVendor>,
    provider: SessionProvider,
}

impl ServerState {
    fn new(loaded_vendors: Vec<CardVendor>, provider: SessionProvider) -> Self {
        ServerState {
            sessions: Mutex::new(HashMap::new()),
            loaded_vendors,
            provider,
        }
    }

    fn load_session(&self, id: &str) -> Option<SmartboxSession> {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
    }

    fn store_session(&self, id: &str, session: SmartboxSession) {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.to_string(), session);
    }

    /// Handles one websocket message (Go `onMessage`). Returns the frame to send back
    /// (`None`: nothing; `Some("")`: an empty frame, which Go sends when a handler fails) and
    /// whether the connection should continue.
    fn on_message(&self, session_id: &mut String, data: &str) -> (Option<String>, Result<()>) {
        let msg: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(e) => return (None, Err(server_err(format!("unmarshaling data: {e}")))),
        };
        let operation = match msg.as_object().map(|o| get_ci(o, "operation")) {
            Some(None | Some(Value::Null)) => String::new(),
            Some(Some(Value::String(s))) => s.clone(),
            _ => {
                return (
                    None,
                    Err(server_err("unmarshaling data: invalid operation")),
                )
            }
        };

        let found = self.load_session(session_id);
        if found.is_none() && operation != OPERATION_GET_INFO {
            return (
                Some(String::new()),
                Err(server_err(format!(
                    "session {session_id} not found, operation {operation}"
                ))),
            );
        }
        let mut session = found.unwrap_or_default();

        let result = match operation.as_str() {
            OPERATION_GET_INFO => self.handle_get_info(session_id, &msg),
            OPERATION_GET_PROVIDERS => self.handle_get_providers(&msg),
            OPERATION_GET_TERMINALS => self.handle_get_terminals(&mut session, &msg),
            OPERATION_GET_CERTIFICATES => self.handle_get_certificates(&mut session, &msg),
            OPERATION_GET_SIGNED_XML => self.handle_get_signed_xml(&mut session, &msg),
            other => Err(server_err(format!(
                "unknown operation {other} in session {session_id}"
            ))),
        };

        self.store_session(session_id, session);

        match result {
            Ok(rsp) => (Some(rsp), Ok(())),
            Err(e) => (Some(String::new()), Err(e)),
        }
    }

    fn handle_get_info(&self, session_id: &mut String, msg: &Value) -> Result<String> {
        let input = input_of(msg)?;
        let sb_session = str_field(input, "sbSession")?;
        str_field(input, "language")?;
        str_field(input, "host")?;
        if sb_session.is_empty() {
            return Err(server_err("invalid session id"));
        }

        let session = match self.load_session(&sb_session) {
            Some(s) => s,
            None => {
                let s = SmartboxSession {
                    id: sb_session.clone(),
                    ..Default::default()
                };
                self.store_session(&sb_session, s.clone());
                s
            }
        };
        *session_id = sb_session;

        response(
            OPERATION_GET_INFO,
            GetInfoPayload {
                terminal_id: session.terminal_id,
                provider_id: session.vendor,
                certificate_id: hex_encode(session.certificate_id.as_deref().unwrap_or_default()),
            },
        )
    }

    fn handle_get_providers(&self, msg: &Value) -> Result<String> {
        input_of(msg)?;
        let providers = self
            .loaded_vendors
            .iter()
            .map(|v| Provider {
                id: v.id(),
                name: v.name(),
            })
            .collect();
        response(OPERATION_GET_PROVIDERS, GetProvidersPayload { providers })
    }

    fn handle_get_terminals(&self, session: &mut SmartboxSession, msg: &Value) -> Result<String> {
        let input = input_of(msg)?;
        let provider_id = string_or_int(get_ci(input, "providerId"))?;
        let vendor =
            CardVendor::from_id(provider_id).ok_or_else(|| server_err("invalid provider id"))?;

        let module = (self.provider)(vendor)?;
        session.vendor = provider_id;
        session.module = Some(module.clone());

        let slots = lock(&module).list_slots()?;
        let terminals = slots
            .into_iter()
            .map(|s| Terminal {
                id: s.id.to_string(),
                name: s.description,
            })
            .collect();
        response(OPERATION_GET_TERMINALS, GetTerminalsPayload { terminals })
    }

    fn handle_get_certificates(
        &self,
        session: &mut SmartboxSession,
        msg: &Value,
    ) -> Result<String> {
        let input = input_of(msg)?;
        let terminal_id = int_field(input, "terminalId")?;
        let pin = str_field(input, "pin")?;
        let module = session
            .module
            .clone()
            .ok_or_else(|| server_err("pkcs11 module not loaded"))?;

        let certs = {
            let mut m = lock(&module);
            m.open_session_and_login(&pin, terminal_id)?;
            m.certificates()?
        };
        let vendor = CardVendor::from_id(session.vendor).unwrap_or(CardVendor::Halcom);
        response(
            OPERATION_GET_CERTIFICATES,
            GetCertificatesPayload {
                certificates: get_certificate_aliases(&get_valid_certificates(&certs, vendor)),
            },
        )
    }

    fn handle_get_signed_xml(&self, session: &mut SmartboxSession, msg: &Value) -> Result<String> {
        let input =
            input_of(msg).map_err(|e| server_err(format!("unmarshaling sign request: {e}")))?;
        let certificate = as_struct(get_ci(input, "certificate"), "certificate")?;
        let alias = str_field(certificate, "alias")?;
        str_field(certificate, "name")?;
        str_field(input, "pin")?;
        let xml_b64 = str_field(input, "xml")?;

        if session.module.is_none() {
            return Err(server_err("pkcs11 module not loaded"));
        }

        session.certificate_id = Some(
            hex_decode(&alias)
                .map_err(|e| server_err(format!("decoding certificate alias: {e}")))?,
        );

        let cert = session.certificate().ok_or_else(|| {
            server_err(format!(
                "certificate {} not found",
                hex_encode(session.certificate_id.as_deref().unwrap_or_default())
            ))
        })?;

        let xml = crate::xmlsign::b64_decode_lenient(&xml_b64)
            .map_err(|e| server_err(format!("decoding base64 payload: {e}")))?;

        let signed = sign_xml(&xml, &*session, &cert.der)
            .map_err(|e| server_err(format!("signing xml: {e}")))?;

        response(
            OPERATION_GET_SIGNED_XML,
            GetSignedXmlPayload {
                xml: base64::engine::general_purpose::STANDARD.encode(signed.as_bytes()),
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Websocket server
// ---------------------------------------------------------------------------

/// Host part of an Origin header value (`scheme://host[:port]/...`).
fn origin_host(origin: &str) -> Option<&str> {
    let (_, rest) = origin.split_once("://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = &rest[..end];
    Some(host.rsplit_once('@').map(|(_, h)| h).unwrap_or(host))
}

/// Origin check of coder/websocket `Accept` with Bas Celik's `OriginPatterns`.
fn origin_allowed(origin: Option<&str>, request_host: Option<&str>) -> bool {
    let Some(origin) = origin else {
        return true;
    };
    let Some(host) = origin_host(origin) else {
        return false;
    };
    if let Some(rh) = request_host {
        if rh.eq_ignore_ascii_case(host) {
            return true;
        }
    }
    !host.is_empty() && ALLOWED_ORIGINS.iter().any(|p| p.eq_ignore_ascii_case(host))
}

#[allow(clippy::result_large_err)]
fn handle_connection(state: Arc<ServerState>, stream: TcpStream) {
    let peer = stream.peer_addr().ok();
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let callback =
        |req: &Request, resp: Response| -> std::result::Result<Response, ErrorResponse> {
            let header = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok());
            if origin_allowed(header("origin"), header("host")) {
                Ok(resp)
            } else {
                log::warn!("smartbox: rejected origin {:?}", header("origin"));
                let mut rsp = ErrorResponse::new(Some("origin not allowed".to_string()));
                *rsp.status_mut() = tungstenite::http::StatusCode::FORBIDDEN;
                Err(rsp)
            }
        };
    let mut ws = match tungstenite::accept_hdr(stream, callback) {
        Ok(ws) => ws,
        Err(e) => {
            log::error!("smartbox: websocket handshake with {peer:?}: {e}");
            return;
        }
    };
    let _ = ws.get_ref().set_read_timeout(None);

    let run = |ws: &mut tungstenite::WebSocket<TcpStream>| -> Result<()> {
        let open = on_open()?;
        ws.send(WsMessage::text(open))
            .map_err(|e| server_err(e.to_string()))?;
        let mut session_id = String::new();
        loop {
            let data = match ws.read() {
                Ok(WsMessage::Text(t)) => t.as_str().to_string(),
                Ok(WsMessage::Binary(b)) => String::from_utf8_lossy(&b).into_owned(),
                Ok(WsMessage::Close(_)) => return Ok(()),
                Ok(_) => continue,
                Err(e) => {
                    return Err(server_err(format!(
                        "reading message in session {session_id}: {e}"
                    )))
                }
            };
            let (frame, result) = state.on_message(&mut session_id, &data);
            if let Some(frame) = frame {
                ws.send(WsMessage::text(frame))
                    .map_err(|e| server_err(format!("writing response: {e}")))?;
            }
            result?;
        }
    };
    if let Err(e) = run(&mut ws) {
        log::error!("smartbox: {e}");
    }
    let _ = ws.get_ref().shutdown(Shutdown::Both);
}

/// Running Smartbox emulation server (used by the ePorezi portal).
///
/// Dropping the server (or calling [`SmartboxServer::stop`]) stops it and closes all
/// connections.
pub struct SmartboxServer {
    /// `127.0.0.1:<port>`; the portal connects to `ws://<address>`.
    pub address: String,
    /// Vendors whose PKCS#11 modules were loaded.
    pub vendors: Vec<CardVendor>,
    port: u16,
    shutdown: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
    connections: Arc<Mutex<HashMap<u64, TcpStream>>>,
    _modules: Option<Arc<Modules>>,
}

impl std::fmt::Debug for SmartboxServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmartboxServer")
            .field("address", &self.address)
            .field("vendors", &self.vendors)
            .finish()
    }
}

impl SmartboxServer {
    /// Loads the PKCS#11 modules (see [`Modules::load`]) and starts the websocket server on the
    /// first free port of [`DEFAULT_PORTS`] in a background thread. Returns once listening.
    /// Errors: [`SignError::NoModules`] when no module could be loaded, [`SignError::Server`]
    /// when no port is available.
    pub fn start(paths: &[ModulePath]) -> Result<Self> {
        let modules = Arc::new(Modules::load(paths)?);
        let vendors = modules.vendors();
        let m = modules.clone();
        let provider: SessionProvider = Arc::new(move |vendor| {
            let session = m.session(vendor)?;
            Ok(Arc::new(Mutex::new(session)) as SharedTokenSession)
        });
        let mut server = Self::start_with_provider(vendors, provider, &DEFAULT_PORTS)?;
        server._modules = Some(modules);
        Ok(server)
    }

    /// Starts the server with a custom token session provider (e.g. a mock) on the first
    /// bindable port of `ports` (`0` = any free port).
    pub fn start_with_provider(
        vendors: Vec<CardVendor>,
        provider: SessionProvider,
        ports: &[u16],
    ) -> Result<Self> {
        let listener = ports
            .iter()
            .find_map(|p| TcpListener::bind((BIND_HOST, *p)).ok())
            .ok_or_else(|| server_err("no valid port available"))?;
        let port = listener
            .local_addr()
            .map_err(|e| server_err(e.to_string()))?
            .port();
        listener
            .set_nonblocking(true)
            .map_err(|e| server_err(e.to_string()))?;

        let state = Arc::new(ServerState::new(vendors.clone(), provider));
        let shutdown = Arc::new(AtomicBool::new(false));
        let connections: Arc<Mutex<HashMap<u64, TcpStream>>> = Arc::default();

        let accept_thread = {
            let shutdown = shutdown.clone();
            let connections = connections.clone();
            std::thread::Builder::new()
                .name("smartbox-accept".into())
                .spawn(move || {
                    let next_id = AtomicU64::new(0);
                    while !shutdown.load(Ordering::SeqCst) {
                        match listener.accept() {
                            Ok((stream, _)) => {
                                let _ = stream.set_nonblocking(false);
                                let id = next_id.fetch_add(1, Ordering::SeqCst);
                                if let Ok(clone) = stream.try_clone() {
                                    connections
                                        .lock()
                                        .unwrap_or_else(|e| e.into_inner())
                                        .insert(id, clone);
                                }
                                let state = state.clone();
                                let connections = connections.clone();
                                let spawned = std::thread::Builder::new()
                                    .name("smartbox-conn".into())
                                    .spawn(move || {
                                        handle_connection(state, stream);
                                        connections
                                            .lock()
                                            .unwrap_or_else(|e| e.into_inner())
                                            .remove(&id);
                                    });
                                if let Err(e) = spawned {
                                    log::error!("smartbox: spawning connection thread: {e}");
                                }
                            }
                            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                                std::thread::sleep(Duration::from_millis(25));
                            }
                            Err(e) => {
                                log::error!("smartbox: accept: {e}");
                                std::thread::sleep(Duration::from_millis(25));
                            }
                        }
                    }
                })
                .map_err(|e| server_err(e.to_string()))?
        };

        let address = format!("{BIND_HOST}:{port}");
        log::info!("Smartbox server listening on ws://{address}");
        Ok(SmartboxServer {
            address,
            vendors,
            port,
            shutdown,
            accept_thread: Some(accept_thread),
            connections,
            _modules: None,
        })
    }

    /// Port the server listens on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Display names of the loaded vendors (Go `StartServer`'s second return value).
    pub fn vendor_names(&self) -> Vec<&'static str> {
        self.vendors.iter().map(|v| v.name()).collect()
    }

    pub fn is_running(&self) -> bool {
        !self.shutdown.load(Ordering::SeqCst)
    }

    /// Stops the server and closes all connections.
    pub fn stop(mut self) {
        self.shutdown_now();
    }

    fn shutdown_now(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(t) = self.accept_thread.take() {
            let _ = t.join();
        }
        for (_, s) in self
            .connections
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain()
        {
            let _ = s.shutdown(Shutdown::Both);
        }
    }
}

impl Drop for SmartboxServer {
    fn drop(&mut self) {
        self.shutdown_now();
    }
}

#[cfg(test)]
mod tests;
