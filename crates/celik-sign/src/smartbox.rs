use crate::{pkcs11::{CardVendor, ModulePath}, Result};

/// Running Smartbox emulation server (used by the ePorezi portal).
pub struct SmartboxServer { pub address: String, pub vendors: Vec<CardVendor> }

impl SmartboxServer {
    /// Start the websocket server in a background thread.
    pub fn start(_paths: &[ModulePath]) -> Result<Self> { todo!() }
    pub fn stop(self) {}
}
