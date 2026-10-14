use serde::{Deserialize, Serialize};
use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CardVendor { Mup, Pks, Posta, Halcom, Esmart }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulePath { pub vendor: CardVendor, pub path: String }

/// Default module path for the vendor on the given OS ("linux" | "macos" | "windows").
pub fn default_path(_vendor: CardVendor, _os: &str) -> String { todo!() }

#[derive(Debug, Clone, Serialize)]
pub struct Slot { pub id: u64, pub description: String }

#[derive(Debug, Clone, Serialize)]
pub struct NamedCert { pub id: Vec<u8>, pub label: String, pub subject: String, pub issuer: String, pub not_after: String, #[serde(skip)] pub der: Vec<u8> }

/// Loaded PKCS#11 modules (keeps libraries alive).
pub struct Modules { /* cryptoki contexts */ }

impl Modules {
    pub fn load(_paths: &[ModulePath]) -> Result<Self> { todo!() }
    pub fn vendors(&self) -> Vec<CardVendor> { todo!() }
    pub fn session(&self, _vendor: CardVendor) -> Result<Pkcs11Session> { todo!() }
}

pub struct Pkcs11Session { /* ... */ }

impl Pkcs11Session {
    pub fn list_slots(&self) -> Result<Vec<Slot>> { todo!() }
    pub fn login(&mut self, _slot: u64, _pin: &str) -> Result<()> { todo!() }
    pub fn certificates(&self) -> Result<Vec<NamedCert>> { todo!() }
    /// RSA PKCS#1 v1.5 signature over a SHA-256 digest.
    pub fn sign_digest(&self, _cert_id: &[u8], _sha256: &[u8]) -> Result<Vec<u8>> { todo!() }
    pub fn close(&mut self) -> Result<()> { todo!() }
}
