//! Gemalto ID cards (ID, identity card for foreigners IF, residence permit RP).

use super::id::{IdFiles, ID_DOCUMENT_FILE_LOC};
use super::{read, read_chunks, select, trim4b, CardDocument, CardKind, CardTransport};
use crate::apdu::{build_apdu, response_ok};
use crate::document::Document;
use crate::{CardError, Result};

/// AID of the ID card application (`SERID`).
pub const AID_ID: &[u8] = &[
    0xF3, 0x81, 0x00, 0x00, 0x02, 0x53, 0x45, 0x52, 0x49, 0x44, 0x01,
];
/// AID of the identity card for foreigners application (`SERIF`).
pub const AID_IF: &[u8] = &[
    0xF3, 0x81, 0x00, 0x00, 0x02, 0x53, 0x45, 0x52, 0x49, 0x46, 0x01,
];
/// AID of the residence permit application (`SERRP`).
pub const AID_RP: &[u8] = &[
    0xF3, 0x81, 0x00, 0x00, 0x02, 0x53, 0x45, 0x52, 0x52, 0x50, 0x01,
];
/// AID of the PKCS#15 cryptography application.
pub const AID_PKCS15: &[u8] = &[
    0xA0, 0x00, 0x00, 0x00, 0x63, 0x50, 0x4B, 0x43, 0x53, 0x2D, 0x31, 0x35,
];

/// Signature files.
pub const SIGNATURE_FILES: [[u8; 2]; 2] = [[0x0F, 0x1C], [0x0F, 0x1D]];
/// Certificate files.
pub const CERTIFICATE_FILES: [[u8; 2]; 2] = [[0x71, 0x02], [0x71, 0x03]];

/// ID cards with Gemalto Java OS. Gemalto replaced Apollo cards around 2014.
pub struct Gemalto<'a> {
    atr: Vec<u8>,
    card: &'a dyn CardTransport,
    files: IdFiles,
}

impl<'a> Gemalto<'a> {
    pub fn new(card: &'a dyn CardTransport, atr: Vec<u8>) -> Self {
        Self {
            atr,
            card,
            files: IdFiles::default(),
        }
    }

    /// Reads a file: 4 byte header with the little-endian data length at offset 2.
    pub fn read_file(&self, name: &[u8]) -> Result<Vec<u8>> {
        read_file(self.card, name)
    }

    /// Reads the two signature files (4 byte header trimmed).
    pub fn read_signatures(&self) -> Result<[Vec<u8>; 2]> {
        let s0 = trim4b(&self.read_file(&SIGNATURE_FILES[0])?).to_vec();
        let s1 = trim4b(&self.read_file(&SIGNATURE_FILES[1])?).to_vec();
        Ok([s0, s1])
    }
}

/// Selects one of the document applications (ID, IF or RP).
pub(crate) fn init_card(card: &dyn CardTransport) -> Result<()> {
    for aid in [AID_ID, AID_IF, AID_RP] {
        let rsp = card.transmit(&build_apdu(0x00, 0xA4, 0x04, 0x00, aid, 0))?;
        if response_ok(&rsp) {
            return Ok(());
        }
    }
    Err(CardError::Parse(
        "initializing identity document card: unknown card type".into(),
    ))
}

pub(crate) fn read_file(card: &dyn CardTransport, name: &[u8]) -> Result<Vec<u8>> {
    select(card, 0x08, 0x00, name, 4)?;

    let data = read(card, 0, 4)?;
    if data.len() < 4 {
        return Err(CardError::Parse("file too short".into()));
    }
    let length = u16::from_le_bytes([data[2], data[3]]) as usize;
    read_chunks(card, data.len(), length, 0xFF)
}

/// Reads a certificate file: 2 byte little-endian length, followed by that many bytes.
/// The returned data includes the length header.
pub(crate) fn read_certificate_file(card: &dyn CardTransport, name: &[u8]) -> Result<Vec<u8>> {
    select(card, 0x00, 0x00, name, 0)?;

    let data = read(card, 0, 2)?;
    if data.len() < 2 {
        return Err(CardError::Parse("file too short".into()));
    }
    let length = u16::from_le_bytes([data[0], data[1]]) as usize + 2;
    read_chunks(card, 0, length, 0xFF)
}

/// Selects the card's cryptography application.
pub(crate) fn init_crypto(card: &dyn CardTransport) -> Result<()> {
    select(card, 0x04, 0x00, AID_PKCS15, 0).map(|_| ())
}

impl CardDocument for Gemalto<'_> {
    fn kind(&self) -> CardKind {
        CardKind::GemaltoId
    }
    fn atr(&self) -> &[u8] {
        &self.atr
    }
    fn init_card(&mut self) -> Result<()> {
        init_card(self.card)
    }
    fn read_card(&mut self) -> Result<()> {
        self.files = IdFiles::read(|n| read_file(self.card, n))?;
        Ok(())
    }
    fn get_document(&self) -> Result<Document> {
        Ok(Document::Id(Box::new(self.files.parse()?)))
    }
    fn test(&mut self) -> bool {
        init_card(self.card).is_ok() && read_file(self.card, &ID_DOCUMENT_FILE_LOC).is_ok()
    }
}
