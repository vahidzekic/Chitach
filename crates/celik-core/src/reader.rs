//! PC/SC reader access.
use crate::{card::CardTransport, Result};

/// List connected reader names.
pub fn list_readers() -> Result<Vec<String>> { todo!() }

/// A card connected through PC/SC.
pub struct PcscCard { /* pcsc::Card */ }

impl PcscCard {
    pub fn connect(_reader: &str) -> Result<Self> { todo!() }
}

impl CardTransport for PcscCard {
    fn atr(&self) -> Result<Vec<u8>> { todo!() }
    fn transmit(&self, _apdu: &[u8]) -> Result<Vec<u8>> { todo!() }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReaderStatus {
    pub name: String,
    pub card_present: bool,
    pub atr: Option<String>,
}

/// Blocking watcher that waits for reader/card changes (SCardGetStatusChange).
pub struct ReaderMonitor { /* pcsc::Context */ }

impl ReaderMonitor {
    pub fn new() -> Result<Self> { todo!() }
    /// Block up to `timeout` until something changes; returns current status of all readers.
    pub fn wait_change(&mut self, _timeout: std::time::Duration) -> Result<Vec<ReaderStatus>> { todo!() }
}
