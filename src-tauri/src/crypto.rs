//! PIN and certificate operations on the ID card in the selected reader.
//!
//! Port of bas-celik `internal/gui/crypto.go` / `pinChange.go` (MIT, (c) Nikola Ubavić).

use celik_core::{reader::PcscCard, CardTransport, Certificate, Document, IdCardCrypto};
use serde::Serialize;
use x509_parser::prelude::{FromDer, X509Certificate};

use crate::{
    error::{card_error, catch, with_detail, CmdResult},
    state::Shared,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CertInfo {
    pub index: usize,
    pub label: String,
    pub subject: String,
    pub issuer: String,
    pub not_after: String,
}

/// Mock mode: the old PIN that is treated as wrong.
pub const MOCK_WRONG_PIN: &str = "0000";

/// Subject, issuer and expiry (`YYYY-MM-DD HH:MM:SS` UTC) of a DER certificate.
pub fn describe_der(der: &[u8]) -> (String, String, String) {
    match X509Certificate::from_der(der) {
        Ok((_, cert)) => {
            let na = cert.validity().not_after.to_datetime();
            (
                cert.subject().to_string(),
                cert.issuer().to_string(),
                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                    na.year(),
                    u8::from(na.month()),
                    na.day(),
                    na.hour(),
                    na.minute(),
                    na.second()
                ),
            )
        }
        Err(e) => {
            log::warn!("cannot parse certificate: {e}");
            (String::new(), String::new(), String::new())
        }
    }
}

pub fn cert_info(index: usize, cert: &Certificate) -> CertInfo {
    let (subject, issuer, not_after) = describe_der(&cert.der);
    CertInfo {
        index,
        label: cert.label.clone(),
        subject,
        issuer,
        not_after,
    }
}

/// Check that an ID card is loaded and return the selected reader name.
fn id_card_reader(shared: &Shared) -> CmdResult<String> {
    let core = shared.core();
    match (&core.app.document, &core.app.selected_reader) {
        (Some(Document::Id(_)), Some(r)) => Ok(r.clone()),
        (_, None) => Err("error.noReader".into()),
        _ => Err("error.notIdCard".into()),
    }
}

/// Connect to the selected reader and run `f` with an [`IdCardCrypto`] inside a transaction.
fn with_crypto<T>(
    shared: &Shared,
    context: &str,
    f: impl FnOnce(&IdCardCrypto<'_>) -> celik_core::Result<T>,
) -> CmdResult<T> {
    let reader = id_card_reader(shared)?;
    catch(context, || {
        let card = PcscCard::connect(&reader).map_err(|e| card_error(context, &e))?;
        card.begin_transaction()
            .map_err(|e| card_error(context, &e))?;
        let res = IdCardCrypto::new(&card).and_then(|c| f(&c));
        let _ = card.end_transaction();
        res.map_err(|e| card_error(context, &e))
    })
}

pub fn pin_tries_left(shared: &Shared) -> CmdResult<u8> {
    if shared.mock {
        id_card_reader(shared)?;
        return Ok(3);
    }
    with_crypto(shared, "error.pin", |c| c.pin_tries_left())
}

pub fn validate_pin(pin: &str) -> CmdResult<()> {
    if pin.len() >= 4 && pin.len() <= 8 && pin.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        Err("error.invalidPin".into())
    }
}

pub fn change_pin(shared: &Shared, old_pin: &str, new_pin: &str) -> CmdResult<()> {
    validate_pin(old_pin)?;
    validate_pin(new_pin)?;
    if shared.mock {
        id_card_reader(shared)?;
        return if old_pin == MOCK_WRONG_PIN {
            Err("error.wrongPin:2".into())
        } else {
            Ok(())
        };
    }
    with_crypto(shared, "error.pinChange", |c| {
        c.change_pin(old_pin, new_pin)
    })
}

fn certificates(shared: &Shared) -> CmdResult<Vec<Certificate>> {
    if shared.mock {
        id_card_reader(shared)?;
        return Ok(Vec::new());
    }
    with_crypto(shared, "error.certificates", |c| c.certificates())
}

pub fn list_certificates(shared: &Shared) -> CmdResult<Vec<CertInfo>> {
    Ok(certificates(shared)?
        .iter()
        .enumerate()
        .map(|(i, c)| cert_info(i, c))
        .collect())
}

pub fn save_certificate(shared: &Shared, index: usize, path: &str) -> CmdResult<()> {
    let certs = certificates(shared)?;
    let cert = certs
        .get(index)
        .ok_or_else(|| with_detail("error.certificates", format!("no certificate {index}")))?;
    std::fs::write(path, &cert.der).map_err(|e| with_detail("error.saveFile", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_validation() {
        assert!(validate_pin("1234").is_ok());
        assert!(validate_pin("12345678").is_ok());
        assert!(validate_pin("123").is_err());
        assert!(validate_pin("123456789").is_err());
        assert!(validate_pin("12a4").is_err());
    }

    #[test]
    fn bad_der_is_empty() {
        let c = cert_info(
            1,
            &Certificate {
                label: "x".into(),
                der: vec![1, 2, 3],
            },
        );
        assert_eq!(c.index, 1);
        assert_eq!(c.subject, "");
    }
}
