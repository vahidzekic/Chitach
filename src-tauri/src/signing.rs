//! "xmlSign" mode: sign XML files with a PKCS#11 token.
//!
//! Port of bas-celik `internal/gui/docSigning.go` / `docSigner` (MIT, (c) Nikola Ubavić).

use std::sync::Mutex;

use celik_sign::{
    pkcs11::{CardVendor, Modules, NamedCert, Pkcs11Session},
    xmlsign::{sign_xml, DigestSigner},
};
use serde::Serialize;

use crate::{
    error::{catch, sign_error, with_detail, CmdResult},
    settings::Settings,
    state::lock,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotInfo {
    pub id: u64,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCert {
    /// Hex encoded CKA_ID.
    pub id: String,
    pub label: String,
    pub subject: String,
    pub issuer: String,
    pub not_after: String,
}

pub fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

impl From<&NamedCert> for TokenCert {
    fn from(c: &NamedCert) -> Self {
        TokenCert {
            id: to_hex(&c.id),
            label: c.label.clone(),
            subject: c.subject.clone(),
            issuer: c.issuer.clone(),
            not_after: c.not_after.clone(),
        }
    }
}

#[derive(Default)]
struct Inner {
    modules: Option<Modules>,
    session: Option<(CardVendor, Pkcs11Session)>,
    certs: Vec<NamedCert>,
}

/// Tauri-managed signing state.
#[derive(Default)]
pub struct SigningState(Mutex<Inner>);

fn os() -> &'static str {
    std::env::consts::OS
}

pub fn module_paths(settings: &Settings) -> Vec<celik_sign::pkcs11::ModulePath> {
    settings
        .pkcs11_paths
        .module_paths(|v| celik_sign::pkcs11::default_path(v, os()))
}

struct SessionSigner<'a> {
    session: &'a Pkcs11Session,
    id: &'a [u8],
}

impl DigestSigner for SessionSigner<'_> {
    fn sign_sha256_digest(&self, digest: &[u8]) -> celik_sign::Result<Vec<u8>> {
        self.session.sign_digest(self.id, digest)
    }
}

impl SigningState {
    pub fn load_modules(&self, settings: &Settings) -> CmdResult<Vec<CardVendor>> {
        let mut inner = lock(&self.0);
        close_session(&mut inner);
        inner.modules = None;
        let paths = catch("error.pkcs11", || Ok(module_paths(settings)))?;
        let modules = catch("error.pkcs11", || {
            Modules::load(&paths).map_err(|e| sign_error("error.pkcs11", &e))
        })?;
        let vendors = catch("error.pkcs11", || Ok(modules.vendors()))?;
        inner.modules = Some(modules);
        Ok(vendors)
    }

    fn ensure_session(inner: &mut Inner, vendor: CardVendor) -> CmdResult<()> {
        if matches!(&inner.session, Some((v, _)) if *v == vendor) {
            return Ok(());
        }
        close_session(inner);
        let modules = inner.modules.as_ref().ok_or("error.noModules")?;
        let session = catch("error.pkcs11", || {
            modules
                .session(vendor)
                .map_err(|e| sign_error("error.pkcs11", &e))
        })?;
        inner.session = Some((vendor, session));
        Ok(())
    }

    pub fn list_slots(&self, vendor: CardVendor) -> CmdResult<Vec<SlotInfo>> {
        let mut inner = lock(&self.0);
        Self::ensure_session(&mut inner, vendor)?;
        let (_, session) = inner.session.as_ref().ok_or("error.pkcs11")?;
        let slots = catch("error.pkcs11", || {
            session
                .list_slots()
                .map_err(|e| sign_error("error.pkcs11", &e))
        })?;
        Ok(slots
            .into_iter()
            .map(|s| SlotInfo {
                id: s.id,
                description: s.description,
            })
            .collect())
    }

    pub fn login(&self, vendor: CardVendor, slot: u64, pin: &str) -> CmdResult<Vec<TokenCert>> {
        let mut inner = lock(&self.0);
        Self::ensure_session(&mut inner, vendor)?;
        let (_, session) = inner.session.as_mut().ok_or("error.pkcs11")?;
        let certs = catch("error.pkcs11", || {
            session
                .login(slot, pin)
                .map_err(|e| sign_error("error.pkcs11", &e))?;
            session
                .certificates()
                .map_err(|e| sign_error("error.pkcs11", &e))
        })?;
        let out = certs.iter().map(TokenCert::from).collect();
        inner.certs = certs;
        Ok(out)
    }

    pub fn sign_file(&self, cert_id: &str, input: &str, output: &str) -> CmdResult<()> {
        let inner = lock(&self.0);
        let (_, session) = inner.session.as_ref().ok_or("error.notLoggedIn")?;
        let id =
            from_hex(cert_id).ok_or_else(|| with_detail("error.sign", "bad certificate id"))?;
        let cert = inner
            .certs
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| with_detail("error.sign", "certificate not found"))?;
        let xml = std::fs::read(input).map_err(|e| with_detail("error.openFile", e))?;
        let signer = SessionSigner {
            session,
            id: &cert.id,
        };
        let signed = catch("error.sign", || {
            sign_xml(&xml, &signer, &cert.der).map_err(|e| sign_error("error.sign", &e))
        })?;
        std::fs::write(output, signed).map_err(|e| with_detail("error.saveFile", e))
    }

    pub fn logout(&self) -> CmdResult<()> {
        let mut inner = lock(&self.0);
        close_session(&mut inner);
        Ok(())
    }
}

fn close_session(inner: &mut Inner) {
    inner.certs.clear();
    if let Some((_, mut s)) = inner.session.take() {
        let r = catch("error.pkcs11", || {
            s.close().map_err(|e| sign_error("error.pkcs11", &e))
        });
        if let Err(e) = r {
            log::warn!("closing PKCS#11 session: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        assert_eq!(to_hex(&[0, 0xab, 0x10]), "00ab10");
        assert_eq!(from_hex("00AB10"), Some(vec![0, 0xab, 0x10]));
        assert_eq!(from_hex("0"), None);
        assert_eq!(from_hex("zz"), None);
    }

    #[test]
    fn not_logged_in() {
        let s = SigningState::default();
        assert_eq!(
            s.sign_file("00", "a", "b").unwrap_err(),
            "error.notLoggedIn"
        );
        assert_eq!(
            s.list_slots(CardVendor::Mup).unwrap_err(),
            "error.noModules"
        );
    }
}
