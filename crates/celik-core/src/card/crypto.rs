//! Cryptographic operations on Gemalto ID cards: PIN and certificates.
//!
//! Port of the crypto part of bas-celik `card/gemalto.go` and
//! `internal/gui/crypto.go` (MIT, (c) Nikola Ubavić).

use super::gemalto::{init_crypto, read_certificate_file, CERTIFICATE_FILES};
use super::{CardDocument, CardKind, CardTransport};
use crate::apdu::{build_apdu, response_ok, status_word, to_hex};
use crate::atr::detect_card_document_by_atr;
use crate::pin::{pad_pin, pin_tries_left, validate_pin};
use crate::{CardError, Result};
use std::io::Read;

/// A certificate stored on the ID card.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Certificate {
    pub label: String,
    /// DER bytes.
    pub der: Vec<u8>,
}

impl Certificate {
    /// Builds a certificate from DER bytes, labelled like Bas Celik does
    /// (`CERT #` + first six digits of the decimal serial number).
    pub fn from_der(der: Vec<u8>) -> Result<Self> {
        let (_, cert) = x509_parser::parse_x509_certificate(&der)
            .map_err(|e| CardError::Parse(format!("parsing certificate: {e}")))?;
        let label = cert_short_name(&cert.tbs_certificate.serial.to_string());
        Ok(Self { label, der })
    }

    fn with_parsed<T>(&self, f: impl FnOnce(&x509_parser::certificate::X509Certificate<'_>) -> T) -> Option<T> {
        x509_parser::parse_x509_certificate(&self.der).ok().map(|(_, c)| f(&c))
    }

    /// Subject distinguished name (RFC 4514 style).
    pub fn subject(&self) -> Option<String> {
        self.with_parsed(|c| c.subject().to_string())
    }

    /// Issuer distinguished name (RFC 4514 style).
    pub fn issuer(&self) -> Option<String> {
        self.with_parsed(|c| c.issuer().to_string())
    }

    /// Common name of the subject, if present.
    pub fn subject_common_name(&self) -> Option<String> {
        self.with_parsed(|c| {
            c.subject().iter_common_name().next().and_then(|cn| cn.as_str().ok()).map(str::to_string)
        })
        .flatten()
    }

    /// Decimal serial number.
    pub fn serial_number(&self) -> Option<String> {
        self.with_parsed(|c| c.tbs_certificate.serial.to_string())
    }

    /// Start of validity as a Unix timestamp (seconds).
    pub fn not_before(&self) -> Option<i64> {
        self.with_parsed(|c| c.validity().not_before.timestamp())
    }

    /// End of validity as a Unix timestamp (seconds).
    pub fn not_after(&self) -> Option<i64> {
        self.with_parsed(|c| c.validity().not_after.timestamp())
    }
}

/// `CERT #` followed by (at most) the first six characters of the serial number.
pub fn cert_short_name(serial_number: &str) -> String {
    let short: String = serial_number.chars().take(6).collect();
    format!("CERT #{short}")
}

/// Crypto operations on (Gemalto) ID cards.
pub struct IdCardCrypto<'a> {
    pub card: &'a dyn CardTransport,
}

impl<'a> IdCardCrypto<'a> {
    /// Checks that the card is a Gemalto ID card.
    pub fn new(card: &'a dyn CardTransport) -> Result<Self> {
        let atr = card.atr()?;
        if !detect_card_document_by_atr(&atr).contains(&CardKind::GemaltoId) {
            return Err(CardError::Unsupported);
        }
        let mut g = super::Gemalto::new(card, atr);
        if !g.test() {
            return Err(CardError::Unsupported);
        }
        Ok(Self { card })
    }

    fn in_transaction<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        self.card.begin_transaction()?;
        let r = f();
        if let Err(e) = self.card.end_transaction() {
            log::warn!("ending transaction: {e}");
        }
        r
    }

    /// Number of PIN verification tries left.
    pub fn pin_tries_left(&self) -> Result<u8> {
        self.in_transaction(|| {
            init_crypto(self.card)?;
            let rsp = self.card.transmit(&build_apdu(0x00, 0x20, 0x00, 0x80, &[], 0))?;
            pin_tries_left(&rsp).ok_or_else(|| bad_status(&rsp, "getting pin tries left"))
        })
    }

    /// Changes the PIN. Returns `Err(CardError::WrongPin(tries_left))` on a wrong
    /// old PIN, `Err(CardError::PinBlocked)` when no tries are left and
    /// `Err(CardError::InvalidPin)` when a PIN has an invalid format.
    pub fn change_pin(&self, old_pin: &str, new_pin: &str) -> Result<()> {
        if !validate_pin(old_pin) || !validate_pin(new_pin) {
            return Err(CardError::InvalidPin);
        }

        self.in_transaction(|| {
            init_crypto(self.card)?;

            let rsp = self.card.transmit(&build_apdu(0x00, 0x20, 0x00, 0x80, &pad_pin(old_pin), 0))?;
            if !response_ok(&rsp) {
                return Err(pin_error(&rsp, "verifying old pin"));
            }

            let mut data = Vec::with_capacity(16);
            data.extend_from_slice(&pad_pin(old_pin));
            data.extend_from_slice(&pad_pin(new_pin));
            let rsp = self.card.transmit(&build_apdu(0x00, 0x24, 0x00, 0x80, &data, 0))?;
            if !response_ok(&rsp) {
                return Err(pin_error(&rsp, "changing pin"));
            }
            Ok(())
        })
    }

    /// Reads and decompresses the certificates stored on the card.
    /// Certificates that fail to load are skipped (and logged); an error is
    /// returned only when none could be loaded and at least one failed.
    pub fn certificates(&self) -> Result<Vec<Certificate>> {
        init_crypto(self.card)?;

        let mut certs = Vec::new();
        let mut errors = Vec::new();

        for file in CERTIFICATE_FILES {
            let name = to_hex(&file);
            match self.load_certificate(&file) {
                Ok(c) => certs.push(c),
                Err(e) => errors.push(format!("file {name}: {e}")),
            }
        }

        if !errors.is_empty() {
            let msg = errors.join("; ");
            log::warn!("loading certificates: {msg}");
            if certs.is_empty() {
                return Err(CardError::Other(format!("loading certificates: {msg}")));
            }
        }

        Ok(certs)
    }

    fn load_certificate(&self, file: &[u8]) -> Result<Certificate> {
        let rsp = read_certificate_file(self.card, file)?;
        Certificate::from_der(decompress_certificate(&rsp)?)
    }
}

/// Decompresses a certificate file as stored on the card (zlib stream at offset 6).
pub(crate) fn decompress_certificate(rsp: &[u8]) -> Result<Vec<u8>> {
    if rsp.len() < 8 {
        return Err(CardError::Parse("file too short".into()));
    }
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(&rsp[6..])
        .read_to_end(&mut out)
        .map_err(|e| CardError::Parse(format!("decompressing certificate: {e}")))?;
    Ok(out)
}

fn bad_status(rsp: &[u8], what: &str) -> CardError {
    log::debug!("{what}: response {}", to_hex(rsp));
    match status_word(rsp) {
        Some((a, b)) => CardError::BadStatus(a, b),
        None => CardError::Parse(format!("{what}: empty response")),
    }
}

fn pin_error(rsp: &[u8], what: &str) -> CardError {
    match pin_tries_left(rsp) {
        Some(0) => CardError::PinBlocked,
        Some(n) => CardError::WrongPin(n),
        None => bad_status(rsp, what),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_name() {
        assert_eq!(cert_short_name(""), "CERT #");
        assert_eq!(cert_short_name("123"), "CERT #123");
        assert_eq!(cert_short_name("123456789"), "CERT #123456");
    }

    #[test]
    fn pin_errors() {
        assert!(matches!(pin_error(&[0x63, 0xC2], ""), CardError::WrongPin(2)));
        assert!(matches!(pin_error(&[0x69, 0x83], ""), CardError::PinBlocked));
        assert!(matches!(pin_error(&[0x6A, 0x82], ""), CardError::BadStatus(0x6A, 0x82)));
    }
}
