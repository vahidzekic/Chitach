//! PKCS#11 modules (signing tokens) of Serbian certificate authorities.

use std::sync::Mutex;

use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::error::{Error as CkError, RvError};
use cryptoki::mechanism::Mechanism;
use cryptoki::object::{Attribute, AttributeType, ObjectClass};
use cryptoki::session::{Session, UserType};
use cryptoki::types::AuthPin;
use serde::{Deserialize, Serialize, Serializer};

use crate::xmlsign::{distinguished_name_string, parse_cert, DigestSigner};
use crate::{Result, SignError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CardVendor {
    Mup,
    Pks,
    Posta,
    Halcom,
    Esmart,
}

impl CardVendor {
    /// All vendors, in the order they are listed in the settings.
    pub const ALL: [CardVendor; 5] = [
        CardVendor::Mup,
        CardVendor::Pks,
        CardVendor::Posta,
        CardVendor::Halcom,
        CardVendor::Esmart,
    ];

    /// Numeric id used by the Smartbox protocol (`providerId`).
    pub fn id(self) -> i64 {
        match self {
            CardVendor::Halcom => 0,
            CardVendor::Posta => 1,
            CardVendor::Esmart => 2,
            CardVendor::Mup => 3,
            CardVendor::Pks => 4,
        }
    }

    pub fn from_id(id: i64) -> Option<CardVendor> {
        Some(match id {
            0 => CardVendor::Halcom,
            1 => CardVendor::Posta,
            2 => CardVendor::Esmart,
            3 => CardVendor::Mup,
            4 => CardVendor::Pks,
            _ => return None,
        })
    }

    /// Display name of the certificate authority.
    pub fn name(self) -> &'static str {
        match self {
            CardVendor::Halcom => "Halcom CA",
            CardVendor::Posta => "Sertifikaciono telo Pošte",
            CardVendor::Esmart => "E-Smart Systems d.o.o. Beograd (ESS QCA)",
            CardVendor::Mup => "Ministarstvo unutrašnjih poslova CA",
            CardVendor::Pks => "Privredna komora Srbije CA",
        }
    }
}

impl std::fmt::Display for CardVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulePath {
    pub vendor: CardVendor,
    pub path: String,
}

/// The current OS name as accepted by [`default_path`] ("linux" | "macos" | "windows").
pub fn current_os() -> &'static str {
    std::env::consts::OS
}

/// Default module path for the vendor on the given OS ("linux" | "macos" | "windows";
/// "darwin" is accepted too). Empty string when there is no default.
pub fn default_path(vendor: CardVendor, os: &str) -> String {
    use CardVendor::*;
    let path = match (os, vendor) {
        ("linux", Posta) => "/usr/lib/libaetpkss.so",
        ("linux", Esmart) => "/usr/lib/libeToken.so",
        ("macos" | "darwin", Posta) => {
            "/Applications/tokenadmin.app/Contents/Frameworks/libaetpkss.dylib"
        }
        ("macos" | "darwin", Esmart) => {
            "/Library/Frameworks/eToken.framework/Versions/A/libIDPrimePKCS11.dylib"
        }
        ("macos" | "darwin", Halcom) => {
            "/Applications/Personal.app/Contents/Frameworks/libtokenapi.dylib"
        }
        ("windows", Halcom) => r"C:\Program Files (x86)\Personal\bin64\personal64.dll",
        ("windows", Posta) => r"C:\Windows\System32\aetpkss1.dll",
        ("windows", Esmart) => {
            r"C:\Program Files\SafeNet\Authentication\SAC\x64\IDPrimePKCS1164.dll"
        }
        ("windows", Mup | Pks) => r"C:\Program Files\TrustEdgeID\netsetpkcs11_x64.dll",
        _ => "",
    };
    path.to_string()
}

#[derive(Debug, Clone, Serialize)]
pub struct Slot {
    pub id: u64,
    pub description: String,
}

fn serialize_hex<S: Serializer>(data: &[u8], s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(&crate::hex_encode(data))
}

/// A certificate stored on a token. Serialises as the UI's `TokenCert`
/// (`{ id: hex, label, subject, issuer, notAfter }`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedCert {
    /// `CKA_ID` of the certificate (and of the matching private key).
    #[serde(serialize_with = "serialize_hex")]
    pub id: Vec<u8>,
    /// `CKA_LABEL`, or the subject common name when the token has no label.
    pub label: String,
    /// Subject DN (RFC 2253-like format).
    pub subject: String,
    /// Issuer DN (RFC 2253-like format).
    pub issuer: String,
    /// Expiry date, `YYYY-MM-DD` (UTC).
    pub not_after: String,
    #[serde(skip)]
    pub der: Vec<u8>,
}

impl NamedCert {
    /// Builds a `NamedCert` from a DER certificate, filling subject/issuer/expiry.
    pub fn from_der(id: Vec<u8>, label: &str, der: Vec<u8>) -> Result<NamedCert> {
        let cert = parse_cert(&der)?;
        let label = if label.trim().is_empty() {
            common_name(&der).unwrap_or_default()
        } else {
            label.trim().to_string()
        };
        let not_after = cert.validity().not_after.to_datetime();
        Ok(NamedCert {
            id,
            label,
            subject: distinguished_name_string(cert.subject()),
            issuer: distinguished_name_string(cert.issuer()),
            not_after: format!(
                "{:04}-{:02}-{:02}",
                not_after.year(),
                u8::from(not_after.month()),
                not_after.day()
            ),
            der,
        })
    }
}

/// Subject common name (last CN attribute).
pub fn common_name(der: &[u8]) -> Option<String> {
    let cert = parse_cert(der).ok()?;
    cert.subject()
        .iter_common_name()
        .filter_map(crate::xmlsign::atv_string)
        .last()
}

fn ck_err(context: &str, e: CkError) -> SignError {
    match e {
        CkError::Pkcs11(rv, function) => {
            SignError::Pkcs11(format!("{context}: {function:?}: {rv:?}"))
        }
        e => SignError::Pkcs11(format!("{context}: {e}")),
    }
}

/// Loaded PKCS#11 modules (keeps libraries alive and initialised until dropped).
pub struct Modules {
    contexts: Vec<(CardVendor, Pkcs11)>,
}

impl std::fmt::Debug for Modules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Modules")
            .field("vendors", &self.vendors())
            .finish()
    }
}

impl Modules {
    /// Loads and initialises the module of every configured vendor. An empty path means the
    /// vendor's [`default_path`] for the current OS; vendors without any path are skipped.
    /// Modules that fail to load are skipped (and logged); [`SignError::NoModules`] is returned
    /// when none could be loaded. Duplicate vendors are an error.
    pub fn load(paths: &[ModulePath]) -> Result<Self> {
        let mut seen = Vec::new();
        for mp in paths {
            if seen.contains(&mp.vendor) {
                return Err(SignError::Other(format!(
                    "duplicate vendor found: {} {}",
                    mp.vendor.id(),
                    mp.path
                )));
            }
            seen.push(mp.vendor);
        }

        let mut contexts = Vec::new();
        for mp in paths {
            let path = match mp.path.trim() {
                "" => default_path(mp.vendor, current_os()),
                p => p.to_string(),
            };
            if path.is_empty() {
                continue;
            }
            let ctx = match Pkcs11::new(&path) {
                Ok(ctx) => ctx,
                Err(e) => {
                    log::warn!("loading module `{path}`: {e}");
                    continue;
                }
            };
            match ctx.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK)) {
                Ok(()) | Err(CkError::Pkcs11(RvError::CryptokiAlreadyInitialized, _)) => {}
                Err(e) => {
                    log::warn!("initializing module `{path}`: {e}");
                    continue;
                }
            }
            log::info!("loaded PKCS#11 module `{path}` for {}", mp.vendor.name());
            contexts.push((mp.vendor, ctx));
        }

        if contexts.is_empty() {
            return Err(SignError::NoModules);
        }
        Ok(Modules { contexts })
    }

    /// Vendors whose modules were loaded, in configuration order.
    pub fn vendors(&self) -> Vec<CardVendor> {
        self.contexts.iter().map(|(v, _)| *v).collect()
    }

    /// New (not yet logged in) session object for the vendor's module.
    pub fn session(&self, vendor: CardVendor) -> Result<Pkcs11Session> {
        let ctx = self
            .contexts
            .iter()
            .find(|(v, _)| *v == vendor)
            .map(|(_, c)| c.clone())
            .ok_or_else(|| SignError::Pkcs11("module context not found".into()))?;
        Ok(Pkcs11Session {
            vendor,
            ctx,
            session: None,
            certs: Mutex::new(None),
        })
    }
}

impl Drop for Modules {
    fn drop(&mut self) {
        for (_, ctx) in self.contexts.drain(..) {
            if let Err(e) = ctx.finalize() {
                log::debug!("finalizing PKCS#11 module: {e}");
            }
        }
    }
}

/// A session with a token of one module. `Send`, so it can be kept in a `Mutex`.
pub struct Pkcs11Session {
    vendor: CardVendor,
    ctx: Pkcs11,
    session: Option<Session>,
    certs: Mutex<Option<Vec<NamedCert>>>,
}

impl std::fmt::Debug for Pkcs11Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pkcs11Session")
            .field("vendor", &self.vendor)
            .field("logged_in", &self.session.is_some())
            .finish()
    }
}

/// `DigestInfo` prefix of a SHA-256 digest (RFC 8017, section 9.2).
const SHA256_DIGEST_INFO_PREFIX: [u8; 19] = [
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05,
    0x00, 0x04, 0x20,
];

impl Pkcs11Session {
    pub fn vendor(&self) -> CardVendor {
        self.vendor
    }

    pub fn is_logged_in(&self) -> bool {
        self.session.is_some()
    }

    /// Slots that contain a token.
    pub fn list_slots(&self) -> Result<Vec<Slot>> {
        let slots = self
            .ctx
            .get_slots_with_token()
            .map_err(|e| ck_err("failed to get slot list", e))?;
        let mut out = Vec::with_capacity(slots.len());
        for slot in slots {
            let Ok(info) = self.ctx.get_slot_info(slot) else {
                continue;
            };
            // some modules (NetSet) don't set the slot flags properly; asking for the token
            // info is a more reliable way to check whether the slot has a token
            if self.ctx.get_token_info(slot).is_err() {
                continue;
            }
            out.push(Slot {
                id: slot.id(),
                description: info.slot_description().trim().to_string(),
            });
        }
        Ok(out)
    }

    /// Opens a session on `slot` and logs in with the user PIN.
    /// A wrong PIN gives [`SignError::WrongPin`].
    pub fn login(&mut self, slot: u64, pin: &str) -> Result<()> {
        self.close_quietly();
        let slot = cryptoki::slot::Slot::try_from(slot)
            .map_err(|e| SignError::Pkcs11(format!("invalid slot id {slot}: {e}")))?;
        let session = self
            .ctx
            .open_rw_session(slot)
            .map_err(|e| ck_err("failed to open PKCS#11 session", e))?;
        let pin = AuthPin::from(pin.to_string());
        match session.login(UserType::User, Some(&pin)) {
            Ok(()) | Err(CkError::Pkcs11(RvError::UserAlreadyLoggedIn, _)) => {}
            Err(CkError::Pkcs11(RvError::PinIncorrect, _)) => return Err(SignError::WrongPin),
            Err(e) => return Err(ck_err("failed to login to smart card", e)),
        }
        self.session = Some(session);
        Ok(())
    }

    fn logged_in(&self) -> Result<&Session> {
        self.session
            .as_ref()
            .ok_or_else(|| SignError::Pkcs11("invalid PKCS#11 session".into()))
    }

    /// Certificates (`CKO_CERTIFICATE` objects) on the token. Cached until the next login.
    pub fn certificates(&self) -> Result<Vec<NamedCert>> {
        let session = self.logged_in()?;
        let mut cache = self.certs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(certs) = cache.as_ref() {
            if !certs.is_empty() {
                return Ok(certs.clone());
            }
        }

        let objects = session
            .find_objects(&[Attribute::Class(ObjectClass::CERTIFICATE)])
            .map_err(|e| ck_err("failed to find objects", e))?;
        let mut certs = Vec::new();
        let mut errors = Vec::new();
        for object in objects {
            let attrs = match session.get_attributes(
                object,
                &[
                    AttributeType::Id,
                    AttributeType::Label,
                    AttributeType::Value,
                ],
            ) {
                Ok(a) => a,
                Err(e) => {
                    errors.push(e.to_string());
                    continue;
                }
            };
            let (mut id, mut label, mut der) = (Vec::new(), String::new(), Vec::new());
            for attr in attrs {
                match attr {
                    Attribute::Id(v) => id = v,
                    Attribute::Label(v) => label = String::from_utf8_lossy(&v).into_owned(),
                    Attribute::Value(v) => der = v,
                    _ => {}
                }
            }
            match NamedCert::from_der(id, &label, der) {
                Ok(c) => certs.push(c),
                Err(e) => errors.push(e.to_string()),
            }
        }
        if !errors.is_empty() {
            log::warn!("reading token certificates: {}", errors.join("; "));
            if certs.is_empty() {
                return Err(SignError::Pkcs11(errors.join("; ")));
            }
        }
        *cache = Some(certs.clone());
        Ok(certs)
    }

    /// RSA PKCS#1 v1.5 signature over a SHA-256 digest, made with the private key whose
    /// `CKA_ID` equals `cert_id` (`CKM_RSA_PKCS` over the DigestInfo).
    pub fn sign_digest(&self, cert_id: &[u8], sha256: &[u8]) -> Result<Vec<u8>> {
        let session = self.logged_in()?;
        let keys = session
            .find_objects(&[
                Attribute::Class(ObjectClass::PRIVATE_KEY),
                Attribute::Id(cert_id.to_vec()),
            ])
            .map_err(|e| ck_err("private key search", e))?;
        let key = *keys
            .first()
            .ok_or_else(|| SignError::Pkcs11("no private key found".into()))?;
        let mut message = SHA256_DIGEST_INFO_PREFIX.to_vec();
        message.extend_from_slice(sha256);
        session
            .sign(&Mechanism::RsaPkcs, key, &message)
            .map_err(|e| ck_err("signing message", e))
    }

    /// Signs an XML document (enveloped XML-DSig) with the certificate `cert_id`.
    pub fn sign_xml(&self, cert_id: &[u8], xml: &[u8]) -> Result<String> {
        let cert = self
            .certificates()?
            .into_iter()
            .rfind(|c| c.id == cert_id)
            .ok_or_else(|| {
                SignError::Pkcs11(format!(
                    "certificate {} not found",
                    crate::hex_encode(cert_id)
                ))
            })?;
        crate::xmlsign::sign_xml(xml, &self.signer(cert_id), &cert.der)
    }

    /// A [`DigestSigner`] using the private key `cert_id` of this session.
    pub fn signer<'a>(&'a self, cert_id: &'a [u8]) -> TokenSigner<'a> {
        TokenSigner {
            session: self,
            cert_id,
        }
    }

    /// Logs out and closes the session.
    pub fn close(&mut self) -> Result<()> {
        *self.certs.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let Some(session) = self.session.take() else {
            return Ok(());
        };
        let logout = match session.logout() {
            Ok(()) | Err(CkError::Pkcs11(RvError::UserNotLoggedIn, _)) => Ok(()),
            Err(e) => Err(ck_err("logout", e)),
        };
        let close = session.close().map_err(|e| ck_err("close session", e));
        logout.and(close)
    }

    fn close_quietly(&mut self) {
        if let Err(e) = self.close() {
            log::debug!("{e}");
        }
    }
}

impl Drop for Pkcs11Session {
    fn drop(&mut self) {
        self.close_quietly();
    }
}

/// [`DigestSigner`] backed by a token private key.
pub struct TokenSigner<'a> {
    session: &'a Pkcs11Session,
    cert_id: &'a [u8],
}

impl DigestSigner for TokenSigner<'_> {
    fn sign_sha256_digest(&self, digest: &[u8]) -> Result<Vec<u8>> {
        self.session.sign_digest(self.cert_id, digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send>() {}

    #[test]
    fn session_is_send() {
        assert_send::<Pkcs11Session>();
        assert_send::<Modules>();
    }

    #[test]
    fn vendor_names() {
        assert_eq!(CardVendor::Halcom.name(), "Halcom CA");
        assert_eq!(CardVendor::Posta.name(), "Sertifikaciono telo Pošte");
        assert_eq!(
            CardVendor::Esmart.name(),
            "E-Smart Systems d.o.o. Beograd (ESS QCA)"
        );
        assert_eq!(
            CardVendor::Mup.name(),
            "Ministarstvo unutrašnjih poslova CA"
        );
        assert_eq!(CardVendor::Pks.name(), "Privredna komora Srbije CA");
        for v in CardVendor::ALL {
            assert_eq!(CardVendor::from_id(v.id()), Some(v));
        }
        assert_eq!(CardVendor::from_id(99), None);
        assert_eq!(CardVendor::from_id(-1), None);
    }

    #[test]
    fn default_paths() {
        use CardVendor::*;
        let cases = [
            (Posta, "linux", "/usr/lib/libaetpkss.so"),
            (Esmart, "linux", "/usr/lib/libeToken.so"),
            (Halcom, "linux", ""),
            (Mup, "linux", ""),
            (
                Posta,
                "darwin",
                "/Applications/tokenadmin.app/Contents/Frameworks/libaetpkss.dylib",
            ),
            (
                Esmart,
                "macos",
                "/Library/Frameworks/eToken.framework/Versions/A/libIDPrimePKCS11.dylib",
            ),
            (
                Halcom,
                "macos",
                "/Applications/Personal.app/Contents/Frameworks/libtokenapi.dylib",
            ),
            (Mup, "macos", ""),
            (
                Halcom,
                "windows",
                r"C:\Program Files (x86)\Personal\bin64\personal64.dll",
            ),
            (Posta, "windows", r"C:\Windows\System32\aetpkss1.dll"),
            (
                Esmart,
                "windows",
                r"C:\Program Files\SafeNet\Authentication\SAC\x64\IDPrimePKCS1164.dll",
            ),
            (
                Mup,
                "windows",
                r"C:\Program Files\TrustEdgeID\netsetpkcs11_x64.dll",
            ),
            (
                Pks,
                "windows",
                r"C:\Program Files\TrustEdgeID\netsetpkcs11_x64.dll",
            ),
            (Halcom, "freebsd", ""),
        ];
        for (vendor, os, want) in cases {
            assert_eq!(default_path(vendor, os), want, "{vendor:?} {os}");
        }
    }

    #[test]
    fn load_failures() {
        assert!(matches!(Modules::load(&[]), Err(SignError::NoModules)));
        let missing = [ModulePath {
            vendor: CardVendor::Posta,
            path: "/nonexistent/libpkcs11.so".into(),
        }];
        assert!(matches!(Modules::load(&missing), Err(SignError::NoModules)));
        let dup = [
            ModulePath {
                vendor: CardVendor::Mup,
                path: "/a.so".into(),
            },
            ModulePath {
                vendor: CardVendor::Mup,
                path: "/b.so".into(),
            },
        ];
        assert!(matches!(Modules::load(&dup), Err(SignError::Other(_))));
    }

    #[test]
    fn named_cert_from_der() {
        let der = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/cert.der"
        ))
        .unwrap();
        let c = NamedCert::from_der(vec![0xaa, 0xbb], "", der).unwrap();
        assert_eq!(c.label, "Petar Petrović 12345678");
        assert_eq!(c.not_after, "2040-01-01");
        assert!(c.issuer.starts_with("CN=Test CA"));
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json["id"], "aabb");
        assert_eq!(json["notAfter"], "2040-01-01");
        assert!(json.get("der").is_none());
    }
}
