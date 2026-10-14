//! APDU construction and response helpers.
//!
//! Port of bas-celik `card/apdu.go` and parts of `card/card.go`
//! (MIT, (c) Nikola Ubavić).

/// Constructs an APDU (Application Protocol Data Unit) command according to
/// ISO 7816-4 (5. Organization for interchange).
///
/// `ne` is the expected response length (0 = no Le field, 256 = short `00`,
/// 65536 = extended `00 00`).
///
/// # Panics
/// Panics if `data` is longer than 0xFFFF bytes (same as Bas Celik).
pub fn build_apdu(cla: u8, ins: u8, p1: u8, p2: u8, data: &[u8], ne: usize) -> Vec<u8> {
    let length = data.len();
    assert!(length <= 0xFFFF, "APDU command length too large");

    let mut apdu = Vec::with_capacity(4 + 3 + length + 2);
    apdu.extend_from_slice(&[cla, ins, p1, p2]);

    if length == 0 {
        if ne != 0 {
            if ne <= 256 {
                apdu.push(if ne == 256 { 0x00 } else { ne as u8 });
            } else if ne == 65536 {
                apdu.extend_from_slice(&[0x00, 0x00]);
            } else {
                apdu.extend_from_slice(&[(ne >> 8) as u8, ne as u8]);
            }
        }
    } else if ne == 0 {
        if length <= 255 {
            apdu.push(length as u8);
        } else {
            apdu.extend_from_slice(&[0x00, (length >> 8) as u8, length as u8]);
        }
        apdu.extend_from_slice(data);
    } else if length <= 255 && ne <= 256 {
        apdu.push(length as u8);
        apdu.extend_from_slice(data);
        apdu.push(if ne == 256 { 0x00 } else { ne as u8 });
    } else {
        apdu.extend_from_slice(&[0x00, (length >> 8) as u8, length as u8]);
        apdu.extend_from_slice(data);
        if ne != 65536 {
            apdu.extend_from_slice(&[(ne >> 8) as u8, ne as u8]);
        }
    }

    apdu
}

/// Checks if the card response ends with the `90 00` status word.
pub fn response_ok(rsp: &[u8]) -> bool {
    rsp.len() >= 2 && rsp[rsp.len() - 2..] == [0x90, 0x00]
}

/// Returns the status word (SW1, SW2) of a response, if present.
pub fn status_word(rsp: &[u8]) -> Option<(u8, u8)> {
    if rsp.len() < 2 {
        None
    } else {
        Some((rsp[rsp.len() - 2], rsp[rsp.len() - 1]))
    }
}

/// Lowercase hex encoding (same as Go's `hex.EncodeToString`).
pub fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build() {
        let long_data: Vec<u8> = (0..0x100).map(|i| i as u8).collect();
        let p = [0x00, 0xA4, 0x04, 0x01];
        let with = |prefix: &[u8], data: &[u8], suffix: &[u8]| {
            let mut v = prefix.to_vec();
            v.extend_from_slice(data);
            v.extend_from_slice(suffix);
            v
        };
        let cases: Vec<(&[u8], usize, Vec<u8>)> = vec![
            (&[], 0, vec![0x00, 0xA4, 0x04, 0x01]),
            (&[], 0xFF, vec![0x00, 0xA4, 0x04, 0x01, 0xFF]),
            (&[], 0x01FF, vec![0x00, 0xA4, 0x04, 0x01, 0x01, 0xFF]),
            (&[], 0x10000, vec![0x00, 0xA4, 0x04, 0x01, 0x00, 0x00]),
            (&[0, 0, 0], 0, vec![0x00, 0xA4, 0x04, 0x01, 0x03, 0x00, 0x00, 0x00]),
            (&[0, 0, 0], 0x01, vec![0x00, 0xA4, 0x04, 0x01, 0x03, 0x00, 0x00, 0x00, 0x01]),
            (&[0, 0, 0], 0x100, vec![0x00, 0xA4, 0x04, 0x01, 0x03, 0x00, 0x00, 0x00, 0x00]),
            (&long_data, 0, with(&[0x00, 0xA4, 0x04, 0x01, 0x00, 0x01, 0x00], &long_data, &[])),
            (&long_data, 0x01FF, with(&[0x00, 0xA4, 0x04, 0x01, 0x00, 0x01, 0x00], &long_data, &[0x01, 0xFF])),
        ];
        for (i, (data, ne, expected)) in cases.into_iter().enumerate() {
            assert_eq!(build_apdu(p[0], p[1], p[2], p[3], data, ne), expected, "case {i}");
        }
    }

    #[test]
    fn response() {
        let cases: [(&[u8], bool); 5] = [
            (&[0x0F, 0x0F], false),
            (&[0x90, 0x00], true),
            (&[0x01, 0xFF, 0x90, 0x00], true),
            (&[0x01, 0xFF, 0x00, 0x00], false),
            (&[0xA1], false),
        ];
        for (v, r) in cases {
            assert_eq!(response_ok(v), r, "{v:?}");
        }
    }

    #[test]
    fn hex() {
        assert_eq!(to_hex(&[0x3B, 0x0a, 0xff]), "3b0aff");
        assert_eq!(status_word(&[1, 0x90, 0x00]), Some((0x90, 0)));
        assert_eq!(status_word(&[1]), None);
    }
}
