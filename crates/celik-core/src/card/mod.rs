//! Card detection and reading.
//!
//! Port of bas-celik `card/card.go`, `gemalto.go`, `apollo.go`, `idCard.go`,
//! `medical.go` and `vehicle.go` (MIT, (c) Nikola Ubavić).

use crate::apdu::build_apdu;
use crate::atr::{atr_to_string, detect_card_document_by_atr};
use crate::{document::Document, CardError, Result};

pub mod apollo;
mod crypto;
pub mod gemalto;
pub mod id;
pub mod medical;
pub mod vehicle;

pub use apollo::Apollo;
pub use crypto::{cert_short_name, Certificate, IdCardCrypto};
pub use gemalto::Gemalto;
pub use medical::MedicalCard;
pub use vehicle::VehicleCard;

/// Abstraction over a connected smart card (real PC/SC card or virtual test card).
pub trait CardTransport {
    /// Answer-To-Reset bytes.
    fn atr(&self) -> Result<Vec<u8>>;
    /// Send an APDU and return the full response *including* SW1 SW2.
    fn transmit(&self, apdu: &[u8]) -> Result<Vec<u8>>;
    fn begin_transaction(&self) -> Result<()> {
        Ok(())
    }
    fn end_transaction(&self) -> Result<()> {
        Ok(())
    }
}

impl<T: CardTransport + ?Sized> CardTransport for &T {
    fn atr(&self) -> Result<Vec<u8>> {
        (**self).atr()
    }
    fn transmit(&self, apdu: &[u8]) -> Result<Vec<u8>> {
        (**self).transmit(apdu)
    }
    fn begin_transaction(&self) -> Result<()> {
        (**self).begin_transaction()
    }
    fn end_transaction(&self) -> Result<()> {
        (**self).end_transaction()
    }
}

impl<T: CardTransport + ?Sized> CardTransport for Box<T> {
    fn atr(&self) -> Result<Vec<u8>> {
        (**self).atr()
    }
    fn transmit(&self, apdu: &[u8]) -> Result<Vec<u8>> {
        (**self).transmit(apdu)
    }
    fn begin_transaction(&self) -> Result<()> {
        (**self).begin_transaction()
    }
    fn end_transaction(&self) -> Result<()> {
        (**self).end_transaction()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub enum CardKind {
    ApolloId,
    GemaltoId,
    Medical,
    Vehicle,
    Unknown,
}

/// A smart card holding a document (port of Bas Celik's `CardDocument` interface).
pub trait CardDocument {
    /// Document type of this card.
    fn kind(&self) -> CardKind;
    /// ATR of the card.
    fn atr(&self) -> &[u8];
    /// Selects the document application.
    fn init_card(&mut self) -> Result<()>;
    /// Reads all document files from the card.
    fn read_card(&mut self) -> Result<()>;
    /// Parses the files read by [`CardDocument::read_card`].
    fn get_document(&self) -> Result<Document>;
    /// Probes whether the card really holds this type of document.
    fn test(&mut self) -> bool;
}

/// Detects the card document from the card's ATR. Ambiguous cases are solved
/// by probing card content. Unknown cards yield [`CardError::UnknownCard`]
/// carrying the ATR as lowercase hex.
pub fn detect_card_document<'a>(card: &'a dyn CardTransport) -> Result<Box<dyn CardDocument + 'a>> {
    let atr = card.atr()?;

    for kind in detect_card_document_by_atr(&atr) {
        match kind {
            CardKind::ApolloId => return Ok(Box::new(Apollo::new(card, atr))),
            CardKind::GemaltoId => {
                let mut c = Gemalto::new(card, atr.clone());
                if c.test() {
                    return Ok(Box::new(c));
                }
            }
            CardKind::Vehicle => {
                let mut c = VehicleCard::new(card, atr.clone());
                if c.test() {
                    return Ok(Box::new(c));
                }
            }
            CardKind::Medical => {
                let mut c = MedicalCard::new(card, atr.clone());
                if c.test() {
                    return Ok(Box::new(c));
                }
            }
            CardKind::Unknown => return Err(CardError::UnknownCard(atr_to_string(&atr))),
        }
    }

    log::info!("unexpected card type (ATR: {})", atr_to_string(&atr));
    Err(CardError::UnknownCard(atr_to_string(&atr)))
}

/// Detect which document lives on the card (ATR + probing).
pub fn detect_card(card: &dyn CardTransport) -> Result<CardKind> {
    match detect_card_document(card) {
        Ok(doc) => Ok(doc.kind()),
        Err(CardError::UnknownCard(_)) => Ok(CardKind::Unknown),
        Err(e) => Err(e),
    }
}

/// Detect and read the whole document from the card (inside a card transaction).
pub fn read_document(card: &dyn CardTransport) -> Result<Document> {
    card.begin_transaction()?;
    let result = (|| {
        let mut doc = detect_card_document(card)?;
        log::info!("ATR read: {}", atr_to_string(doc.atr()));
        doc.init_card()?;
        doc.read_card()?;
        doc.get_document()
    })();
    if let Err(e) = card.end_transaction() {
        log::warn!("ending transaction: {e}");
    }
    result
}

/// Reads binary data from the card starting at `offset` (at most 0xFF bytes).
/// Like Bas Celik, the status word is stripped without being checked.
pub(crate) fn read(card: &dyn CardTransport, offset: usize, length: usize) -> Result<Vec<u8>> {
    if length == 0 {
        return Ok(Vec::new());
    }
    if offset >= 0xFFFF {
        return Err(CardError::Parse("reading binary: offset too large".into()));
    }

    let read_size = length.min(0xFF);
    let apdu = build_apdu(0x00, 0xB0, (offset >> 8) as u8, offset as u8, &[], read_size);
    let mut rsp = card.transmit(&apdu)?;
    if rsp.len() < 2 {
        return Err(CardError::Parse("reading binary: bad status code".into()));
    }
    rsp.truncate(rsp.len() - 2);
    Ok(rsp)
}

/// Reads `length` bytes from `offset` in chunks of at most `chunk` bytes.
pub(crate) fn read_chunks(card: &dyn CardTransport, mut offset: usize, mut length: usize, chunk: usize) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(length);
    while length > 0 {
        let data = read(card, offset, length.min(chunk))?;
        if data.is_empty() {
            break;
        }
        offset += data.len();
        length = length.saturating_sub(data.len());
        output.extend_from_slice(&data);
    }
    Ok(output)
}

/// Sends a SELECT command and checks for `90 00`.
pub(crate) fn select(card: &dyn CardTransport, p1: u8, p2: u8, name: &[u8], ne: usize) -> Result<Vec<u8>> {
    let apdu = build_apdu(0x00, 0xA4, p1, p2, name, ne);
    let rsp = card.transmit(&apdu)?;
    check_ok(&rsp, &format!("selecting file {}", crate::apdu::to_hex(name)))?;
    Ok(rsp)
}

/// Converts a non-`90 00` response into [`CardError::BadStatus`].
pub(crate) fn check_ok(rsp: &[u8], what: &str) -> Result<()> {
    if crate::apdu::response_ok(rsp) {
        return Ok(());
    }
    log::debug!("{what}: response {}", crate::apdu::to_hex(rsp));
    match crate::apdu::status_word(rsp) {
        Some((a, b)) => Err(CardError::BadStatus(a, b)),
        None => Err(CardError::Parse(format!("{what}: empty response"))),
    }
}

/// Adds context to parse errors.
pub(crate) fn context(e: CardError, what: &str) -> CardError {
    match e {
        CardError::Parse(m) => CardError::Parse(format!("{what}: {m}")),
        other => other,
    }
}

/// Trims four bytes from the start of the slice (if it is longer than four bytes).
pub(crate) fn trim4b(data: &[u8]) -> &[u8] {
    if data.len() > 4 {
        &data[4..]
    } else {
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim() {
        assert_eq!(trim4b(&[1, 2, 3, 4, 5]), &[5]);
        assert_eq!(trim4b(&[1, 2, 3, 4]), &[1, 2, 3, 4]);
        assert_eq!(trim4b(&[]), &[] as &[u8]);
    }
}
