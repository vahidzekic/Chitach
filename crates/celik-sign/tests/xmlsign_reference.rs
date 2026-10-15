//! Byte-for-byte comparison of signed XML against reference fixtures.
//!
//! The `out*.xml` fixtures are the expected signatures of the corresponding `in*.xml` fixtures, with the RSA key
//! `key.pem` and certificate `cert.der`. RSASSA-PKCS1-v1_5 is deterministic, so the Rust output
//! must match exactly.

use std::path::PathBuf;

use celik_sign::xmlsign::{
    distinguished_name_string, format_subject, sign_xml, verify_signed_xml, SoftwareSigner,
};
use x509_parser::prelude::{FromDer, X509Certificate};

fn fixture(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn signer() -> SoftwareSigner {
    SoftwareSigner::from_pkcs8_pem(&String::from_utf8(fixture("key.pem")).unwrap()).unwrap()
}

#[test]
fn identical_to_reference_output() {
    let cert = fixture("cert.der");
    let signer = signer();
    for case in [
        "01_simple",
        "02_decl",
        "03_ns",
        "04_defaultns",
        "05_nested_sig",
    ] {
        let input = fixture(&format!("in{case}.xml"));
        let expected = String::from_utf8(fixture(&format!("out{case}.xml"))).unwrap();
        let signed = sign_xml(&input, &signer, &cert).unwrap();
        assert_eq!(signed, expected, "case {case}");
        let signer_cert = verify_signed_xml(&signed).unwrap();
        assert_eq!(signer_cert, cert);
    }
}

#[test]
fn rejects_invalid_input() {
    let cert = fixture("cert.der");
    let signer = signer();
    let err = sign_xml(&fixture("err01_existing_sig.xml"), &signer, &cert).unwrap_err();
    assert!(err.to_string().contains("Signature element already exists"));
    assert!(sign_xml(&fixture("err02_malformed.xml"), &signer, &cert).is_err());
    assert!(sign_xml(b"", &signer, &cert).is_err());
    assert!(sign_xml(b"<a/>", &signer, b"not a cert").is_err());
}

#[test]
fn tampered_documents_do_not_verify() {
    let cert = fixture("cert.der");
    let signed = sign_xml(&fixture("in03_ns.xml"), &signer(), &cert).unwrap();
    let tampered = signed.replace("1234.56", "1234.57");
    assert!(verify_signed_xml(&tampered).is_err());
    let tampered = signed.replace("<ns1:VrstaPrijave>1<", "<ns1:VrstaPrijave>2<");
    assert!(verify_signed_xml(&tampered).is_err());
    // whitespace-only reformatting of SignedInfo content is not allowed either
    let tampered = signed.replace("<Reference URI=\"\">", "<Reference URI=\"\" >\n");
    assert!(verify_signed_xml(&tampered).is_err() || tampered == signed);
}

#[test]
fn certificate_names() {
    let der = fixture("cert.der");
    let (_, cert) = X509Certificate::from_der(&der).unwrap();
    // expected issuer DN and formatted subject of the test certificate
    assert_eq!(
        distinguished_name_string(cert.issuer()),
        r#"CN=Test CA\, \"Root\" #1,O=Test Org,L=Beograd,C=RS,2.5.4.97=#130f56415452532d313030303030303030"#
    );
    assert_eq!(
        distinguished_name_string(cert.subject()),
        "SERIALNUMBER=PNORS-1234567890123,CN=Petar Petrović 12345678,C=RS,2.5.4.42=#13055065746172,2.5.4.4=#0c09506574726f7669c487"
    );
    assert_eq!(
        format_subject(cert.subject()),
        "CN=Petar Petrović 12345678, GIVENNAME=Petar, SURNAME=Petrović, SERIALNUMBER=PNORS-1234567890123, SERIALNUMBER=12345678, C=RS"
    );
}
