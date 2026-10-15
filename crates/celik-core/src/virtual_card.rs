//! In-memory smart card used by tests, mock mode and UI development.
//!
//! Inspired by bas-celik `card/smartCard.go` `MakeVirtualCard` (MIT, (c) Nikola Ubavić),
//! but implementing enough of ISO 7816-4 (SELECT, READ BINARY, VERIFY,
//! CHANGE REFERENCE DATA) for the real card readers in [`crate::card`] to work on it.

use crate::card::gemalto::{AID_ID, AID_IF, AID_PKCS15, AID_RP};
use crate::card::id::{
    ID_DOCUMENT_FILE_LOC, ID_PERSONAL_FILE_LOC, ID_PHOTO_FILE_LOC, ID_RESIDENCE_FILE_LOC,
};
use crate::card::medical::{
    AID_MEDICAL, MED_DOCUMENT_FILE_LOC, MED_FIXED_PERSONAL_FILE_LOC, MED_VARIABLE_ADMIN_FILE_LOC,
    MED_VARIABLE_PERSONAL_FILE_LOC,
};
use crate::card::vehicle::{INIT_COMMANDS, VEHICLE_FILES};
use crate::card::CardTransport;
use crate::pin::pad_pin;
use crate::Result;
use std::collections::HashMap;
use std::io::Write;
use std::sync::Mutex;

const SW_OK: [u8; 2] = [0x90, 0x00];
const SW_FILE_NOT_FOUND: [u8; 2] = [0x6A, 0x82];
const SW_NO_EF_SELECTED: [u8; 2] = [0x69, 0x86];
const SW_WRONG_OFFSET: [u8; 2] = [0x6B, 0x00];
const SW_INS_NOT_SUPPORTED: [u8; 2] = [0x6D, 0x00];
const SW_WRONG_LENGTH: [u8; 2] = [0x67, 0x00];
const SW_PIN_BLOCKED: [u8; 2] = [0x69, 0x83];
const SW_REF_DATA_NOT_FOUND: [u8; 2] = [0x6A, 0x88];

/// Mutable part of the virtual card.
#[derive(Debug, Default)]
struct State {
    selected: Option<Vec<u8>>,
    pin: Option<[u8; 8]>,
    max_tries: u8,
    tries_left: u8,
    log: Vec<Vec<u8>>,
    transaction_depth: i32,
}

/// A virtual card: file id -> file content, plus a set of selectable AIDs and an optional PIN.
#[derive(Debug, Default)]
pub struct VirtualCard {
    atr: Vec<u8>,
    files: HashMap<Vec<u8>, Vec<u8>>,
    aids: Vec<Vec<u8>>,
    state: Mutex<State>,
}

impl VirtualCard {
    /// Empty card with the given ATR (like Go's `MakeVirtualCard`).
    pub fn new(atr: impl Into<Vec<u8>>) -> Self {
        Self {
            atr: atr.into(),
            ..Default::default()
        }
    }

    /// Card with the given raw files (file id -> complete file content, including headers).
    pub fn with_files(atr: impl Into<Vec<u8>>, files: HashMap<Vec<u8>, Vec<u8>>) -> Self {
        Self {
            atr: atr.into(),
            files,
            ..Default::default()
        }
    }

    /// Adds a raw file (complete content, including any card specific header).
    pub fn with_file(mut self, id: &[u8], content: impl Into<Vec<u8>>) -> Self {
        self.files.insert(id.to_vec(), content.into());
        self
    }

    /// Adds an application identifier that can be selected (answers `90 00`).
    pub fn with_aid(mut self, aid: &[u8]) -> Self {
        self.aids.push(aid.to_vec());
        self
    }

    /// Sets a PIN with the given number of tries.
    pub fn with_pin(self, pin: &str, tries: u8) -> Self {
        {
            let mut s = self.state.lock().unwrap();
            s.pin = Some(pad_pin(pin));
            s.max_tries = tries;
            s.tries_left = tries;
        }
        self
    }

    /// Adds a certificate file in the compressed format used by Gemalto ID
    /// cards (2 byte length, 4 byte header, zlib stream of the DER bytes).
    pub fn with_certificate(self, id: &[u8], der: &[u8]) -> Self {
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(der).expect("writing to a Vec cannot fail");
        let zlib = enc.finish().expect("writing to a Vec cannot fail");
        let mut content = vec![0x01, 0x00];
        content.extend_from_slice(&(der.len() as u16).to_le_bytes());
        content.extend_from_slice(&zlib);
        let mut file = (content.len() as u16).to_le_bytes().to_vec();
        file.extend_from_slice(&content);
        self.with_file(id, file)
    }

    /// Gemalto ID card (`SERID` application) from the TLV payloads and portrait JPEG.
    pub fn gemalto_id(
        atr: &[u8],
        document: &[u8],
        personal: &[u8],
        residence: &[u8],
        portrait_jpeg: &[u8],
    ) -> Self {
        Self::gemalto_with_aid(atr, AID_ID, document, personal, residence, portrait_jpeg)
    }

    /// Gemalto card with a specific document application
    /// ([`AID_ID`], [`AID_IF`] for foreigners or [`AID_RP`] for residence permits).
    pub fn gemalto_with_aid(
        atr: &[u8],
        aid: &[u8],
        document: &[u8],
        personal: &[u8],
        residence: &[u8],
        portrait_jpeg: &[u8],
    ) -> Self {
        debug_assert!([AID_ID, AID_IF, AID_RP].contains(&aid));
        let mut photo = vec![0u8; 4];
        photo.extend_from_slice(portrait_jpeg);
        Self::new(atr)
            .with_aid(aid)
            .with_aid(AID_PKCS15)
            .with_file(&ID_DOCUMENT_FILE_LOC, gemalto_file(document))
            .with_file(&ID_PERSONAL_FILE_LOC, gemalto_file(personal))
            .with_file(&ID_RESIDENCE_FILE_LOC, gemalto_file(residence))
            .with_file(&ID_PHOTO_FILE_LOC, gemalto_file(&photo))
    }

    /// Apollo ID card from the TLV payloads and portrait JPEG.
    pub fn apollo_id(
        atr: &[u8],
        document: &[u8],
        personal: &[u8],
        residence: &[u8],
        portrait_jpeg: &[u8],
    ) -> Self {
        let mut photo = vec![0u8; 4];
        photo.extend_from_slice(portrait_jpeg);
        Self::new(atr)
            .with_file(&ID_DOCUMENT_FILE_LOC, apollo_file(document))
            .with_file(&ID_PERSONAL_FILE_LOC, apollo_file(personal))
            .with_file(&ID_RESIDENCE_FILE_LOC, apollo_file(residence))
            .with_file(&ID_PHOTO_FILE_LOC, apollo_file(&photo))
    }

    /// Medical insurance card from the four TLV payloads.
    pub fn medical(atr: &[u8], files: [&[u8]; 4]) -> Self {
        let ids = [
            MED_DOCUMENT_FILE_LOC,
            MED_FIXED_PERSONAL_FILE_LOC,
            MED_VARIABLE_PERSONAL_FILE_LOC,
            MED_VARIABLE_ADMIN_FILE_LOC,
        ];
        ids.iter()
            .zip(files)
            .fold(Self::new(atr).with_aid(AID_MEDICAL), |c, (id, f)| {
                c.with_file(id, gemalto_file(f))
            })
    }

    /// Vehicle registration card from the four BER payloads.
    pub fn vehicle(atr: &[u8], files: [&[u8]; 4]) -> Self {
        VEHICLE_FILES.iter().zip(files).fold(
            Self::new(atr).with_aid(INIT_COMMANDS[0][0]),
            |c, (id, f)| c.with_file(id, vehicle_file(f)),
        )
    }

    /// All APDUs sent to the card so far.
    pub fn sent_apdus(&self) -> Vec<Vec<u8>> {
        self.state.lock().unwrap().log.clone()
    }

    /// Current transaction nesting depth (0 when balanced).
    pub fn transaction_depth(&self) -> i32 {
        self.state.lock().unwrap().transaction_depth
    }

    /// PIN tries left.
    pub fn pin_tries_left(&self) -> u8 {
        self.state.lock().unwrap().tries_left
    }

    fn process(&self, apdu: &[u8]) -> Vec<u8> {
        let mut s = self.state.lock().unwrap();
        s.log.push(apdu.to_vec());

        if apdu.len() < 4 {
            return SW_WRONG_LENGTH.to_vec();
        }
        let (ins, p1, p2) = (apdu[1], apdu[2], apdu[3]);
        let Some((data, le)) = split_body(&apdu[4..]) else {
            return SW_WRONG_LENGTH.to_vec();
        };

        match ins {
            0xA4 => {
                if self.files.contains_key(data) {
                    s.selected = Some(data.to_vec());
                    SW_OK.to_vec()
                } else if self.aids.iter().any(|a| a == data) {
                    s.selected = None;
                    SW_OK.to_vec()
                } else {
                    SW_FILE_NOT_FOUND.to_vec()
                }
            }
            0xB0 => {
                let Some(file) = s.selected.as_ref().and_then(|id| self.files.get(id)) else {
                    return SW_NO_EF_SELECTED.to_vec();
                };
                let offset = (p1 as usize) << 8 | p2 as usize;
                if offset > file.len() {
                    return SW_WRONG_OFFSET.to_vec();
                }
                let le = match le {
                    None | Some(0) => 256,
                    Some(n) => n,
                };
                let end = (offset + le).min(file.len());
                let mut rsp = file[offset..end].to_vec();
                rsp.extend_from_slice(&SW_OK);
                rsp
            }
            0x20 => {
                if s.pin.is_none() {
                    return SW_REF_DATA_NOT_FOUND.to_vec();
                }
                if data.is_empty() {
                    return tries_sw(s.tries_left).to_vec();
                }
                check_pin(&mut s, data).to_vec()
            }
            0x24 => {
                if s.pin.is_none() {
                    return SW_REF_DATA_NOT_FOUND.to_vec();
                }
                if data.len() != 16 {
                    return SW_WRONG_LENGTH.to_vec();
                }
                let sw = check_pin(&mut s, &data[..8]);
                if sw == SW_OK {
                    let mut new = [0u8; 8];
                    new.copy_from_slice(&data[8..]);
                    s.pin = Some(new);
                }
                sw.to_vec()
            }
            _ => SW_INS_NOT_SUPPORTED.to_vec(),
        }
    }
}

fn tries_sw(tries: u8) -> [u8; 2] {
    if tries == 0 {
        SW_PIN_BLOCKED
    } else {
        [0x63, 0xC0 | tries.min(0x0F)]
    }
}

fn check_pin(s: &mut State, candidate: &[u8]) -> [u8; 2] {
    if s.tries_left == 0 {
        return SW_PIN_BLOCKED;
    }
    if s.pin.as_ref().map(|p| p.as_slice()) == Some(candidate) {
        s.tries_left = s.max_tries;
        SW_OK
    } else {
        s.tries_left -= 1;
        [0x63, 0xC0 | s.tries_left.min(0x0F)]
    }
}

/// Splits a short APDU body into (data, Le).
fn split_body(body: &[u8]) -> Option<(&[u8], Option<usize>)> {
    match body.len() {
        0 => Some((&[], None)),
        1 => Some((&[], Some(body[0] as usize))),
        _ => {
            let lc = body[0] as usize;
            let rest = &body[1..];
            match rest.len().checked_sub(lc)? {
                0 => Some((rest, None)),
                1 => Some((&rest[..lc], Some(rest[lc] as usize))),
                _ => None,
            }
        }
    }
}

/// Wraps data with the 4 byte header used by Gemalto ID and medical cards.
pub fn gemalto_file(payload: &[u8]) -> Vec<u8> {
    let mut f = vec![0x00, 0x00];
    f.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    f.extend_from_slice(payload);
    f
}

/// Wraps data with the 6 byte header used by Apollo ID cards.
pub fn apollo_file(payload: &[u8]) -> Vec<u8> {
    let mut f = vec![0x00, 0x00, 0x00, 0x00];
    f.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    f.extend_from_slice(payload);
    f
}

/// Prefixes a BER record with the header found on vehicle cards.
pub fn vehicle_file(ber_payload: &[u8]) -> Vec<u8> {
    let mut f = vec![
        0x78, 0x0E, 0x4F, 0x0C, 0xA0, 0x00, 0x00, 0x00, 0x18, 0x65, 0x56, 0x4C, 0x2D, 0x30, 0x30,
        0x31,
    ];
    f.extend_from_slice(ber_payload);
    f
}

impl CardTransport for VirtualCard {
    fn atr(&self) -> Result<Vec<u8>> {
        Ok(self.atr.clone())
    }

    fn transmit(&self, apdu: &[u8]) -> Result<Vec<u8>> {
        Ok(self.process(apdu))
    }

    fn begin_transaction(&self) -> Result<()> {
        self.state.lock().unwrap().transaction_depth += 1;
        Ok(())
    }

    fn end_transaction(&self) -> Result<()> {
        self.state.lock().unwrap().transaction_depth -= 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apdu::build_apdu;

    #[test]
    fn select_and_read() {
        let c = VirtualCard::new(vec![0x3B])
            .with_file(&[0x0F, 0x02], (0..=255u8).chain(0..10).collect::<Vec<_>>());
        assert_eq!(
            c.transmit(&build_apdu(0, 0xB0, 0, 0, &[], 4)).unwrap(),
            SW_NO_EF_SELECTED
        );
        assert_eq!(
            c.transmit(&build_apdu(0, 0xA4, 8, 0, &[0x0F, 0x01], 4))
                .unwrap(),
            SW_FILE_NOT_FOUND
        );
        assert_eq!(
            c.transmit(&build_apdu(0, 0xA4, 8, 0, &[0x0F, 0x02], 4))
                .unwrap(),
            SW_OK
        );
        assert_eq!(
            c.transmit(&build_apdu(0, 0xB0, 0, 2, &[], 3)).unwrap(),
            vec![2, 3, 4, 0x90, 0]
        );
        assert_eq!(
            c.transmit(&build_apdu(0, 0xB0, 1, 0, &[], 0xFF))
                .unwrap()
                .len(),
            12
        );
        assert_eq!(
            c.transmit(&build_apdu(0, 0xB0, 2, 0, &[], 1)).unwrap(),
            SW_WRONG_OFFSET
        );
        assert_eq!(c.transmit(&[0, 0xCA, 0, 0]).unwrap(), SW_INS_NOT_SUPPORTED);
        assert_eq!(c.sent_apdus().len(), 7);
    }

    #[test]
    fn pin() {
        let c = VirtualCard::new(vec![]).with_pin("1234", 3);
        let verify = |p: &str| {
            c.transmit(&build_apdu(0, 0x20, 0, 0x80, &pad_pin(p), 0))
                .unwrap()
        };
        assert_eq!(
            c.transmit(&build_apdu(0, 0x20, 0, 0x80, &[], 0)).unwrap(),
            [0x63, 0xC3]
        );
        assert_eq!(verify("0000"), [0x63, 0xC2]);
        assert_eq!(verify("1234"), SW_OK);
        assert_eq!(c.pin_tries_left(), 3);
        for expected in [[0x63, 0xC2], [0x63, 0xC1], [0x63, 0xC0], SW_PIN_BLOCKED] {
            assert_eq!(verify("9999"), expected);
        }
        assert_eq!(verify("1234"), SW_PIN_BLOCKED);
    }
}
