//! Tests of the Smartbox protocol handlers and server.

use std::net::TcpStream;
use std::time::Duration;

use base64::Engine;
use serde_json::{json, Value};
use tungstenite::client::IntoClientRequest;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::WebSocket;

use super::*;
use crate::xmlsign::{verify_digest_signature, verify_signed_xml, SoftwareSigner};

// --- mock -----------------------------------------------------------------------------------

type SignFn = Box<dyn Fn(&[u8], &[u8]) -> Result<Vec<u8>> + Send>;

#[derive(Default)]
struct MockToken {
    slots: Vec<Slot>,
    list_slots_err: Option<String>,
    login_pin: String,
    login_slot: i64,
    login_err: Option<String>,
    certs: Vec<NamedCert>,
    get_certs_err: Option<String>,
    sign: Option<SignFn>,
}

impl TokenSession for MockToken {
    fn list_slots(&mut self) -> Result<Vec<Slot>> {
        match &self.list_slots_err {
            Some(e) => Err(SignError::Pkcs11(e.clone())),
            None => Ok(self.slots.clone()),
        }
    }

    fn open_session_and_login(&mut self, pin: &str, terminal_id: i64) -> Result<()> {
        self.login_pin = pin.to_string();
        self.login_slot = terminal_id;
        match &self.login_err {
            Some(e) => Err(SignError::Pkcs11(e.clone())),
            None => Ok(()),
        }
    }

    fn certificates(&mut self) -> Result<Vec<NamedCert>> {
        match &self.get_certs_err {
            Some(e) => Err(SignError::Pkcs11(e.clone())),
            None => Ok(self.certs.clone()),
        }
    }

    fn sign_digest(&mut self, cert_id: &[u8], digest: &[u8]) -> Result<Vec<u8>> {
        match &self.sign {
            Some(f) => f(cert_id, digest),
            None => Err(SignError::Other("sign not configured".into())),
        }
    }
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn software_signer() -> SoftwareSigner {
    SoftwareSigner::from_pkcs8_pem(&String::from_utf8(fixture("key.pem")).unwrap()).unwrap()
}

/// Valid until 2040, digitalSignature + contentCommitment, CN "Petar Petrović 12345678".
fn valid_cert(id: &[u8]) -> NamedCert {
    NamedCert::from_der(id.to_vec(), "", fixture("cert.der")).unwrap()
}

fn cert_variant(id: &[u8], name: &str) -> NamedCert {
    NamedCert::from_der(id.to_vec(), "", fixture(name)).unwrap()
}

fn signing_mock(certs: Vec<NamedCert>) -> MockToken {
    let signer = software_signer();
    MockToken {
        slots: vec![Slot {
            id: 0,
            description: "Virtual Reader".into(),
        }],
        certs,
        sign: Some(Box::new(move |_, digest| signer.sign_sha256_digest(digest))),
        ..Default::default()
    }
}

fn shared(mock: MockToken) -> Arc<Mutex<MockToken>> {
    Arc::new(Mutex::new(mock))
}

fn provider_of(mock: &Arc<Mutex<MockToken>>) -> SessionProvider {
    let m = mock.clone();
    Arc::new(move |_| Ok(m.clone() as SharedTokenSession))
}

fn new_test_state(mock: Option<&Arc<Mutex<MockToken>>>, vendors: &[CardVendor]) -> ServerState {
    let provider: SessionProvider = match mock {
        Some(m) => provider_of(m),
        None => Arc::new(|_| Err(SignError::Other("no mock".into()))),
    };
    ServerState::new(vendors.to_vec(), provider)
}

fn session_with(mock: &Arc<Mutex<MockToken>>, vendor: CardVendor) -> SmartboxSession {
    SmartboxSession {
        module: Some(mock.clone() as SharedTokenSession),
        vendor: vendor.id(),
        ..Default::default()
    }
}

fn parse(rsp: &str) -> Value {
    assert!(rsp.ends_with('\n'), "responses end with a newline");
    serde_json::from_str(rsp).unwrap()
}

// --- string or int --------------------------------------------------------------------------

#[test]
fn string_or_int_values() {
    assert_eq!(string_or_int(Some(&json!(42))).unwrap(), 42);
    assert_eq!(string_or_int(Some(&json!("7"))).unwrap(), 7);
    assert!(string_or_int(Some(&json!("abc"))).is_err());
    assert!(string_or_int(Some(&json!(true))).is_err());
    for (v, want) in [
        (json!({"id": 3}), 3),
        (json!({"id": "5"}), 5),
        (json!({"id": 0}), 0),
        (json!({"id": "0"}), 0),
    ] {
        assert_eq!(string_or_int(v.get("id")).unwrap(), want);
    }
    assert_eq!(string_or_int(None).unwrap(), 0);
}

// --- ON_OPEN_EVENT --------------------------------------------------------------------------

#[test]
fn on_open_payload() {
    let rsp = parse(&on_open().unwrap());
    assert_eq!(rsp["operation"], ON_OPEN_EVENT);
    assert_eq!(rsp["status"], 0);
    assert_eq!(rsp["payload"]["appName"], "SmartBox");
    assert_eq!(rsp["payload"]["appVersion"], "2.0.0");
    assert_eq!(rsp["payload"]["appBuild"], 1);
    assert_eq!(rsp["payload"]["osName"], protocol_os_name());
    assert_eq!(rsp["payload"]["osArch"], protocol_arch_name());
    assert_eq!(rsp["payload"]["osVersion"], "1");
}

// --- GET_INFO -------------------------------------------------------------------------------

#[test]
fn get_info_new_session() {
    let st = new_test_state(None, &[]);
    let msg = json!({"operation": "GET_INFO", "input": {
        "sbSession": "test-session-123", "language": "sr", "host": "eporezi.purs.gov.rs"}});
    let mut id = String::new();
    let rsp = parse(&st.handle_get_info(&mut id, &msg).unwrap());
    assert_eq!(id, "test-session-123");
    assert!(st.load_session("test-session-123").is_some());
    assert_eq!(rsp["operation"], OPERATION_GET_INFO);
}

#[test]
fn get_info_existing_session() {
    let st = new_test_state(None, &[]);
    st.store_session(
        "existing-session",
        SmartboxSession {
            id: "existing-session".into(),
            terminal_id: 2,
            certificate_id: Some(vec![0xab]),
            ..Default::default()
        },
    );
    let msg = json!({"operation": "GET_INFO", "input": {"sbSession": "existing-session"}});
    let mut id = String::new();
    let rsp = parse(&st.handle_get_info(&mut id, &msg).unwrap());
    assert_eq!(rsp["payload"]["terminalId"], 2);
    assert_eq!(rsp["payload"]["certificateId"], "ab");
}

#[test]
fn get_info_empty_session() {
    let st = new_test_state(None, &[]);
    let msg = json!({"operation": "GET_INFO", "input": {"sbSession": ""}});
    assert!(st.handle_get_info(&mut String::new(), &msg).is_err());
}

#[test]
fn get_info_accepts_capitalized_field_names() {
    let st = new_test_state(None, &[]);
    let msg = json!({"operation": "GET_INFO", "Input": {"SbSession": "x"}});
    let mut id = String::new();
    st.handle_get_info(&mut id, &msg).unwrap();
    assert_eq!(id, "x");
}

// --- GET_PROVIDERS --------------------------------------------------------------------------

#[test]
fn get_providers() {
    let st = new_test_state(None, &[CardVendor::Posta, CardVendor::Mup]);
    let rsp = parse(
        &st.handle_get_providers(&json!({"operation": "GET_PROVIDERS"}))
            .unwrap(),
    );
    assert_eq!(rsp["operation"], OPERATION_GET_PROVIDERS);
    let providers = rsp["payload"]["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 2);
    assert_eq!(providers[0]["id"], CardVendor::Posta.id());
    assert_eq!(providers[0]["name"], "Sertifikaciono telo Pošte");
    assert_eq!(providers[1]["id"], CardVendor::Mup.id());
}

#[test]
fn get_providers_empty() {
    let st = new_test_state(None, &[]);
    let rsp = parse(
        &st.handle_get_providers(&json!({"operation": "GET_PROVIDERS"}))
            .unwrap(),
    );
    assert_eq!(rsp["payload"]["providers"].as_array().unwrap().len(), 0);
}

// --- GET_TERMINALS --------------------------------------------------------------------------

fn terminals_msg(provider: Value) -> Value {
    json!({"operation": "GET_TERMINALS", "input": {"providerId": provider}})
}

#[test]
fn get_terminals() {
    let mock = shared(MockToken {
        slots: vec![
            Slot {
                id: 0,
                description: "Reader 0".into(),
            },
            Slot {
                id: 1,
                description: "Reader 1".into(),
            },
        ],
        ..Default::default()
    });
    let st = new_test_state(Some(&mock), &[CardVendor::Posta]);
    let mut session = SmartboxSession::default();
    let rsp = parse(
        &st.handle_get_terminals(&mut session, &terminals_msg(json!(CardVendor::Posta.id())))
            .unwrap(),
    );
    assert_eq!(rsp["operation"], OPERATION_GET_TERMINALS);
    let terminals = rsp["payload"]["terminals"].as_array().unwrap();
    assert_eq!(terminals.len(), 2);
    assert_eq!(terminals[0]["id"], "0");
    assert_eq!(terminals[0]["name"], "Reader 0");
    assert_eq!(session.vendor, CardVendor::Posta.id());
    assert!(session.module.is_some());

    // provider id as a string
    let mut session = SmartboxSession::default();
    st.handle_get_terminals(&mut session, &terminals_msg(json!("1")))
        .unwrap();
    assert_eq!(session.vendor, 1);
}

#[test]
fn get_terminals_invalid_provider() {
    let mock = shared(MockToken::default());
    let st = new_test_state(Some(&mock), &[CardVendor::Posta]);
    let mut session = SmartboxSession::default();
    assert!(st
        .handle_get_terminals(&mut session, &terminals_msg(json!(99)))
        .is_err());
    assert!(st
        .handle_get_terminals(&mut session, &terminals_msg(json!(-1)))
        .is_err());
}

#[test]
fn get_terminals_session_provider_error() {
    let st = new_test_state(None, &[CardVendor::Posta]);
    let mut session = SmartboxSession::default();
    assert!(st
        .handle_get_terminals(&mut session, &terminals_msg(json!(1)))
        .is_err());
}

#[test]
fn get_terminals_list_slots_error() {
    let mock = shared(MockToken {
        list_slots_err: Some("no slots".into()),
        ..Default::default()
    });
    let st = new_test_state(Some(&mock), &[CardVendor::Posta]);
    let mut session = SmartboxSession::default();
    assert!(st
        .handle_get_terminals(&mut session, &terminals_msg(json!(1)))
        .is_err());
}

#[test]
fn get_terminals_no_slots() {
    let mock = shared(MockToken::default());
    let st = new_test_state(Some(&mock), &[CardVendor::Posta]);
    let mut session = SmartboxSession::default();
    let rsp = parse(
        &st.handle_get_terminals(&mut session, &terminals_msg(json!(1)))
            .unwrap(),
    );
    assert_eq!(rsp["payload"]["terminals"].as_array().unwrap().len(), 0);
}

// --- GET_CERTIFICATES -----------------------------------------------------------------------

#[test]
fn valid_certificates() {
    let certs = vec![
        valid_cert(&[1]),
        cert_variant(&[2], "cert_expired.der"),
        cert_variant(&[3], "cert_future.der"),
    ];
    let valid = get_valid_certificates(&certs, CardVendor::Posta);
    assert_eq!(valid.len(), 1);
    assert_eq!(valid[0].id, vec![1]);
}

#[test]
fn valid_certificates_mup_filter() {
    let certs = vec![
        cert_variant(&[1], "cert_commitonly.der"),
        cert_variant(&[2], "cert_nocommit.der"),
        valid_cert(&[3]),
    ];
    let valid = get_valid_certificates(&certs, CardVendor::Mup);
    let ids: Vec<_> = valid.iter().map(|c| c.id.clone()).collect();
    assert_eq!(ids, vec![vec![1], vec![3]]);
    // other vendors don't require content commitment
    assert_eq!(get_valid_certificates(&certs, CardVendor::Posta).len(), 3);
    assert!(get_valid_certificates(&[], CardVendor::Posta).is_empty());
}

#[test]
fn certificate_aliases() {
    let aliases = get_certificate_aliases(&[cert_variant(&[0xab, 0xcd], "cert_nocommit.der")]);
    assert_eq!(
        aliases,
        vec![CertificateAlias {
            alias: "abcd".into(),
            name: "Test User".into()
        }]
    );
    assert!(get_certificate_aliases(&[]).is_empty());
}

fn certificates_msg(pin: &str) -> Value {
    json!({"operation": "GET_CERTIFICATES", "input": {"terminalId": 0, "pin": pin}})
}

#[test]
fn get_certificates() {
    let mock = shared(MockToken {
        certs: vec![valid_cert(&[1, 2])],
        ..Default::default()
    });
    let st = new_test_state(Some(&mock), &[CardVendor::Posta]);
    let mut session = session_with(&mock, CardVendor::Posta);
    let rsp = parse(
        &st.handle_get_certificates(&mut session, &certificates_msg("1234"))
            .unwrap(),
    );
    assert_eq!(rsp["operation"], OPERATION_GET_CERTIFICATES);
    let certs = rsp["payload"]["certificates"].as_array().unwrap();
    assert_eq!(certs.len(), 1);
    assert_eq!(certs[0]["alias"], "0102");
    assert_eq!(certs[0]["name"], "Petar Petrović 12345678");
    assert_eq!(mock.lock().unwrap().login_pin, "1234");
}

#[test]
fn get_certificates_nil_module() {
    let st = new_test_state(None, &[CardVendor::Posta]);
    let mut session = SmartboxSession::default();
    assert!(st
        .handle_get_certificates(&mut session, &certificates_msg("1234"))
        .is_err());
}

#[test]
fn get_certificates_login_error() {
    let mock = shared(MockToken {
        login_err: Some("wrong pin".into()),
        ..Default::default()
    });
    let st = new_test_state(Some(&mock), &[CardVendor::Posta]);
    let mut session = session_with(&mock, CardVendor::Posta);
    assert!(st
        .handle_get_certificates(&mut session, &certificates_msg("0000"))
        .is_err());
}

#[test]
fn get_certificates_terminal_id_must_be_int() {
    let mock = shared(MockToken::default());
    let st = new_test_state(Some(&mock), &[CardVendor::Posta]);
    let mut session = session_with(&mock, CardVendor::Posta);
    let msg = json!({"operation": "GET_CERTIFICATES", "input": {"terminalId": "0", "pin": "1"}});
    assert!(st.handle_get_certificates(&mut session, &msg).is_err());
}

// --- GET_SIGNED_XML -------------------------------------------------------------------------

fn signed_xml_msg(alias: &str, xml_b64: &str) -> Value {
    json!({"operation": "GET_SIGNED_XML", "input": {
        "certificate": {"alias": alias, "name": "Test User"}, "pin": "1234", "xml": xml_b64}})
}

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s)
}

#[test]
fn get_signed_xml() {
    let mock = shared(signing_mock(vec![valid_cert(&[1, 2])]));
    let st = new_test_state(Some(&mock), &[CardVendor::Posta]);
    let mut session = session_with(&mock, CardVendor::Posta);
    let rsp = parse(
        &st.handle_get_signed_xml(
            &mut session,
            &signed_xml_msg("0102", &b64("<Document><Data>test data</Data></Document>")),
        )
        .unwrap(),
    );
    assert_eq!(rsp["operation"], OPERATION_GET_SIGNED_XML);
    let signed = base64::engine::general_purpose::STANDARD
        .decode(rsp["payload"]["xml"].as_str().unwrap())
        .unwrap();
    let signed = String::from_utf8(signed).unwrap();
    assert!(signed.contains("test data"));
    assert_eq!(verify_signed_xml(&signed).unwrap(), fixture("cert.der"));
    assert_eq!(session.certificate_id, Some(vec![1, 2]));
}

#[test]
fn get_signed_xml_accepts_unpadded_base64() {
    let mock = shared(signing_mock(vec![valid_cert(&[1])]));
    let st = new_test_state(Some(&mock), &[CardVendor::Posta]);
    let mut session = session_with(&mock, CardVendor::Posta);
    let xml = base64::engine::general_purpose::STANDARD_NO_PAD.encode("<a>x</a>");
    st.handle_get_signed_xml(&mut session, &signed_xml_msg("01", &xml))
        .unwrap();
}

#[test]
fn get_signed_xml_nil_module() {
    let st = new_test_state(None, &[CardVendor::Posta]);
    let mut session = SmartboxSession::default();
    assert!(st
        .handle_get_signed_xml(&mut session, &signed_xml_msg("0102", &b64("<Root/>")))
        .is_err());
}

#[test]
fn get_signed_xml_invalid_base64() {
    let mock = shared(MockToken {
        certs: vec![valid_cert(&[1])],
        ..Default::default()
    });
    let st = new_test_state(Some(&mock), &[CardVendor::Posta]);
    let mut session = session_with(&mock, CardVendor::Posta);
    assert!(st
        .handle_get_signed_xml(&mut session, &signed_xml_msg("01", "not-valid-base64!@#$"))
        .is_err());
}

#[test]
fn get_signed_xml_cert_not_found() {
    let mock = shared(MockToken {
        certs: vec![valid_cert(&[1])],
        ..Default::default()
    });
    let st = new_test_state(Some(&mock), &[CardVendor::Posta]);
    let mut session = session_with(&mock, CardVendor::Posta);
    assert!(st
        .handle_get_signed_xml(&mut session, &signed_xml_msg("ff", &b64("<Root/>")))
        .is_err());
    assert!(st
        .handle_get_signed_xml(&mut session, &signed_xml_msg("zz", &b64("<Root/>")))
        .is_err());
}

// --- session --------------------------------------------------------------------------------

#[test]
fn session_certificate() {
    let mock = shared(MockToken {
        certs: vec![valid_cert(&[1, 2])],
        ..Default::default()
    });
    let mut session = session_with(&mock, CardVendor::Posta);
    session.certificate_id = Some(vec![1, 2]);
    let cert = session.certificate().unwrap();
    assert_eq!(subject_common_name(&cert.der), "Petar Petrović 12345678");

    session.certificate_id = Some(vec![0xff]);
    assert!(session.certificate().is_none());

    assert!(SmartboxSession::default().certificate().is_none());
}

#[test]
fn session_sign() {
    let mock = shared(signing_mock(vec![valid_cert(&[1])]));
    let mut session = session_with(&mock, CardVendor::Posta);
    session.certificate_id = Some(vec![1]);
    let digest = Sha256Digest::of(b"hello");
    let sig = session.sign_sha256_digest(&digest).unwrap();
    verify_digest_signature(&fixture("cert.der"), &digest, &sig).unwrap();
}

struct Sha256Digest;
impl Sha256Digest {
    fn of(data: &[u8]) -> Vec<u8> {
        use sha2::Digest;
        sha2::Sha256::digest(data).to_vec()
    }
}

#[test]
fn session_sign_errors() {
    let session = SmartboxSession {
        certificate_id: Some(vec![1]),
        ..Default::default()
    };
    assert!(session.sign_sha256_digest(&[0; 32]).is_err(), "nil module");

    let mock = shared(MockToken::default());
    let session = session_with(&mock, CardVendor::Posta);
    assert!(session.sign_sha256_digest(&[0; 32]).is_err(), "no cert id");

    let mut session = session_with(&mock, CardVendor::Posta);
    session.certificate_id = Some(vec![1]);
    assert!(session.sign_sha256_digest(&[0; 20]).is_err(), "sha1 digest");
}

// --- message dispatch -----------------------------------------------------------------------

#[test]
fn on_message_requires_session() {
    let st = new_test_state(None, &[CardVendor::Posta]);
    let mut id = String::new();
    let (frame, res) = st.on_message(&mut id, r#"{"operation":"GET_PROVIDERS"}"#);
    assert_eq!(frame.as_deref(), Some(""));
    assert!(res.is_err());

    let (frame, res) = st.on_message(&mut id, "not json");
    assert!(frame.is_none());
    assert!(res.is_err());

    let (frame, res) = st.on_message(
        &mut id,
        r#"{"operation":"GET_INFO","input":{"sbSession":"s"}}"#,
    );
    assert!(res.is_ok());
    assert!(frame.unwrap().contains("GET_INFO"));
    assert_eq!(id, "s");
    let (_, res) = st.on_message(&mut id, r#"{"operation":"GET_PROVIDERS"}"#);
    assert!(res.is_ok());
    let (frame, res) = st.on_message(&mut id, r#"{"operation":"NOPE"}"#);
    assert_eq!(frame.as_deref(), Some(""));
    assert!(res.is_err());
}

// --- server (integration over a real websocket) ---------------------------------------------

type Client = WebSocket<MaybeTlsStream<TcpStream>>;

fn start_server(provider: SessionProvider) -> SmartboxServer {
    SmartboxServer::start_with_provider(vec![CardVendor::Posta], provider, &[0]).unwrap()
}

fn dial(server: &SmartboxServer, origin: Option<&str>) -> tungstenite::Result<Client> {
    let mut req = format!("ws://{}", server.address)
        .into_client_request()
        .unwrap();
    if let Some(o) = origin {
        req.headers_mut().insert("Origin", o.parse().unwrap());
    }
    let (ws, _) = tungstenite::connect(req)?;
    if let MaybeTlsStream::Plain(s) = ws.get_ref() {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    }
    Ok(ws)
}

fn read_json(ws: &mut Client) -> Value {
    loop {
        match ws.read().unwrap() {
            tungstenite::Message::Text(t) => return serde_json::from_str(t.as_str()).unwrap(),
            _ => continue,
        }
    }
}

fn write_json(ws: &mut Client, v: Value) {
    ws.send(tungstenite::Message::text(v.to_string())).unwrap();
}

/// Reads until the connection fails (the server is expected to close it).
fn expect_closed(ws: &mut Client) {
    for _ in 0..10 {
        match ws.read() {
            Ok(tungstenite::Message::Text(t)) if t.as_str().is_empty() => continue,
            Ok(_) => continue,
            Err(_) => return,
        }
    }
    panic!("connection was not closed");
}

fn get_info(ws: &mut Client, sb: &str) -> Value {
    write_json(
        ws,
        json!({"operation": "GET_INFO", "input": {"sbSession": sb}}),
    );
    read_json(ws)
}

#[test]
fn integration_full_flow() {
    let mock = shared(signing_mock(vec![valid_cert(&[0xaa, 0xbb])]));
    let server = start_server(provider_of(&mock));
    let mut ws = dial(&server, Some("https://eporezi.purs.gov.rs")).unwrap();

    let open = read_json(&mut ws);
    assert_eq!(open["operation"], ON_OPEN_EVENT);
    assert_eq!(open["payload"]["appName"], "SmartBox");

    assert_eq!(get_info(&mut ws, "flow-session")["operation"], "GET_INFO");

    write_json(&mut ws, json!({"operation": "GET_PROVIDERS"}));
    let prov = read_json(&mut ws);
    assert_eq!(
        prov["payload"]["providers"][0]["name"],
        CardVendor::Posta.name()
    );
    let provider_id = prov["payload"]["providers"][0]["id"].clone();

    write_json(
        &mut ws,
        json!({"operation": "GET_TERMINALS", "input": {"providerId": provider_id}}),
    );
    let term = read_json(&mut ws);
    assert_eq!(term["payload"]["terminals"][0]["name"], "Virtual Reader");

    write_json(&mut ws, certificates_msg("1234"));
    let certs = read_json(&mut ws);
    let cert = certs["payload"]["certificates"][0].clone();
    assert_eq!(cert["alias"], "aabb");

    write_json(
        &mut ws,
        json!({"operation": "GET_SIGNED_XML", "input": {"certificate": cert, "pin": "1234",
            "xml": b64("<Document><Data>test payload</Data></Document>")}}),
    );
    let signed = read_json(&mut ws);
    assert_eq!(signed["operation"], OPERATION_GET_SIGNED_XML);
    let xml = base64::engine::general_purpose::STANDARD
        .decode(signed["payload"]["xml"].as_str().unwrap())
        .unwrap();
    let xml = String::from_utf8(xml).unwrap();
    assert!(xml.contains("<SignatureValue>"));
    assert!(xml.contains("test payload"));
    verify_signed_xml(&xml).unwrap();

    // GET_INFO on the same session reports the selected provider and certificate
    let info = get_info(&mut ws, "flow-session");
    assert_eq!(info["payload"]["providerId"], CardVendor::Posta.id());
    assert_eq!(info["payload"]["certificateId"], "aabb");

    server.stop();
}

#[test]
fn integration_session_not_found_before_get_info() {
    let mock = shared(signing_mock(vec![]));
    let server = start_server(provider_of(&mock));
    let mut ws = dial(&server, None).unwrap();
    read_json(&mut ws);
    write_json(&mut ws, json!({"operation": "GET_PROVIDERS"}));
    expect_closed(&mut ws);
}

#[test]
fn integration_two_sessions_one_fails_login() {
    let good = shared(signing_mock(vec![valid_cert(&[0xaa, 0xbb])]));
    let bad = shared(MockToken {
        slots: vec![Slot {
            id: 0,
            description: "Reader B".into(),
        }],
        login_err: Some("incorrect PIN".into()),
        ..Default::default()
    });
    let calls = Arc::new(Mutex::new(0));
    let provider: SessionProvider = {
        let (good, bad, calls) = (good.clone(), bad.clone(), calls.clone());
        Arc::new(move |_| {
            let mut c = calls.lock().unwrap();
            *c += 1;
            Ok(if *c == 1 {
                good.clone() as SharedTokenSession
            } else {
                bad.clone() as SharedTokenSession
            })
        })
    };
    let server = start_server(provider);

    let mut a = dial(&server, None).unwrap();
    read_json(&mut a);
    get_info(&mut a, "session-a");
    write_json(&mut a, terminals_msg(json!(1)));
    read_json(&mut a);

    let mut b = dial(&server, None).unwrap();
    read_json(&mut b);
    get_info(&mut b, "session-b");
    write_json(&mut b, terminals_msg(json!(1)));
    read_json(&mut b);
    write_json(&mut b, certificates_msg("0000"));
    expect_closed(&mut b);

    write_json(&mut a, certificates_msg("1234"));
    let certs = read_json(&mut a);
    assert_eq!(certs["payload"]["certificates"][0]["alias"], "aabb");
}

#[test]
fn integration_two_sessions_one_closes_connection() {
    let mock = shared(signing_mock(vec![valid_cert(&[0xaa, 0xbb])]));
    let server = start_server(provider_of(&mock));

    let mut a = dial(&server, None).unwrap();
    read_json(&mut a);
    get_info(&mut a, "session-a");

    let mut b = dial(&server, None).unwrap();
    read_json(&mut b);
    get_info(&mut b, "session-b");

    a.close(None).unwrap();
    drop(a);
    std::thread::sleep(Duration::from_millis(100));

    write_json(&mut b, terminals_msg(json!(1)));
    assert_eq!(
        read_json(&mut b)["payload"]["terminals"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    write_json(&mut b, certificates_msg("1234"));
    let cert = read_json(&mut b)["payload"]["certificates"][0].clone();
    write_json(
        &mut b,
        json!({"operation": "GET_SIGNED_XML", "input": {"certificate": cert, "pin": "1234",
            "xml": b64("<Document><Data>session B payload</Data></Document>")}}),
    );
    let signed = read_json(&mut b);
    let xml = base64::engine::general_purpose::STANDARD
        .decode(signed["payload"]["xml"].as_str().unwrap())
        .unwrap();
    assert!(String::from_utf8(xml)
        .unwrap()
        .contains("session B payload"));
}

#[test]
fn integration_origin_rejection() {
    let mock = shared(signing_mock(vec![]));
    let server = start_server(provider_of(&mock));
    match dial(&server, Some("https://evil.example.com")) {
        Err(tungstenite::Error::Http(rsp)) => assert_eq!(rsp.status(), 403),
        Err(e) => panic!("unexpected error {e}"),
        Ok(_) => panic!("expected rejection"),
    }
    dial(&server, Some("https://test.purs.gov.rs")).unwrap();
    dial(&server, Some(&format!("http://{}", server.address))).unwrap();
}

#[test]
fn origin_rules() {
    assert!(origin_allowed(None, None));
    assert!(origin_allowed(
        Some("https://eporezi.purs.gov.rs"),
        Some("127.0.0.1:17165")
    ));
    assert!(origin_allowed(Some("https://EPOREZI.purs.gov.rs/x"), None));
    assert!(!origin_allowed(
        Some("https://eporezi.purs.gov.rs:8443"),
        None
    ));
    assert!(!origin_allowed(
        Some("https://evil.example.com"),
        Some("127.0.0.1")
    ));
    assert!(!origin_allowed(Some("null"), Some("127.0.0.1")));
    assert!(origin_allowed(
        Some("http://127.0.0.1:1"),
        Some("127.0.0.1:1")
    ));
}

#[test]
fn server_stops() {
    let mock = shared(signing_mock(vec![]));
    let server = start_server(provider_of(&mock));
    let address = server.address.clone();
    let mut ws = dial(&server, None).unwrap();
    read_json(&mut ws);
    assert!(server.is_running());
    server.stop();
    expect_closed(&mut ws);
    assert!(TcpStream::connect(address).is_err());
}

#[test]
fn start_without_modules_fails() {
    assert!(matches!(
        SmartboxServer::start(&[]),
        Err(SignError::NoModules)
    ));
}

#[test]
fn types_are_send() {
    fn assert_send<T: Send>() {}
    assert_send::<SmartboxServer>();
    assert_send::<crate::pkcs11::Modules>();
    assert_send::<Pkcs11Session>();
}
