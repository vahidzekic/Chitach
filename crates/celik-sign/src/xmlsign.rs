//! Enveloped XML-DSig signatures (RSA-SHA256, C14N 1.0).
//!
//! Port of bas-celik (MIT, (c) Nikola Ubavić) `internal/xmlsign` (`SignXMLBytes`,
//! `PopulateSignature`, `FormatSubject`). The output is byte-identical to Bas Celik's, which
//! signs with `moov-io/signedxml`:
//!
//! * a `<Signature xmlns="http://www.w3.org/2000/09/xmldsig#">` element is appended as the last
//!   child of the document root,
//! * `SignedInfo` carries an explicit copy of the XML-DSig namespace declaration,
//! * the reference (`URI=""`) uses the enveloped-signature transform, SHA-256 digest of the
//!   inclusive C14N 1.0 form of the root element,
//! * `KeyInfo/X509Data` contains the certificate (base64 *without* padding, like Go's
//!   `RawStdEncoding`), issuer name (Go `pkix.Name.String()` format), decimal serial number and
//!   the subject formatted by [`format_subject`].

use base64::Engine;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::{Pkcs1v15Sign, RsaPrivateKey, RsaPublicKey};
use sha2::{Digest, Sha256};
use x509_parser::prelude::{FromDer, X509Certificate, X509Name};
use x509_parser::x509::AttributeTypeAndValue;

use crate::xmltree::{canonicalize, Document, Element};
use crate::{Result, SignError};

pub const XMLDSIG_NAMESPACE: &str = "http://www.w3.org/2000/09/xmldsig#";
pub const C14N_ALGORITHM: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
pub const RSA_SHA256_ALGORITHM: &str = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256";
pub const ENVELOPED_SIGNATURE_ALGORITHM: &str =
    "http://www.w3.org/2000/09/xmldsig#enveloped-signature";
pub const SHA256_ALGORITHM: &str = "http://www.w3.org/2001/04/xmlenc#sha256";

/// Anything that can produce an RSA-SHA256 signature over a SHA-256 digest.
pub trait DigestSigner {
    fn sign_sha256_digest(&self, digest: &[u8]) -> Result<Vec<u8>>;
}

impl<T: DigestSigner + ?Sized> DigestSigner for &T {
    fn sign_sha256_digest(&self, digest: &[u8]) -> Result<Vec<u8>> {
        (**self).sign_sha256_digest(digest)
    }
}

/// Software RSA key implementing [`DigestSigner`] (RSASSA-PKCS1-v1_5 with SHA-256).
/// Useful for tests and for signing without a token.
#[derive(Clone)]
pub struct SoftwareSigner {
    key: RsaPrivateKey,
}

impl SoftwareSigner {
    pub fn new(key: RsaPrivateKey) -> Self {
        SoftwareSigner { key }
    }

    /// Loads an unencrypted PKCS#8 PEM (`-----BEGIN PRIVATE KEY-----`) RSA key.
    pub fn from_pkcs8_pem(pem: &str) -> Result<Self> {
        RsaPrivateKey::from_pkcs8_pem(pem)
            .map(Self::new)
            .map_err(|e| SignError::Other(format!("parsing private key: {e}")))
    }

    /// Loads an unencrypted PKCS#8 DER RSA key.
    pub fn from_pkcs8_der(der: &[u8]) -> Result<Self> {
        RsaPrivateKey::from_pkcs8_der(der)
            .map(Self::new)
            .map_err(|e| SignError::Other(format!("parsing private key: {e}")))
    }

    pub fn key(&self) -> &RsaPrivateKey {
        &self.key
    }
}

impl DigestSigner for SoftwareSigner {
    fn sign_sha256_digest(&self, digest: &[u8]) -> Result<Vec<u8>> {
        self.key
            .sign(Pkcs1v15Sign::new::<Sha256>(), digest)
            .map_err(|e| SignError::Other(format!("signing: {e}")))
    }
}

/// Verifies an RSASSA-PKCS1-v1_5 SHA-256 signature over `digest` with the certificate's key.
pub fn verify_digest_signature(cert_der: &[u8], digest: &[u8], signature: &[u8]) -> Result<()> {
    let cert = parse_cert(cert_der)?;
    let key = RsaPublicKey::from_pkcs1_der(&cert.public_key().subject_public_key.data)
        .map_err(|e| SignError::Other(format!("certificate public key: {e}")))?;
    key.verify(Pkcs1v15Sign::new::<Sha256>(), digest, signature)
        .map_err(|_| SignError::Xml("signature verification failed".into()))
}

pub(crate) fn parse_cert(der: &[u8]) -> Result<X509Certificate<'_>> {
    X509Certificate::from_der(der)
        .map(|(_, c)| c)
        .map_err(|e| SignError::Other(format!("parsing certificate: {e}")))
}

// ---------------------------------------------------------------------------
// X.509 names (Go crypto/x509/pkix semantics)
// ---------------------------------------------------------------------------

/// Decodes a directory string the way Go's x509 parser does.
pub(crate) fn atv_string(atv: &AttributeTypeAndValue) -> Option<String> {
    let any = atv.attr_value();
    let data = any.data;
    match any.tag().0 {
        // UTF8String, NumericString, PrintableString, IA5String
        12 | 18 | 19 | 22 => std::str::from_utf8(data).ok().map(str::to_string),
        // T61String: Go returns the bytes unchanged
        20 => Some(String::from_utf8_lossy(data).into_owned()),
        // BMPString (UTF-16BE)
        30 => {
            if data.len() % 2 != 0 {
                return None;
            }
            let units: Vec<u16> = data
                .chunks(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16(&units).ok()
        }
        _ => None,
    }
}

fn oid_string(atv: &AttributeTypeAndValue) -> String {
    atv.attr_type().to_id_string()
}

/// Go `pkix.Name.FillFromRDNSequence` fields.
#[derive(Default)]
struct GoName {
    names: Vec<(String, Option<String>, Vec<u8>)>,
    country: Vec<String>,
    organization: Vec<String>,
    organizational_unit: Vec<String>,
    locality: Vec<String>,
    province: Vec<String>,
    street_address: Vec<String>,
    postal_code: Vec<String>,
    serial_number: String,
    common_name: String,
}

fn der_tlv(tag: u8, data: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let len = data.len();
    if len < 0x80 {
        out.push(len as u8);
    } else {
        let bytes: Vec<u8> = len
            .to_be_bytes()
            .iter()
            .copied()
            .skip_while(|b| *b == 0)
            .collect();
        out.push(0x80 | bytes.len() as u8);
        out.extend(bytes);
    }
    out.extend_from_slice(data);
    out
}

impl GoName {
    fn from_x509(name: &X509Name) -> Self {
        let mut n = GoName::default();
        for atv in name.iter_attributes() {
            let oid = oid_string(atv);
            let value = atv_string(atv);
            let any = atv.attr_value();
            let mut tag = any.tag().0 as u8;
            if any.header.is_constructed() {
                tag |= 0x20;
            }
            let raw = der_tlv(tag, any.data);
            n.names.push((oid.clone(), value.clone(), raw));
            let Some(v) = value else { continue };
            match oid.as_str() {
                "2.5.4.3" => n.common_name = v,
                "2.5.4.5" => n.serial_number = v,
                "2.5.4.6" => n.country.push(v),
                "2.5.4.7" => n.locality.push(v),
                "2.5.4.8" => n.province.push(v),
                "2.5.4.9" => n.street_address.push(v),
                "2.5.4.10" => n.organization.push(v),
                "2.5.4.11" => n.organizational_unit.push(v),
                "2.5.4.17" => n.postal_code.push(v),
                _ => {}
            }
        }
        n
    }
}

/// Go `asn1.Marshal` of a string value (PrintableString when possible, otherwise UTF8String).
fn marshal_go_string(s: &str) -> Vec<u8> {
    let printable = s.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                ' ' | '\'' | '(' | ')' | '+' | ',' | '-' | '.' | '/' | ':' | '=' | '?'
            )
    });
    der_tlv(if printable { 0x13 } else { 0x0c }, s.as_bytes())
}

fn attribute_type_name(oid: &str) -> Option<&'static str> {
    Some(match oid {
        "2.5.4.6" => "C",
        "2.5.4.10" => "O",
        "2.5.4.11" => "OU",
        "2.5.4.3" => "CN",
        "2.5.4.5" => "SERIALNUMBER",
        "2.5.4.7" => "L",
        "2.5.4.8" => "ST",
        "2.5.4.9" => "STREET",
        "2.5.4.17" => "POSTALCODE",
        _ => return None,
    })
}

fn hex_lower(data: &[u8]) -> String {
    crate::hex_encode(data)
}

/// Formats a distinguished name exactly like Go's `pkix.Name.String()` (RFC 2253-like,
/// most specific RDN first). Used for `X509IssuerName`.
pub fn distinguished_name_string(name: &X509Name) -> String {
    let n = GoName::from_x509(name);

    // (oid, Some(string value) | None => raw DER)
    type Atv = (String, Option<String>, Vec<u8>);
    let mut rdns: Vec<Vec<Atv>> = Vec::new();
    for (oid, value, raw) in &n.names {
        if matches!(
            oid.as_str(),
            "2.5.4.3"
                | "2.5.4.5"
                | "2.5.4.6"
                | "2.5.4.7"
                | "2.5.4.8"
                | "2.5.4.9"
                | "2.5.4.10"
                | "2.5.4.11"
                | "2.5.4.17"
        ) {
            continue;
        }
        rdns.push(vec![(oid.clone(), value.clone(), raw.clone())]);
    }
    let groups: [(&str, Vec<String>); 7] = [
        ("2.5.4.6", n.country.clone()),
        ("2.5.4.8", n.province.clone()),
        ("2.5.4.7", n.locality.clone()),
        ("2.5.4.9", n.street_address.clone()),
        ("2.5.4.17", n.postal_code.clone()),
        ("2.5.4.10", n.organization.clone()),
        ("2.5.4.11", n.organizational_unit.clone()),
    ];
    for (oid, values) in groups {
        if !values.is_empty() {
            rdns.push(
                values
                    .into_iter()
                    .map(|v| (oid.to_string(), Some(v), Vec::new()))
                    .collect(),
            );
        }
    }
    if !n.common_name.is_empty() {
        rdns.push(vec![("2.5.4.3".into(), Some(n.common_name.clone()), vec![])]);
    }
    if !n.serial_number.is_empty() {
        rdns.push(vec![(
            "2.5.4.5".into(),
            Some(n.serial_number.clone()),
            vec![],
        )]);
    }

    let mut s = String::new();
    for (i, rdn) in rdns.iter().rev().enumerate() {
        if i > 0 {
            s.push(',');
        }
        for (j, (oid, value, raw)) in rdn.iter().enumerate() {
            if j > 0 {
                s.push('+');
            }
            let Some(type_name) = attribute_type_name(oid) else {
                let der = match value {
                    Some(v) => marshal_go_string(v),
                    None => raw.clone(),
                };
                s.push_str(oid);
                s.push_str("=#");
                s.push_str(&hex_lower(&der));
                continue;
            };
            let value = value.clone().unwrap_or_default();
            s.push_str(type_name);
            s.push('=');
            let last = value.len().saturating_sub(1);
            for (k, c) in value.char_indices() {
                let escape = match c {
                    ',' | '+' | '"' | '\\' | '<' | '>' | ';' => true,
                    ' ' => k == 0 || k == last,
                    '#' => k == 0,
                    _ => false,
                };
                if escape {
                    s.push('\\');
                }
                s.push(c);
            }
        }
    }
    s
}

/// Port of Go `FormatSubject`: the subject format expected by the ePorezi portal
/// (`CN=…, GIVENNAME=…, SURNAME=…, SERIALNUMBER=…, SERIALNUMBER=…, C=…`).
pub fn format_subject(subject: &X509Name) -> String {
    let n = GoName::from_x509(subject);
    let (mut ser_no1, mut ser_no2) = (String::new(), String::new());
    let (mut given_name, mut surname) = (String::new(), String::new());

    for (oid, value, _) in &n.names {
        let Some(v) = value else { continue };
        match oid.as_str() {
            "2.5.4.5" => {
                if ser_no1.is_empty() {
                    ser_no1 = v.clone();
                } else {
                    ser_no2 = v.clone();
                }
            }
            "2.5.4.42" => given_name = v.clone(),
            "2.5.4.4" => surname = v.clone(),
            _ => {}
        }
    }

    if ser_no2.contains("PNORS") {
        std::mem::swap(&mut ser_no1, &mut ser_no2);
    }

    let country = n.country.first().cloned().unwrap_or_default();
    format!(
        "CN={}, GIVENNAME={}, SURNAME={}, SERIALNUMBER={}, SERIALNUMBER={}, C={}",
        n.common_name, given_name, surname, ser_no1, ser_no2, country
    )
}

// ---------------------------------------------------------------------------
// Signing
// ---------------------------------------------------------------------------

fn b64_std(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Lenient base64 decoder (padding optional, CR/LF/whitespace ignored, like Go's decoder).
pub(crate) fn b64_decode_lenient(s: &str) -> Result<Vec<u8>> {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect::<String>()
        .trim_end_matches('=')
        .to_string();
    let engine = base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::GeneralPurposeConfig::new()
            .with_decode_allow_trailing_bits(true)
            .with_decode_padding_mode(base64::engine::DecodePaddingMode::RequireNone),
    );
    engine
        .decode(cleaned)
        .map_err(|e| SignError::Other(format!("decoding base64: {e}")))
}

/// Port of Go `PopulateSignature`: appends the signature skeleton to `root`.
fn populate_signature(root: &mut Element, cert_der: &[u8]) -> Result<()> {
    if root.child("Signature").is_some() {
        return Err(SignError::Xml("Signature element already exists".into()));
    }
    let cert = parse_cert(cert_der)?;

    let signature = root.create_element("Signature");
    signature.create_attr("xmlns", XMLDSIG_NAMESPACE);

    let signed_info = signature.create_element("SignedInfo");
    // moov-io/signedxml copies the Signature namespace declaration onto SignedInfo.
    signed_info.create_attr("xmlns", XMLDSIG_NAMESPACE);
    signed_info
        .create_element("CanonicalizationMethod")
        .create_attr("Algorithm", C14N_ALGORITHM);
    signed_info
        .create_element("SignatureMethod")
        .create_attr("Algorithm", RSA_SHA256_ALGORITHM);
    let reference = signed_info.create_element("Reference");
    reference.create_attr("URI", "");
    reference
        .create_element("Transforms")
        .create_element("Transform")
        .create_attr("Algorithm", ENVELOPED_SIGNATURE_ALGORITHM);
    reference
        .create_element("DigestMethod")
        .create_attr("Algorithm", SHA256_ALGORITHM);
    reference.create_element("DigestValue");

    signature.create_element("SignatureValue");

    let x509_data = signature
        .create_element("KeyInfo")
        .create_element("X509Data");
    x509_data.create_element("X509Certificate").set_text(
        &base64::engine::general_purpose::STANDARD_NO_PAD.encode(cert_der),
    );
    let issuer_serial = x509_data.create_element("X509IssuerSerial");
    issuer_serial
        .create_element("X509IssuerName")
        .set_text(&distinguished_name_string(cert.issuer()));
    issuer_serial
        .create_element("X509SerialNumber")
        .set_text(&cert.tbs_certificate.serial.to_string());
    x509_data
        .create_element("X509SubjectName")
        .set_text(&format_subject(cert.subject()));
    Ok(())
}

/// SHA-256 digest of the canonical form of the document after the enveloped-signature
/// transform. The signature to remove is the last direct child of the root named `Signature`.
fn reference_digest(doc: &Document) -> Result<Vec<u8>> {
    let mut copy = doc.clone();
    let root = copy
        .root_mut()
        .ok_or_else(|| SignError::Xml("missing root element".into()))?;
    let idx = root
        .children
        .iter()
        .rposition(|c| matches!(c, crate::xmltree::Node::Element(e) if e.tag == "Signature"))
        .ok_or_else(|| SignError::Xml("unable to find Signature node".into()))?;
    root.children.remove(idx);
    // signedxml serialises the transformed document and parses it again before hashing.
    let reparsed = Document::parse(&copy.to_xml_string())?;
    let root = reparsed
        .root()
        .ok_or_else(|| SignError::Xml("missing root element".into()))?;
    Ok(Sha256::digest(canonicalize(root, &[]).as_bytes()).to_vec())
}

fn signature_of(root: &Element) -> Result<&Element> {
    root.child_elements()
        .filter(|e| e.tag == "Signature")
        .last()
        .ok_or_else(|| SignError::Xml("unable to find Signature node".into()))
}

fn canonical_signed_info(root: &Element) -> Result<String> {
    let signature = signature_of(root)?;
    let signed_info = signature
        .child("SignedInfo")
        .ok_or_else(|| SignError::Xml("unable to find SignedInfo element".into()))?;
    Ok(canonicalize(signed_info, &[root, signature]))
}

/// Produce an enveloped XML-DSig signature (same output as bas-celik `SignXMLBytes`).
pub fn sign_xml(xml: &[u8], signer: &dyn DigestSigner, cert_der: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(xml)
        .map_err(|e| SignError::Xml(format!("parsing XML: invalid UTF-8: {e}")))?;
    let mut doc =
        Document::parse(text).map_err(|e| SignError::Xml(format!("parsing XML: {e}")))?;
    {
        let root = doc
            .root_mut()
            .ok_or_else(|| SignError::Xml("parsing XML: missing root element".into()))?;
        populate_signature(root, cert_der)?;
    }

    let digest = reference_digest(&doc)?;
    {
        let root = doc.root_mut().expect("root checked above");
        let signature = root
            .children
            .iter_mut()
            .rev()
            .find_map(|c| match c {
                crate::xmltree::Node::Element(e) if e.tag == "Signature" => Some(e),
                _ => None,
            })
            .expect("signature populated above");
        signature
            .child_mut("SignedInfo")
            .and_then(|si| si.child_mut("Reference"))
            .and_then(|r| r.child_mut("DigestValue"))
            .expect("skeleton has DigestValue")
            .set_text(&b64_std(&digest));
    }

    let canon = canonical_signed_info(doc.root().expect("root checked above"))?;
    let hashed = Sha256::digest(canon.as_bytes());
    let signature_value = signer.sign_sha256_digest(&hashed)?;
    {
        let root = doc.root_mut().expect("root checked above");
        let signature = root
            .children
            .iter_mut()
            .rev()
            .find_map(|c| match c {
                crate::xmltree::Node::Element(e) if e.tag == "Signature" => Some(e),
                _ => None,
            })
            .expect("signature populated above");
        signature
            .child_mut("SignatureValue")
            .expect("skeleton has SignatureValue")
            .set_text(&b64_std(&signature_value));
    }

    Ok(doc.to_xml_string())
}

/// Verifies an enveloped signature produced by [`sign_xml`] (or Bas Celik): checks the
/// algorithms, the reference digest and the RSA-SHA256 signature against the embedded
/// `X509Certificate`. Returns the DER of the signing certificate.
pub fn verify_signed_xml(xml: &str) -> Result<Vec<u8>> {
    let doc = Document::parse(xml)?;
    let root = doc
        .root()
        .ok_or_else(|| SignError::Xml("missing root element".into()))?;
    let signature = signature_of(root)?;
    let signed_info = signature
        .child("SignedInfo")
        .ok_or_else(|| SignError::Xml("unable to find SignedInfo element".into()))?;

    let algorithm = |el: Option<&Element>| el.and_then(|e| e.attr_value("Algorithm"));
    if algorithm(signed_info.child("CanonicalizationMethod")) != Some(C14N_ALGORITHM) {
        return Err(SignError::Xml("unsupported canonicalization method".into()));
    }
    if algorithm(signed_info.child("SignatureMethod")) != Some(RSA_SHA256_ALGORITHM) {
        return Err(SignError::Xml("unsupported signature method".into()));
    }
    let reference = signed_info
        .child("Reference")
        .ok_or_else(|| SignError::Xml("unable to find Reference".into()))?;
    if reference.attr_value("URI") != Some("") {
        return Err(SignError::Xml("unsupported reference URI".into()));
    }
    let transforms: Vec<&str> = reference
        .child("Transforms")
        .map(|t| {
            t.child_elements()
                .filter_map(|e| e.attr_value("Algorithm"))
                .collect()
        })
        .unwrap_or_default();
    if transforms != [ENVELOPED_SIGNATURE_ALGORITHM] {
        return Err(SignError::Xml("unsupported transforms".into()));
    }
    if algorithm(reference.child("DigestMethod")) != Some(SHA256_ALGORITHM) {
        return Err(SignError::Xml("unsupported digest method".into()));
    }

    let digest_value = reference
        .child("DigestValue")
        .map(|e| e.text())
        .ok_or_else(|| SignError::Xml("unable to find DigestValue".into()))?;
    if b64_decode_lenient(&digest_value)? != reference_digest(&doc)? {
        return Err(SignError::Xml("reference digest mismatch".into()));
    }

    let cert_der = signature
        .child("KeyInfo")
        .and_then(|k| k.child("X509Data"))
        .and_then(|d| d.child("X509Certificate"))
        .map(|c| b64_decode_lenient(&c.text()))
        .transpose()?
        .ok_or_else(|| SignError::Xml("unable to find X509Certificate".into()))?;
    let signature_value = signature
        .child("SignatureValue")
        .map(|e| b64_decode_lenient(&e.text()))
        .transpose()?
        .ok_or_else(|| SignError::Xml("unable to find SignatureValue".into()))?;

    let hashed = Sha256::digest(canonical_signed_info(root)?.as_bytes());
    verify_digest_signature(&cert_der, &hashed, &signature_value)?;
    Ok(cert_der)
}
