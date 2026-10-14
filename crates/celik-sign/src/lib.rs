//! PKCS#11 module handling, XML-DSig signing and Smartbox server.
//! Port of bas-celik (MIT, (c) Nikola Ubavić) `internal/pkcs11`, `internal/xmlsign`,
//! `internal/smartbox`.

pub mod pkcs11;
pub mod smartbox;
pub mod xmlsign;
mod xmltree;

#[derive(Debug, thiserror::Error)]
pub enum SignError {
    #[error("pkcs11: {0}")]
    Pkcs11(String),
    #[error("no module loaded")]
    NoModules,
    #[error("wrong PIN")]
    WrongPin,
    #[error("xml: {0}")]
    Xml(String),
    #[error("server: {0}")]
    Server(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, SignError>;

/// Lowercase hex encoding (Go `hex.EncodeToString`).
pub fn hex_encode(data: &[u8]) -> String {
    use std::fmt::Write;
    data.iter()
        .fold(String::with_capacity(data.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Hex decoding (Go `hex.DecodeString`: case-insensitive, even length required).
pub fn hex_decode(s: &str) -> Result<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return Err(SignError::Other(
            "encoding/hex: odd length hex string".into(),
        ));
    }
    s.as_bytes()
        .chunks(2)
        .map(|pair| {
            let hi = (pair[0] as char).to_digit(16);
            let lo = (pair[1] as char).to_digit(16);
            match (hi, lo) {
                (Some(h), Some(l)) => Ok((h * 16 + l) as u8),
                _ => Err(SignError::Other(format!(
                    "encoding/hex: invalid byte: {:?}",
                    String::from_utf8_lossy(pair)
                ))),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        assert_eq!(hex_encode(&[0xab, 0x01]), "ab01");
        assert_eq!(hex_decode("AB01").unwrap(), vec![0xab, 0x01]);
        assert_eq!(hex_decode("").unwrap(), Vec::<u8>::new());
        assert!(hex_decode("abc").is_err());
        assert!(hex_decode("zz").is_err());
    }
}
