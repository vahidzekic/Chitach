//! PIN helpers. Port of bas-celik `card/pin.go` (MIT, (c) Nikola Ubavić).

/// Checks that the PIN consists only of (ASCII) digits and that its length is between 4 and 8.
pub fn validate_pin(pin: &str) -> bool {
    (4..=8).contains(&pin.len()) && pin.bytes().all(|b| b.is_ascii_digit())
}

/// Creates an 8 byte array containing the PIN at the beginning, zero padded.
pub fn pad_pin(pin: &str) -> [u8; 8] {
    let mut data = [0u8; 8];
    for (d, b) in data.iter_mut().zip(pin.bytes()) {
        *d = b;
    }
    data
}

/// Number of PIN tries left encoded in a VERIFY / CHANGE REFERENCE DATA status word.
/// Returns `None` if the response doesn't carry that information.
pub fn pin_tries_left(rsp: &[u8]) -> Option<u8> {
    match rsp {
        [0x63, 0xC0] | [0x69, 0x83] => Some(0),
        [0x63, 0xC1] => Some(1),
        [0x63, 0xC2] => Some(2),
        [0x63, 0xC3] => Some(3),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate() {
        for (pin, expected) in [
            ("", false),
            ("1", false),
            ("123", false),
            ("1234", true),
            ("dddd", false),
            ("123a", false),
            ("12345678", true),
            ("123456789", false),
        ] {
            assert_eq!(validate_pin(pin), expected, "{pin}");
        }
    }

    #[test]
    fn pad() {
        assert_eq!(pad_pin("1234"), [0x31, 0x32, 0x33, 0x34, 0, 0, 0, 0]);
        assert_eq!(pad_pin(""), [0; 8]);
        assert_eq!(
            pad_pin("12345678"),
            [0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38]
        );
        assert_eq!(
            pad_pin("123456789"),
            [0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38]
        );
    }

    #[test]
    fn tries_left() {
        let cases: [(&[u8], Option<u8>); 9] = [
            (&[0x63, 0xC0], Some(0)),
            (&[0x69, 0x83], Some(0)),
            (&[0x63, 0xC1], Some(1)),
            (&[0x63, 0xC2], Some(2)),
            (&[0x63, 0xC3], Some(3)),
            (&[], None),
            (&[0x63], None),
            (&[0x63, 0xAA], None),
            (&[0x63, 0xC2, 0x00], None),
        ];
        for (rsp, expected) in cases {
            assert_eq!(pin_tries_left(rsp), expected, "{rsp:02X?}");
        }
    }
}
