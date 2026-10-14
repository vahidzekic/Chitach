//! Card detection and reading. Port of bas-celik `card/card.go`, `gemalto.go`,
//! `apollo.go`, `medical.go`, `vehicle.go`.

use crate::{document::Document, Result};

/// Abstraction over a connected smart card (real PC/SC card or virtual test card).
pub trait CardTransport {
    /// Answer-To-Reset bytes.
    fn atr(&self) -> Result<Vec<u8>>;
    /// Send an APDU and return the full response *including* SW1 SW2.
    fn transmit(&self, apdu: &[u8]) -> Result<Vec<u8>>;
    fn begin_transaction(&self) -> Result<()> { Ok(()) }
    fn end_transaction(&self) -> Result<()> { Ok(()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum CardKind { ApolloId, GemaltoId, Medical, Vehicle, Unknown }

/// Detect which document lives on the card (ATR + probing).
pub fn detect_card(_card: &dyn CardTransport) -> Result<CardKind> { todo!() }

/// Detect and read the whole document from the card.
pub fn read_document(_card: &dyn CardTransport) -> Result<Document> { todo!() }

/// A certificate stored on the ID card.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Certificate {
    pub label: String,
    /// DER bytes.
    pub der: Vec<u8>,
}

/// Crypto operations on (Gemalto) ID cards.
pub struct IdCardCrypto<'a> { pub card: &'a dyn CardTransport }

impl<'a> IdCardCrypto<'a> {
    pub fn new(card: &'a dyn CardTransport) -> Result<Self> { let _ = card; todo!() }
    pub fn pin_tries_left(&self) -> Result<u8> { todo!() }
    /// Returns Err(CardError::WrongPin(tries_left)) on wrong old PIN.
    pub fn change_pin(&self, _old_pin: &str, _new_pin: &str) -> Result<()> { todo!() }
    pub fn certificates(&self) -> Result<Vec<Certificate>> { todo!() }
}
