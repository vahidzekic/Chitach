//! Serbian medical insurance cards (RFZO).
//!
//! Port of bas-celik `card/medical.go` (MIT, (c) Nikola Ubavić).

use super::{read, read_chunks, select, CardDocument, CardKind, CardTransport};
use crate::apdu::build_apdu;
use crate::document::{Document, MedicalDocument};
use crate::localization::format_date;
use crate::tlv::{assign_bool_field, assign_field, parse_tlv, TlvFields};
use crate::{CardError, Result};

/// AID of the medical application (`SERVSZK`).
pub const AID_MEDICAL: &[u8] = &[
    0xF3, 0x81, 0x00, 0x00, 0x02, 0x53, 0x45, 0x52, 0x56, 0x53, 0x5A, 0x4B, 0x01,
];

/// Location of the file with document data.
pub const MED_DOCUMENT_FILE_LOC: [u8; 2] = [0x0D, 0x01];
/// Location of the file with fixed personal data.
pub const MED_FIXED_PERSONAL_FILE_LOC: [u8; 2] = [0x0D, 0x02];
/// Location of the file with variable personal data.
pub const MED_VARIABLE_PERSONAL_FILE_LOC: [u8; 2] = [0x0D, 0x03];
/// Location of the file with variable administrative data.
pub const MED_VARIABLE_ADMIN_FILE_LOC: [u8; 2] = [0x0D, 0x04];

/// Name of the insurer, used to recognise medical cards sharing an ATR with ID cards.
pub const RFZO_NAME: &str = "Републички фонд за здравствено осигурање";

/// A smart card holding a Serbian medical insurance document.
pub struct MedicalCard<'a> {
    atr: Vec<u8>,
    card: &'a dyn CardTransport,
    files: [Vec<u8>; 4],
}

impl<'a> MedicalCard<'a> {
    pub fn new(card: &'a dyn CardTransport, atr: Vec<u8>) -> Self {
        Self {
            atr,
            card,
            files: Default::default(),
        }
    }

    /// Reads a file: 4 byte header with the little-endian data length at offset 2.
    pub fn read_file(&self, name: &[u8]) -> Result<Vec<u8>> {
        select(self.card, 0x00, 0x00, name, 0)?;

        let data = read(self.card, 0, 4)?;
        if data.len() < 4 {
            return Err(CardError::Parse("file too short".into()));
        }
        let length = u16::from_le_bytes([data[2], data[3]]) as usize;
        read_chunks(self.card, data.len(), length, 0xFF)
    }
}

impl CardDocument for MedicalCard<'_> {
    fn kind(&self) -> CardKind {
        CardKind::Medical
    }
    fn atr(&self) -> &[u8] {
        &self.atr
    }
    fn init_card(&mut self) -> Result<()> {
        let rsp = self
            .card
            .transmit(&build_apdu(0x00, 0xA4, 0x04, 0x00, AID_MEDICAL, 0))?;
        super::check_ok(&rsp, "initializing card")
    }
    fn read_card(&mut self) -> Result<()> {
        let names = [
            (MED_DOCUMENT_FILE_LOC, "reading document file"),
            (MED_FIXED_PERSONAL_FILE_LOC, "reading fixed personal file"),
            (
                MED_VARIABLE_PERSONAL_FILE_LOC,
                "reading variable personal file",
            ),
            (
                MED_VARIABLE_ADMIN_FILE_LOC,
                "reading variable administrative file",
            ),
        ];
        let mut files: [Vec<u8>; 4] = Default::default();
        for (i, (name, what)) in names.iter().enumerate() {
            files[i] = self.read_file(name).map_err(|e| super::context(e, what))?;
        }
        self.files = files;
        Ok(())
    }
    fn get_document(&self) -> Result<Document> {
        let [a, b, c, d] = &self.files;
        Ok(Document::Medical(Box::new(parse_medical_document(
            a, b, c, d,
        )?)))
    }
    /// Newer medical cards share ATR with the ID cards (GEMALTO_ATR_2).
    fn test(&mut self) -> bool {
        if self
            .card
            .transmit(&build_apdu(0x00, 0xA4, 0x04, 0x00, AID_MEDICAL, 0))
            .is_err()
        {
            return false;
        }
        let Ok(rsp) = self.read_file(&MED_DOCUMENT_FILE_LOC) else {
            return false;
        };
        let Ok(mut fields) = parse_tlv(&rsp) else {
            return false;
        };
        descramble(&mut fields, 1553);
        fields
            .get(&1553)
            .is_some_and(|v| v.as_slice() == RFZO_NAME.as_bytes())
    }
}

/// Decodes UTF-16 (little endian, BOM honoured) text fields of medical cards
/// into UTF-8 in place. A missing tag becomes an empty value.
pub fn descramble(fields: &mut TlvFields, tag: u32) {
    let decoded = fields
        .get(&tag)
        .map(|bs| decode_utf16(bs))
        .unwrap_or_default();
    fields.insert(tag, decoded.into_bytes());
}

fn decode_utf16(bs: &[u8]) -> String {
    let (big_endian, body) = match bs {
        [0xFF, 0xFE, rest @ ..] => (false, rest),
        [0xFE, 0xFF, rest @ ..] => (true, rest),
        _ => (false, bs),
    };
    let units = body.chunks(2).map(|c| match (c, big_endian) {
        ([a, b], false) => u16::from_le_bytes([*a, *b]),
        ([a, b], true) => u16::from_be_bytes([*a, *b]),
        _ => 0xFFFD,
    });
    char::decode_utf16(units)
        .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

pub fn parse_medical_document_file(data: &[u8], doc: &mut MedicalDocument) -> Result<()> {
    let mut f = parse_tlv(data)?;
    descramble(&mut f, 1553);
    assign_field(&f, 1553, &mut doc.insurer_name);
    assign_field(&f, 1554, &mut doc.insurer_id);
    assign_field(&f, 1555, &mut doc.card_id);
    assign_field(&f, 1557, &mut doc.date_of_issue);
    format_date(&mut doc.date_of_issue);
    assign_field(&f, 1558, &mut doc.date_of_expiry);
    format_date(&mut doc.date_of_expiry);
    assign_field(&f, 1559, &mut doc.chip_serial_number);
    assign_field(&f, 1560, &mut doc.print_language);
    Ok(())
}

pub fn parse_medical_fixed_personal_file(data: &[u8], doc: &mut MedicalDocument) -> Result<()> {
    let mut f = parse_tlv(data)?;
    assign_field(&f, 1569, &mut doc.insurant_number);
    descramble(&mut f, 1570);
    assign_field(&f, 1570, &mut doc.family_name);
    descramble(&mut f, 1571);
    assign_field(&f, 1571, &mut doc.family_name_latin);
    descramble(&mut f, 1572);
    assign_field(&f, 1572, &mut doc.given_name);
    descramble(&mut f, 1573);
    assign_field(&f, 1573, &mut doc.given_name_latin);
    assign_field(&f, 1574, &mut doc.date_of_birth);
    format_date(&mut doc.date_of_birth);
    Ok(())
}

pub fn parse_medical_variable_personal_file(data: &[u8], doc: &mut MedicalDocument) -> Result<()> {
    let f = parse_tlv(data)?;
    assign_field(&f, 1586, &mut doc.valid_until);
    format_date(&mut doc.valid_until);
    assign_bool_field(&f, 1587, &mut doc.permanently_valid);
    Ok(())
}

pub fn parse_medical_variable_admin_file(data: &[u8], doc: &mut MedicalDocument) -> Result<()> {
    let mut f = parse_tlv(data)?;
    let utf16 = |f: &mut TlvFields, tag: u32, target: &mut String| {
        descramble(f, tag);
        assign_field(f, tag, target);
    };
    utf16(&mut f, 1601, &mut doc.parent_name);
    utf16(&mut f, 1602, &mut doc.parent_name_latin);
    doc.gender = if f.get(&1603).map(Vec::as_slice) == Some(b"01") {
        "Мушко"
    } else {
        "Женско"
    }
    .to_string();
    assign_field(&f, 1604, &mut doc.personal_number);
    utf16(&mut f, 1605, &mut doc.street);
    assign_field(&f, 1606, &mut doc.post_number);
    utf16(&mut f, 1607, &mut doc.municipality);
    utf16(&mut f, 1608, &mut doc.place);
    assign_field(&f, 1609, &mut doc.street_code);
    utf16(&mut f, 1610, &mut doc.number);
    utf16(&mut f, 1611, &mut doc.entrance);
    utf16(&mut f, 1612, &mut doc.apartment);
    assign_field(&f, 1614, &mut doc.insurance_basis_rzzo);
    utf16(&mut f, 1615, &mut doc.insurance_description);
    utf16(&mut f, 1616, &mut doc.carrier_relationship);
    assign_bool_field(&f, 1617, &mut doc.carrier_family_member);
    assign_field(&f, 1618, &mut doc.carrier_id_number);
    assign_field(&f, 1619, &mut doc.carrier_insurant_number);
    utf16(&mut f, 1620, &mut doc.carrier_family_name);
    utf16(&mut f, 1621, &mut doc.carrier_family_name_latin);
    utf16(&mut f, 1622, &mut doc.carrier_given_name);
    utf16(&mut f, 1623, &mut doc.carrier_given_name_latin);
    assign_field(&f, 1624, &mut doc.insurance_start_date);
    format_date(&mut doc.insurance_start_date);
    utf16(&mut f, 1626, &mut doc.country);
    utf16(&mut f, 1630, &mut doc.taxpayer_name);
    utf16(&mut f, 1631, &mut doc.taxpayer_residence);
    assign_field(&f, 1632, &mut doc.taxpayer_id_number);
    if doc.taxpayer_id_number.is_empty() {
        assign_field(&f, 1633, &mut doc.taxpayer_id_number);
    }
    assign_field(&f, 1634, &mut doc.taxpayer_activity_code);
    Ok(())
}

/// Parses the four medical card files.
pub fn parse_medical_document(
    document_file: &[u8],
    fixed_personal_file: &[u8],
    variable_personal_file: &[u8],
    variable_admin_file: &[u8],
) -> Result<MedicalDocument> {
    let mut doc = MedicalDocument::default();
    parse_medical_document_file(document_file, &mut doc)
        .map_err(|e| super::context(e, "parsing document file"))?;
    parse_medical_fixed_personal_file(fixed_personal_file, &mut doc)
        .map_err(|e| super::context(e, "parsing fixed personal file"))?;
    parse_medical_variable_personal_file(variable_personal_file, &mut doc)
        .map_err(|e| super::context(e, "parsing variable personal file"))?;
    parse_medical_variable_admin_file(variable_admin_file, &mut doc)
        .map_err(|e| super::context(e, "parsing variable administrative file"))?;
    Ok(doc)
}

/// Encodes text the way medical cards store it (UTF-16 LE, no BOM). Helper for tests and virtual cards.
pub fn encode_utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn descramble_cases() {
        let cases = [
            (
                "200435043f04430431043b04380447043a043804200044043e043d04340420003704300420003704340440043004320441044204320435043d043e0420003e044104380433044304400430045a043504",
                "Републички фонд за здравствено осигурање",
            ),
            ("210440043104380458043004", "Србија"),
            ("110415041e041304200410041404", "БЕОГРАД"),
            (
                "170430043f043e0441043b0435043d0438042000430420003f044004380432044004350434043d043e043c04200034044004430448044204320443042c00200034044004430433043e043c0420003f044004300432043d043e043c0420003b043804460443042c0020003a043e04340420003f0440043504340443043704350442043d0438043a0430042c00200046043804320438043b043d04300420003b0438044604300420003d043004200041043b04430436043104380420004304200032043e045804410446043804",
                "Запослени у привредном друштву, другом правном лицу, код предузетника, цивилна лица на служби у војсци",
            ),
            (
                "110443045f04350442042000200435043f04430431043b0438043a0435042000210440043104380458043504",
                "Буџет Републике Србије",
            ),
        ];
        for (i, (data, expected)) in cases.iter().enumerate() {
            let mut fields = TlvFields::new();
            fields.insert(i as u32, unhex(data));
            descramble(&mut fields, i as u32);
            assert_eq!(
                String::from_utf8(fields[&(i as u32)].clone()).unwrap(),
                *expected
            );
        }

        let mut fields = TlvFields::new();
        descramble(&mut fields, 1);
        assert_eq!(fields[&1], Vec::<u8>::new());
    }

    #[test]
    fn bom() {
        assert_eq!(decode_utf16(&[0xFF, 0xFE, 0x41, 0x00]), "A");
        assert_eq!(decode_utf16(&[0xFE, 0xFF, 0x00, 0x41]), "A");
        assert_eq!(decode_utf16(&encode_utf16("Ђорђе")), "Ђорђе");
    }
}
