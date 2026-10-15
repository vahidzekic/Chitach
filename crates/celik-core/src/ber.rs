//! BER-TLV parser (ISO/IEC 7816-4) used by vehicle registration cards.
//!
//! Port of bas-celik `card/ber` (MIT, (c) Nikola Ubavić).

use crate::error::FormatError;
use std::fmt;

/// A node (or a tree) of a BER structure. Leaf nodes are *primitive* and
/// carry data; constructed nodes only carry children.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ber {
    /// Complete tag of the node (0 for the root).
    pub tag: u32,
    /// Denotes if the node is a leaf.
    pub primitive: bool,
    /// Data of a leaf node.
    pub data: Vec<u8>,
    /// Children of a constructed node.
    pub children: Vec<Ber>,
}

/// Error produced by BER tree operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BerError {
    #[error(transparent)]
    Format(#[from] FormatError),
    #[error("can't add a value into primitive value")]
    AddIntoPrimitive,
    #[error("types don't match")]
    TypesMismatch,
    #[error("tags don't match")]
    TagsMismatch,
}

impl From<BerError> for crate::CardError {
    fn from(e: BerError) -> Self {
        crate::CardError::Parse(e.to_string())
    }
}

/// Parses BER data (described in ISO/IEC 7816-4 (2005)) into a tree with root tag 0.
pub fn parse_ber(data: &[u8]) -> Result<Ber, BerError> {
    let (primitive, constructed) = parse_ber_layer(data)?;

    let mut ber = Ber::default();

    for (tag, value) in primitive {
        ber.add(Ber {
            tag,
            primitive: true,
            data: value.to_vec(),
            children: Vec::new(),
        })?;
    }

    for (tag, value) in constructed {
        let sub = parse_ber(value)?;
        ber.add(Ber {
            tag,
            primitive: false,
            data: Vec::new(),
            children: sub.children,
        })?;
    }

    Ok(ber)
}

/// Inserts `(tag, value)` replacing an existing entry with the same tag
/// (mirrors the Go map semantics: the last record with a tag wins).
fn upsert<'a>(v: &mut Vec<(u32, &'a [u8])>, tag: u32, value: &'a [u8]) {
    if let Some(e) = v.iter_mut().find(|(t, _)| *t == tag) {
        e.1 = value;
    } else {
        v.push((tag, value));
    }
}

type Layer<'a> = (Vec<(u32, &'a [u8])>, Vec<(u32, &'a [u8])>);

/// Parses one level of BER-TLV encoded data. Returns primitive and constructed fields.
fn parse_ber_layer(data: &[u8]) -> Result<Layer<'_>, FormatError> {
    let mut prim = Vec::new();
    let mut cons = Vec::new();
    let mut offset = 0usize;

    loop {
        let (tag, primitive, delta) = parse_tag(&data[offset..])?;
        offset += delta;

        let (length, delta) = parse_length(&data[offset..])?;
        let length = length as usize;
        offset += delta;

        if length > data.len() - offset {
            return Err(FormatError::InvalidLength);
        }
        let value = &data[offset..offset + length];

        if primitive {
            upsert(&mut prim, tag, value);
        } else {
            upsert(&mut cons, tag, value);
        }

        offset += length;

        if offset == data.len() {
            break;
        }
    }

    Ok((prim, cons))
}

impl Ber {
    /// Accesses a node's data by address (list of tags from the root).
    pub fn access(&self, address: &[u32]) -> Option<&[u8]> {
        match address.split_first() {
            None => Some(&self.data),
            Some((first, rest)) => self.children.iter().find(|c| c.tag == *first)?.access(rest),
        }
    }

    /// Recursively inserts a node. Nodes with the same tag and kind are merged
    /// (primitive ones are replaced); same tag with a different kind is an error.
    pub fn add(&mut self, new: Ber) -> Result<(), BerError> {
        if self.primitive {
            return Err(BerError::AddIntoPrimitive);
        }

        match self.children.iter_mut().find(|c| c.tag == new.tag) {
            None => {
                self.children.push(new);
                Ok(())
            }
            Some(target) => {
                if target.primitive != new.primitive {
                    return Err(BerError::TypesMismatch);
                }
                if target.primitive {
                    *target = new;
                } else {
                    for child in new.children {
                        target.add(child)?;
                    }
                }
                Ok(())
            }
        }
    }

    /// Merges two trees by adding all nodes of `new` into `self`.
    pub fn merge(&mut self, new: Ber) -> Result<(), BerError> {
        if self.tag != new.tag {
            return Err(BerError::TagsMismatch);
        }
        for c in new.children {
            self.add(c)?;
        }
        Ok(())
    }

    /// Assigns the data at `address` to `target` if the node exists
    /// (leaves `target` untouched otherwise).
    pub fn assign_from(&self, target: &mut String, address: &[u32]) {
        if let Some(bytes) = self.access(address) {
            *target = String::from_utf8_lossy(bytes).into_owned();
        }
    }

    fn levels(&self) -> Vec<String> {
        if self.primitive {
            vec![format!(
                "{:X}: {}",
                self.tag,
                String::from_utf8_lossy(&self.data)
            )]
        } else {
            let mut out = vec![format!("{}:", self.tag)];
            for child in &self.children {
                out.extend(child.levels().into_iter().map(|l| format!("  {l}")));
            }
            out
        }
    }
}

impl fmt::Display for Ber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.levels().join("\n"))
    }
}

/// Parses the length of a field (ISO 7816-4). Returns `(length, parsed bytes)`.
pub fn parse_length(data: &[u8]) -> Result<(u32, usize), FormatError> {
    let first = *data.first().ok_or(FormatError::InvalidLength)?;
    match first {
        b if b < 0x80 => Ok((b as u32, 1)),
        0x80 => Err(FormatError::InvalidFormat),
        0x81 if data.len() >= 2 => Ok((data[1] as u32, 2)),
        0x82 if data.len() >= 3 => Ok((u16::from_be_bytes([data[1], data[2]]) as u32, 3)),
        0x83 if data.len() >= 4 => Ok((u32::from_be_bytes([0, data[1], data[2], data[3]]), 4)),
        0x84 if data.len() >= 5 => {
            Ok((u32::from_be_bytes([data[1], data[2], data[3], data[4]]), 5))
        }
        _ => Err(FormatError::InvalidLength),
    }
}

/// Parses the tag of a field (ISO 7816-4). Returns `(tag, primitive, parsed bytes)`.
pub fn parse_tag(data: &[u8]) -> Result<(u32, bool, usize), FormatError> {
    let first = *data.first().ok_or(FormatError::InvalidLength)?;
    let primitive = first & 0b10_0000 == 0;

    if first & 0x1F != 0x1F {
        Ok((first as u32, primitive, 1))
    } else if data.len() >= 2 && data[1] & 0x80 == 0 {
        Ok((u16::from_be_bytes([data[0], data[1]]) as u32, primitive, 2))
    } else if data.len() >= 3 {
        Ok((
            (data[0] as u32) << 16 | (data[1] as u32) << 8 | data[2] as u32,
            primitive,
            3,
        ))
    } else {
        Err(FormatError::InvalidLength)
    }
}

/// Encodes a single BER-TLV record (helper for tests and virtual cards).
pub fn encode_record(tag: &[u8], value: &[u8]) -> Vec<u8> {
    let mut out = tag.to_vec();
    let n = value.len();
    if n < 0x80 {
        out.push(n as u8);
    } else if n < 0x100 {
        out.extend_from_slice(&[0x81, n as u8]);
    } else {
        out.extend_from_slice(&[0x82, (n >> 8) as u8, n as u8]);
    }
    out.extend_from_slice(value);
    out
}

#[cfg(test)]
#[allow(clippy::type_complexity)]
mod tests {
    use super::*;
    use FormatError::*;

    #[test]
    fn length() {
        let cases: Vec<(&[u8], Result<(u32, usize), FormatError>)> = vec![
            (&[], Err(InvalidLength)),
            (&[0x79], Ok((0x79, 1))),
            (&[0x80, 0x91], Err(InvalidFormat)),
            (&[0x81, 0x01], Ok((0x01, 2))),
            (&[0x81], Err(InvalidLength)),
            (&[0x82, 0x01, 0x02], Ok((0x0102, 3))),
            (&[0x82], Err(InvalidLength)),
            (&[0x83, 0x01, 0x02, 0x03], Ok((0x010203, 4))),
            (&[0x83], Err(InvalidLength)),
            (&[0x84, 0x01, 0x02, 0x03, 0x04], Ok((0x01020304, 5))),
            (&[0x84], Err(InvalidLength)),
        ];
        for (data, expected) in cases {
            assert_eq!(parse_length(data), expected, "{data:02X?}");
        }
    }

    #[test]
    fn tag() {
        let cases: Vec<(&[u8], Result<(u32, bool, usize), FormatError>)> = vec![
            (&[], Err(InvalidLength)),
            (&[0b000001], Ok((0b000001, true, 1))),
            (&[0b00100001], Ok((0b00100001, false, 1))),
            (&[0b10111111, 0b00101111], Ok((0xBF2F, false, 2))),
            (&[0b10111111, 0b10101111], Err(InvalidLength)),
            (
                &[0b10111111, 0b10101111, 0b11010101],
                Ok((0xBFAFD5, false, 3)),
            ),
        ];
        for (data, expected) in cases {
            assert_eq!(parse_tag(data), expected, "{data:02X?}");
        }
    }

    #[test]
    fn parse_and_access() {
        let data = encode_record(
            &[0x71],
            &[
                encode_record(&[0x81], b"BG123AB"),
                encode_record(
                    &[0xA1],
                    &encode_record(&[0xA2], &encode_record(&[0x83], b"Name")),
                ),
                encode_record(&[0x9F, 0x33], b"SRB"),
            ]
            .concat(),
        );
        let ber = parse_ber(&data).unwrap();
        assert_eq!(ber.access(&[0x71, 0x81]), Some(&b"BG123AB"[..]));
        assert_eq!(ber.access(&[0x71, 0xA1, 0xA2, 0x83]), Some(&b"Name"[..]));
        assert_eq!(ber.access(&[0x71, 0x9F33]), Some(&b"SRB"[..]));
        assert_eq!(ber.access(&[0x72]), None);

        let mut s = "keep".to_string();
        ber.assign_from(&mut s, &[0x71, 0x99]);
        assert_eq!(s, "keep");
        ber.assign_from(&mut s, &[0x71, 0x81]);
        assert_eq!(s, "BG123AB");
        assert!(ber.to_string().contains("81: BG123AB"));
    }

    #[test]
    fn merge() {
        let a = encode_record(
            &[0x71],
            &[encode_record(&[0x81], b"A"), encode_record(&[0x82], b"B")].concat(),
        );
        let b = encode_record(
            &[0x71],
            &[
                encode_record(&[0x82], b"C"),
                encode_record(&[0xA3], &encode_record(&[0x87], b"D")),
            ]
            .concat(),
        );
        let mut t = parse_ber(&a).unwrap();
        t.merge(parse_ber(&b).unwrap()).unwrap();
        assert_eq!(t.access(&[0x71, 0x81]), Some(&b"A"[..]));
        assert_eq!(t.access(&[0x71, 0x82]), Some(&b"C"[..]));
        assert_eq!(t.access(&[0x71, 0xA3, 0x87]), Some(&b"D"[..]));

        // Same tag, different kind.
        let leaf = Ber {
            tag: 0x81,
            primitive: false,
            ..Default::default()
        };
        assert_eq!(t.children[0].add(leaf), Err(BerError::TypesMismatch));

        let mut other = Ber {
            tag: 5,
            ..Default::default()
        };
        assert_eq!(other.merge(t.clone()), Err(BerError::TagsMismatch));
        let mut leaf = Ber {
            primitive: true,
            ..Default::default()
        };
        assert_eq!(leaf.add(Ber::default()), Err(BerError::AddIntoPrimitive));
    }

    #[test]
    fn invalid() {
        assert!(parse_ber(&[]).is_err());
        assert!(parse_ber(&[0x81, 0x05, 0x01]).is_err());
        assert!(parse_ber(&[0x81, 0x01, 0x01, 0x82]).is_err());
    }
}
