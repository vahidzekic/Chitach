//! Serbian vehicle registration cards.
//!
//! Port of bas-celik `card/vehicle.go` (MIT, (c) Nikola Ubavić).

use super::{read, read_chunks, select, CardDocument, CardKind, CardTransport};
use crate::apdu::build_apdu;
use crate::ber::{parse_ber, parse_length, parse_tag, Ber};
use crate::document::{Document, VehicleDocument};
use crate::error::FormatError;
use crate::localization::format_date_ymd;
use crate::{CardError, Result};

/// The three sets of SELECT commands tried when initialising a vehicle card
/// (reverse-engineered from the official binary).
pub const INIT_COMMANDS: [[&[u8]; 3]; 3] = [
    [
        &[0xA0, 0x00, 0x00, 0x01, 0x51, 0x00, 0x00],
        &[0xA0, 0x00, 0x00, 0x00, 0x77, 0x01, 0x08, 0x00, 0x07, 0x00, 0x00, 0xFE, 0x00, 0x00, 0x01, 0x00],
        &[0xA0, 0x00, 0x00, 0x00, 0x77, 0x01, 0x08, 0x00, 0x07, 0x00, 0x00, 0xFE, 0x00, 0x00, 0xAD, 0xF2],
    ],
    [
        &[0xA0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00],
        &[0xF3, 0x81, 0x00, 0x00, 0x02, 0x53, 0x45, 0x52, 0x56, 0x4C, 0x04, 0x02, 0x01],
        &[0xA0, 0x00, 0x00, 0x00, 0x77, 0x01, 0x08, 0x00, 0x07, 0x00, 0x00, 0xFE, 0x00, 0x00, 0xAD, 0xF2],
    ],
    [
        &[0xA0, 0x00, 0x00, 0x00, 0x18, 0x43, 0x4D, 0x00],
        &[0xA0, 0x00, 0x00, 0x00, 0x18, 0x34, 0x14, 0x01, 0x00, 0x65, 0x56, 0x4C, 0x2D, 0x30, 0x30, 0x31],
        &[0xA0, 0x00, 0x00, 0x00, 0x18, 0x65, 0x56, 0x4C, 0x2D, 0x30, 0x30, 0x31],
    ],
];

/// Locations of the four document files (`D0 01`, `D0 11`, `D0 21`, `D0 31`).
pub const VEHICLE_FILES: [[u8; 2]; 4] = [[0xD0, 0x01], [0xD0, 0x11], [0xD0, 0x21], [0xD0, 0x31]];

/// A smart card holding a Serbian vehicle registration document.
pub struct VehicleCard<'a> {
    atr: Vec<u8>,
    card: &'a dyn CardTransport,
    files: [Vec<u8>; 4],
}

impl<'a> VehicleCard<'a> {
    pub fn new(card: &'a dyn CardTransport, atr: Vec<u8>) -> Self {
        Self { atr, card, files: Default::default() }
    }

    /// Reads a file whose size is encoded in a BER header.
    pub fn read_file(&self, name: &[u8]) -> Result<Vec<u8>> {
        select(self.card, 0x02, 0x04, name, 0)?;

        const HEADER_SIZE: usize = 0x20;
        let header = read(self.card, 0, HEADER_SIZE)?;
        let (length, offset) = parse_vehicle_card_file_size(&header)
            .map_err(|e| CardError::Parse(format!("parsing file header: {e}")))?;

        read_chunks(self.card, offset, length, 0x64)
    }

    fn try_to_select(&self, cmds: &[&[u8]; 3]) -> Result<()> {
        let rsp = self.card.transmit(&build_apdu(0x00, 0xA4, 0x04, 0x00, cmds[0], 0))?;
        super::check_ok(&rsp, "first command")?;
        self.card.transmit(&build_apdu(0x00, 0xA4, 0x04, 0x00, cmds[1], 0))?;
        self.card.transmit(&build_apdu(0x00, 0xA4, 0x04, 0x0C, cmds[2], 0))?;
        Ok(())
    }
}

impl CardDocument for VehicleCard<'_> {
    fn kind(&self) -> CardKind {
        CardKind::Vehicle
    }
    fn atr(&self) -> &[u8] {
        &self.atr
    }
    /// Initializes the card by trying three different sets of commands.
    fn init_card(&mut self) -> Result<()> {
        let mut last = None;
        for cmds in &INIT_COMMANDS {
            match self.try_to_select(cmds) {
                Ok(()) => return Ok(()),
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap_or_else(|| CardError::Other("card not responsive".into())))
    }
    fn read_card(&mut self) -> Result<()> {
        let mut files: [Vec<u8>; 4] = Default::default();
        for (i, name) in VEHICLE_FILES.iter().enumerate() {
            files[i] = self.read_file(name).map_err(|e| super::context(e, &format!("reading document {i} file")))?;
        }
        self.files = files;
        Ok(())
    }
    fn get_document(&self) -> Result<Document> {
        Ok(Document::Vehicle(Box::new(parse_vehicle_document(&self.files)?)))
    }
    fn test(&mut self) -> bool {
        self.init_card().is_ok()
    }
}

/// Parses the vehicle file header. Returns `(length, offset)` of the data.
pub fn parse_vehicle_card_file_size(data: &[u8]) -> std::result::Result<(usize, usize), FormatError> {
    if data.len() < 2 {
        return Err(FormatError::InvalidLength);
    }

    let offset = data[1] as usize + 2;
    if offset >= data.len() {
        return Err(FormatError::InvalidLength);
    }

    let (_, _, delta1) = parse_tag(&data[offset..])?;
    if offset + delta1 >= data.len() {
        return Err(FormatError::InvalidLength);
    }

    let (data_length, delta2) = parse_length(&data[offset + delta1..])?;
    Ok((data_length as usize + delta1 + delta2, offset))
}

/// Parses and merges the four BER encoded vehicle files.
pub fn parse_vehicle_document<F: AsRef<[u8]>>(files: &[F]) -> Result<VehicleDocument> {
    let mut data = Ber::default();
    for (i, file) in files.iter().enumerate() {
        let parsed = parse_ber(file.as_ref()).map_err(|e| CardError::Parse(format!("parsing {i} file: {e}")))?;
        data.merge(parsed).map_err(|e| CardError::Parse(format!("merging {i} data: {e}")))?;
    }

    let mut doc = VehicleDocument::default();
    let d = &data;
    d.assign_from(&mut doc.registration_number_of_vehicle, &[0x71, 0x81]);
    d.assign_from(&mut doc.date_of_first_registration, &[0x71, 0x82]);
    format_date_ymd(&mut doc.date_of_first_registration);
    d.assign_from(&mut doc.vehicle_id_number, &[0x71, 0x8A]);
    d.assign_from(&mut doc.vehicle_mass, &[0x71, 0x8C]);
    d.assign_from(&mut doc.expiry_date, &[0x71, 0x8D]);
    format_date_ymd(&mut doc.expiry_date);
    d.assign_from(&mut doc.issuing_date, &[0x71, 0x8E]);
    format_date_ymd(&mut doc.issuing_date);
    d.assign_from(&mut doc.type_approval_number, &[0x71, 0x8F]);
    d.assign_from(&mut doc.power_weight_ratio, &[0x71, 0x93]);
    d.assign_from(&mut doc.vehicle_make, &[0x71, 0xA3, 0x87]);
    d.assign_from(&mut doc.vehicle_type, &[0x71, 0xA3, 0x88]);
    d.assign_from(&mut doc.commercial_description, &[0x71, 0xA3, 0x89]);
    d.assign_from(&mut doc.maximum_permissible_laden_mass, &[0x71, 0xA4, 0x8B]);
    d.assign_from(&mut doc.engine_capacity, &[0x71, 0xA5, 0x90]);
    d.assign_from(&mut doc.maximum_net_power, &[0x71, 0xA5, 0x91]);
    d.assign_from(&mut doc.type_of_fuel, &[0x71, 0xA5, 0x92]);
    d.assign_from(&mut doc.number_of_seats, &[0x71, 0xA6, 0x94]);
    d.assign_from(&mut doc.number_of_standing_places, &[0x71, 0xA6, 0x95]);
    d.assign_from(&mut doc.state_issuing, &[0x71, 0x9F33]);
    d.assign_from(&mut doc.competent_authority, &[0x71, 0x9F35]);
    d.assign_from(&mut doc.authority_issuing, &[0x71, 0x9F36]);
    d.assign_from(&mut doc.unambiguous_number, &[0x71, 0x9F38]);
    d.assign_from(&mut doc.vehicle_category, &[0x72, 0x98]);
    d.assign_from(&mut doc.number_of_axles, &[0x72, 0x99]);
    d.assign_from(&mut doc.vehicle_load, &[0x72, 0xC4]);
    d.assign_from(&mut doc.year_of_production, &[0x72, 0xC5]);
    d.assign_from(&mut doc.engine_id_number, &[0x72, 0xA5, 0x9E]);
    d.assign_from(&mut doc.serial_number, &[0x72, 0xC9]);
    d.assign_from(&mut doc.colour_of_vehicle, &[0x72, 0x9F24]);
    d.assign_from(&mut doc.users_personal_no, &[0x72, 0xC3]);
    d.assign_from(&mut doc.owners_personal_no, &[0x72, 0xC2]);

    d.assign_from(&mut doc.owners_surname_or_business_name, &[0x71, 0xA1, 0xA2, 0x83]);
    d.assign_from(&mut doc.owner_name, &[0x71, 0xA1, 0xA2, 0x84]);
    d.assign_from(&mut doc.owner_address, &[0x71, 0xA1, 0xA2, 0x85]);

    d.assign_from(&mut doc.users_surname_or_business_name, &[0x71, 0xA1, 0xA9, 0x83]);
    d.assign_from(&mut doc.users_name, &[0x71, 0xA1, 0xA9, 0x84]);
    d.assign_from(&mut doc.users_address, &[0x71, 0xA1, 0xA9, 0x85]);
    if doc.users_name.is_empty() && doc.users_surname_or_business_name.is_empty() && doc.users_address.is_empty() {
        d.assign_from(&mut doc.users_surname_or_business_name, &[0x72, 0xA1, 0xA9, 0x83]);
        d.assign_from(&mut doc.users_name, &[0x72, 0xA1, 0xA9, 0x84]);
        d.assign_from(&mut doc.users_address, &[0x72, 0xA1, 0xA9, 0x85]);
    }

    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use FormatError::*;

    #[test]
    fn file_size() {
        let cases: Vec<(Vec<u8>, std::result::Result<(usize, usize), FormatError>)> = vec![
            (vec![], Err(InvalidLength)),
            (vec![0x01], Err(InvalidLength)),
            (vec![0x01, 0x02, 0x03, 0x04], Err(InvalidLength)),
            (
                vec![
                    0x78, 0x0E, 0x4F, 0x0C, 0xA0, 0x00, 0x00, 0x00, 0x18, 0x65, 0x56, 0x4C, 0x2D, 0x30, 0x30, 0x31, 0x72,
                ],
                Err(InvalidLength),
            ),
            (
                vec![
                    0x78, 0x0E, 0x4F, 0x0C, 0xA0, 0x00, 0x00, 0x00, 0x18, 0x65, 0x56, 0x4C, 0x2D, 0x30, 0x30, 0x31, 0x72,
                    0x27,
                ],
                Ok((41, 16)),
            ),
            (vec![0x01, 0x01, 0x01, 0x00, 0x80], Err(InvalidFormat)),
        ];
        for (data, expected) in cases {
            assert_eq!(parse_vehicle_card_file_size(&data), expected, "{data:02X?}");
        }
    }
}
