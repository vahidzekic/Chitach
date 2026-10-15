//! Real PKCS#11 test against a SoftHSM2 token. Skipped unless `CELIK_SIGN_TEST_MODULE` is set.
//!
//! Setup (Debian/Ubuntu: `apt install softhsm2 opensc`):
//! ```sh
//! export SOFTHSM2_CONF=/tmp/softhsm/softhsm2.conf   # with directories.tokendir = /tmp/softhsm/tokens
//! softhsm2-util --init-token --free --label test --pin 1234 --so-pin 5678
//! openssl pkey -in tests/fixtures/key.pem -outform DER -out /tmp/softhsm/key.der
//! M=/usr/lib/softhsm/libsofthsm2.so
//! pkcs11-tool --module $M --login --pin 1234 --write-object /tmp/softhsm/key.der --type privkey --id aabb
//! pkcs11-tool --module $M --login --pin 1234 --write-object tests/fixtures/cert.der --type cert --id aabb --label "Test cert"
//! CELIK_SIGN_TEST_MODULE=$M cargo test -p celik-sign --test softhsm
//! ```

use std::path::PathBuf;

use celik_sign::pkcs11::{CardVendor, ModulePath, Modules};
use celik_sign::xmlsign::{sign_xml, verify_signed_xml, SoftwareSigner};
use celik_sign::SignError;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap()
}

#[test]
fn softhsm_token() {
    let Ok(module) = std::env::var("CELIK_SIGN_TEST_MODULE") else {
        eprintln!("CELIK_SIGN_TEST_MODULE not set, skipping");
        return;
    };
    let modules = Modules::load(&[ModulePath {
        vendor: CardVendor::Posta,
        path: module,
    }])
    .unwrap();
    assert_eq!(modules.vendors(), vec![CardVendor::Posta]);
    assert!(modules.session(CardVendor::Mup).is_err());

    let mut session = modules.session(CardVendor::Posta).unwrap();
    let slots = session.list_slots().unwrap();
    assert!(!slots.is_empty());
    let slot = slots[0].id;

    assert!(session.certificates().is_err(), "not logged in");
    assert!(matches!(
        session.login(slot, "0000"),
        Err(SignError::WrongPin)
    ));
    session.login(slot, "1234").unwrap();

    let certs = session.certificates().unwrap();
    let cert = certs.iter().find(|c| c.id == [0xaa, 0xbb]).unwrap();
    assert_eq!(cert.der, fixture("cert.der"));
    assert_eq!(cert.label, "Test cert");

    let xml = fixture("in03_ns.xml");
    let signed = session.sign_xml(&[0xaa, 0xbb], &xml).unwrap();
    verify_signed_xml(&signed).unwrap();
    // PKCS#1 v1.5 is deterministic: same output as the software key
    let software =
        SoftwareSigner::from_pkcs8_pem(&String::from_utf8(fixture("key.pem")).unwrap()).unwrap();
    assert_eq!(signed, sign_xml(&xml, &software, &cert.der).unwrap());
    assert_eq!(signed.as_bytes(), fixture("out03_ns.xml"));

    assert!(session.sign_digest(&[0x01], &[0; 32]).is_err());
    session.close().unwrap();
    assert!(session.certificates().is_err());
    drop(session);
    drop(modules);
    smartbox_with_real_module(&std::env::var("CELIK_SIGN_TEST_MODULE").unwrap());
}

fn smartbox_with_real_module(module: &str) {
    use base64::Engine;
    use celik_sign::smartbox::SmartboxServer;
    use serde_json::{json, Value};

    let server = SmartboxServer::start(&[ModulePath {
        vendor: CardVendor::Posta,
        path: module.to_string(),
    }])
    .unwrap();
    assert_eq!(server.vendors, vec![CardVendor::Posta]);
    let (mut ws, _) = tungstenite::connect(format!("ws://{}", server.address)).unwrap();
    let mut call = |v: Option<Value>| -> Value {
        if let Some(v) = v {
            ws.send(tungstenite::Message::text(v.to_string())).unwrap();
        }
        let msg = ws.read().unwrap();
        serde_json::from_str(msg.to_text().unwrap()).unwrap()
    };
    assert_eq!(call(None)["operation"], "ON_OPEN_EVENT");
    call(Some(
        json!({"operation": "GET_INFO", "input": {"sbSession": "s"}}),
    ));
    let terms = call(Some(
        json!({"operation": "GET_TERMINALS", "input": {"providerId": "1"}}),
    ));
    let terminal: i64 = terms["payload"]["terminals"][0]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let certs = call(Some(
        json!({"operation": "GET_CERTIFICATES", "input": {"terminalId": terminal, "pin": "1234"}}),
    ));
    let cert = certs["payload"]["certificates"][0].clone();
    assert_eq!(cert["alias"], "aabb");
    let xml = base64::engine::general_purpose::STANDARD.encode(fixture("in01_simple.xml"));
    let signed = call(Some(json!({"operation": "GET_SIGNED_XML",
        "input": {"certificate": cert, "pin": "1234", "xml": xml}})));
    let signed = base64::engine::general_purpose::STANDARD
        .decode(signed["payload"]["xml"].as_str().unwrap())
        .unwrap();
    assert_eq!(signed, fixture("out01_simple.xml"));
    server.stop();
}
