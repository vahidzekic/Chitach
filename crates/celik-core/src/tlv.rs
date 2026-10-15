//! Simple TLV format used by Serbian ID and medical cards
//! (two-byte little-endian tag, two-byte little-endian length, value).

use crate::error::FormatError;
use std::collections::HashMap;
use std::hash::Hash;

/// Parsed TLV records: tag -> value.
pub type TlvFields = HashMap<u32, Vec<u8>>;

/// Parses simple TLV-encoded data and returns a map of tags to values.
pub fn parse_tlv(data: &[u8]) -> Result<TlvFields, FormatError> {
    if data.is_empty() {
        return Err(FormatError::InvalidLength);
    }

    let mut m = HashMap::new();
    let mut offset = 0usize;

    loop {
        if data.len() <= offset + 4 {
            return Err(FormatError::InvalidLength);
        }

        let tag = u16::from_le_bytes([data[offset], data[offset + 1]]) as u32;
        let length = u16::from_le_bytes([data[offset + 2], data[offset + 3]]) as usize;
        offset += 4;

        if offset + length > data.len() {
            return Err(FormatError::InvalidLength);
        }

        m.insert(tag, data[offset..offset + length].to_vec());
        offset += length;

        if offset >= data.len() {
            break;
        }
    }

    Ok(m)
}

/// Assigns the value of `tag` to `target`; an absent tag yields an empty string.
pub fn assign_field<T: Eq + Hash>(fields: &HashMap<T, Vec<u8>>, tag: T, target: &mut String) {
    *target = match fields.get(&tag) {
        Some(v) => String::from_utf8_lossy(v).into_owned(),
        None => String::new(),
    };
}

/// Assigns `true` iff the value of `tag` is exactly `0x31` (`'1'`).
pub fn assign_bool_field<T: Eq + Hash>(fields: &HashMap<T, Vec<u8>>, tag: T, target: &mut bool) {
    *target = matches!(fields.get(&tag), Some(v) if v.as_slice() == [0x31]);
}

/// Encodes a single TLV record (helper for tests and virtual cards).
pub fn encode_record(tag: u16, value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + value.len());
    out.extend_from_slice(&tag.to_le_bytes());
    out.extend_from_slice(&(value.len() as u16).to_le_bytes());
    out.extend_from_slice(value);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(parse_tlv(&[]), Err(FormatError::InvalidLength));

        let r = parse_tlv(&[
            0x01, 0x00, 0x05, 0x00, 0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x09, 0x00, 0x05, 0x00, 0x57,
            0x6F, 0x72, 0x6C, 0x64,
        ])
        .unwrap();
        let expected: TlvFields = [(9, b"World".to_vec()), (1, b"Hello".to_vec())]
            .into_iter()
            .collect();
        assert_eq!(r, expected);

        assert_eq!(
            parse_tlv(&[0x01, 0x00, 0x05, 0x00, 0x48, 0x65]),
            Err(FormatError::InvalidLength)
        );
        assert_eq!(parse_tlv(&[0x02, 0x00]), Err(FormatError::InvalidLength));
        assert_eq!(
            parse_tlv(&[0x02, 0x00, 0x01, 0x00]),
            Err(FormatError::InvalidLength)
        );

        let r = parse_tlv(&[0x02, 0x00, 0x01, 0x00, 0x01]).unwrap();
        assert_eq!(r, [(2, vec![0x01])].into_iter().collect::<TlvFields>());
    }

    #[test]
    fn assign() {
        let mut target = "x".to_string();
        let mut fields: HashMap<u32, Vec<u8>> = HashMap::new();
        fields.insert(0, vec![]);
        assign_field(&fields, 0, &mut target);
        assert_eq!(target, "");

        let mut fields: HashMap<u32, Vec<u8>> = HashMap::new();
        fields.insert(5, b"Hello".to_vec());
        assign_field(&fields, 5, &mut target);
        assert_eq!(target, "Hello");

        let fields: HashMap<u32, Vec<u8>> = HashMap::new();
        assign_field(&fields, 0, &mut target);
        assert_eq!(target, "");
    }

    #[test]
    fn assign_bool() {
        for (value, expected) in [(vec![0x31u8], true), (vec![0x01], false), (vec![], false)] {
            let mut target = !expected;
            let fields: HashMap<u32, Vec<u8>> = [(0, value)].into_iter().collect();
            assign_bool_field(&fields, 0, &mut target);
            assert_eq!(target, expected);
        }
    }

    #[test]
    fn encode() {
        assert_eq!(
            encode_record(0x0102, b"ab"),
            vec![0x02, 0x01, 0x02, 0x00, b'a', b'b']
        );
        let data = [encode_record(1, b"x"), encode_record(1546, b"yz")].concat();
        let f = parse_tlv(&data).unwrap();
        assert_eq!(f[&1546], b"yz");
    }
}
