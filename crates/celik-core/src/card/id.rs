//! Parsing of ID card files (shared by Apollo and Gemalto cards).
//!
//! Port of bas-celik `card/idCard.go` (MIT, (c) Nikola Ubavić).

use crate::document::IdDocument;
use crate::localization::format_date;
use crate::tlv::{assign_field, parse_tlv};
use crate::{CardError, Result};

/// Location of the file with document data.
pub const ID_DOCUMENT_FILE_LOC: [u8; 2] = [0x0F, 0x02];
/// Location of the file with personal data.
pub const ID_PERSONAL_FILE_LOC: [u8; 2] = [0x0F, 0x03];
/// Location of the file with residence data.
pub const ID_RESIDENCE_FILE_LOC: [u8; 2] = [0x0F, 0x04];
/// Location of the portrait (JPEG).
pub const ID_PHOTO_FILE_LOC: [u8; 2] = [0x0F, 0x06];

pub fn parse_id_document_file(data: &[u8], doc: &mut IdDocument) -> Result<()> {
    let f = parse_tlv(data)?;
    assign_field(&f, 1546, &mut doc.doc_reg_no);
    assign_field(&f, 1547, &mut doc.document_type);
    assign_field(&f, 1548, &mut doc.document_serial_number);
    assign_field(&f, 1549, &mut doc.issuing_date);
    assign_field(&f, 1550, &mut doc.expiry_date);
    assign_field(&f, 1551, &mut doc.issuing_authority);
    assign_field(&f, 1681, &mut doc.chip_serial_number);
    assign_field(&f, 1682, &mut doc.document_name);
    format_date(&mut doc.issuing_date);
    format_date(&mut doc.expiry_date);
    Ok(())
}

pub fn parse_id_personal_file(data: &[u8], doc: &mut IdDocument) -> Result<()> {
    let f = parse_tlv(data)?;
    assign_field(&f, 1558, &mut doc.personal_number);
    assign_field(&f, 1559, &mut doc.surname);
    assign_field(&f, 1560, &mut doc.given_name);
    assign_field(&f, 1561, &mut doc.parent_given_name);
    assign_field(&f, 1562, &mut doc.sex);
    assign_field(&f, 1563, &mut doc.place_of_birth);
    assign_field(&f, 1564, &mut doc.community_of_birth);
    assign_field(&f, 1565, &mut doc.state_of_birth);
    assign_field(&f, 1566, &mut doc.date_of_birth);
    assign_field(&f, 1567, &mut doc.state_of_birth_code);
    assign_field(&f, 1583, &mut doc.nationality_full);
    assign_field(&f, 1683, &mut doc.purpose_of_stay);
    if doc.purpose_of_stay.is_empty() {
        assign_field(&f, 1582, &mut doc.purpose_of_stay);
    }
    assign_field(&f, 1684, &mut doc.e_note);
    format_date(&mut doc.date_of_birth);
    Ok(())
}

pub fn parse_id_residence_file(data: &[u8], doc: &mut IdDocument) -> Result<()> {
    let f = parse_tlv(data)?;
    assign_field(&f, 1568, &mut doc.state);
    assign_field(&f, 1569, &mut doc.community);
    assign_field(&f, 1570, &mut doc.place);
    assign_field(&f, 1571, &mut doc.street);
    assign_field(&f, 1572, &mut doc.house_number);
    assign_field(&f, 1573, &mut doc.house_letter);
    assign_field(&f, 1574, &mut doc.entrance);
    assign_field(&f, 1575, &mut doc.floor);
    assign_field(&f, 1578, &mut doc.apartment_number);
    assign_field(&f, 1580, &mut doc.address_date);
    format_date(&mut doc.address_date);
    assign_field(&f, 1581, &mut doc.address_label);
    Ok(())
}

/// Stores the portrait. Bas Celik decodes the image here; we keep the JPEG
/// bytes but still reject data that is not a JPEG image.
pub fn parse_and_assign_id_photo_file(data: &[u8], doc: &mut IdDocument) -> Result<()> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err(CardError::Parse("decoding photo file: not a JPEG image".into()));
    }
    doc.portrait = data.to_vec();
    Ok(())
}

/// Parses the four ID card files (photo file already stripped of its 4 byte header).
pub fn parse_id_document(document_file: &[u8], personal_file: &[u8], residence_file: &[u8], photo_file: &[u8]) -> Result<IdDocument> {
    let mut doc = IdDocument::default();
    parse_id_document_file(document_file, &mut doc).map_err(|e| super::context(e, "parsing document file"))?;
    parse_id_personal_file(personal_file, &mut doc).map_err(|e| super::context(e, "parsing personal file"))?;
    parse_id_residence_file(residence_file, &mut doc).map_err(|e| super::context(e, "parsing residence file"))?;
    parse_and_assign_id_photo_file(photo_file, &mut doc).map_err(|e| super::context(e, "parsing photo file"))?;
    Ok(doc)
}

/// Raw files of an ID card.
#[derive(Debug, Clone, Default)]
pub(crate) struct IdFiles {
    pub document: Vec<u8>,
    pub personal: Vec<u8>,
    pub residence: Vec<u8>,
    pub photo: Vec<u8>,
}

impl IdFiles {
    pub fn read(mut read_file: impl FnMut(&[u8]) -> Result<Vec<u8>>) -> Result<Self> {
        let document = read_file(&ID_DOCUMENT_FILE_LOC).map_err(|e| super::context(e, "reading document file"))?;
        let personal = read_file(&ID_PERSONAL_FILE_LOC).map_err(|e| super::context(e, "reading personal file"))?;
        let residence = read_file(&ID_RESIDENCE_FILE_LOC).map_err(|e| super::context(e, "reading residence file"))?;
        let photo = read_file(&ID_PHOTO_FILE_LOC).map_err(|e| super::context(e, "reading photo file"))?;
        Ok(Self { document, personal, residence, photo: super::trim4b(&photo).to_vec() })
    }

    pub fn parse(&self) -> Result<IdDocument> {
        parse_id_document(&self.document, &self.personal, &self.residence, &self.photo)
    }
}
