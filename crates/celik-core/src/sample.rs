//! Sample (fake) documents for mock mode, UI development and tests.
//!
//! All values are invented; they only mimic the shape of real documents.

use crate::document::{IdDocument, MedicalDocument, VehicleDocument, ID_TYPE_ID};

/// Grey silhouette portrait (300x400 JPEG) used by the sample ID document.
pub const SAMPLE_PORTRAIT_JPEG: &[u8] = include_bytes!("sample_portrait.jpg");

pub fn id() -> IdDocument {
    IdDocument {
        portrait: SAMPLE_PORTRAIT_JPEG.to_vec(),
        doc_reg_no: "012345678".into(),
        document_type: ID_TYPE_ID.into(),
        issuing_date: "15.03.2021.".into(),
        expiry_date: "15.03.2031.".into(),
        issuing_authority: "PU za grad Beograd".into(),
        document_serial_number: "".into(),
        chip_serial_number: "".into(),
        document_name: "".into(),
        personal_number: "0101990710020".into(),
        surname: "Petrović".into(),
        given_name: "Petar".into(),
        parent_given_name: "Marko".into(),
        sex: "M".into(),
        place_of_birth: "Beograd".into(),
        community_of_birth: "Vračar".into(),
        state_of_birth: "Republika Srbija".into(),
        state_of_birth_code: "SRB".into(),
        date_of_birth: "01.01.1990.".into(),
        status_of_foreigner: "".into(),
        nationality_full: "".into(),
        purpose_of_stay: "".into(),
        e_note: "".into(),
        state: "SRB".into(),
        community: "Vračar".into(),
        place: "Beograd".into(),
        street: "Kneza Miloša".into(),
        house_number: "12".into(),
        house_letter: "".into(),
        entrance: "".into(),
        floor: "3".into(),
        apartment_number: "5".into(),
        address_date: "15.06.2021.".into(),
        address_label: "".into(),
    }
}

pub fn medical() -> MedicalDocument {
    MedicalDocument {
        insurer_name: "Републички фонд за здравствено осигурање".into(),
        insurer_id: "06110101000000".into(),
        card_id: "12345678901".into(),
        date_of_issue: "01.01.2020.".into(),
        date_of_expiry: "01.01.2030.".into(),
        chip_serial_number: "1234567890123456".into(),
        print_language: "SR".into(),
        personal_number: "0101990710020".into(),
        family_name_latin: "Petrović".into(),
        given_name_latin: "Petar".into(),
        parent_name_latin: "Marko".into(),
        family_name: "Петровић".into(),
        given_name: "Петар".into(),
        parent_name: "Марко".into(),
        gender: "Мушко".into(),
        insurant_number: "12345678901".into(),
        date_of_birth: "01.01.1990.".into(),
        apartment: "5".into(),
        number: "12".into(),
        entrance: "".into(),
        street: "Кнеза Милоша".into(),
        street_code: "00123".into(),
        place: "Београд".into(),
        post_number: "11000".into(),
        municipality: "Врачар".into(),
        country: "Србија".into(),
        valid_until: "31.12.2026.".into(),
        permanently_valid: false,
        carrier_given_name_latin: "".into(),
        carrier_family_name_latin: "".into(),
        carrier_given_name: "".into(),
        carrier_family_name: "".into(),
        carrier_id_number: "".into(),
        carrier_insurant_number: "".into(),
        carrier_family_member: false,
        carrier_relationship: "".into(),
        insurance_basis_rzzo: "123".into(),
        insurance_start_date: "15.06.2021.".into(),
        insurance_description: "Запослени у привредном друштву".into(),
        taxpayer_name: "Пример д.о.о. Београд".into(),
        taxpayer_residence: "Београд".into(),
        taxpayer_number: "".into(),
        taxpayer_id_number: "100000001".into(),
        taxpayer_activity_code: "6201".into(),
    }
}

pub fn vehicle() -> VehicleDocument {
    VehicleDocument {
        authority_issuing: "PU Beograd".into(),
        colour_of_vehicle: "PLAVA".into(),
        commercial_description: "101 SKALA".into(),
        competent_authority: "MUP Republike Srbije".into(),
        date_of_first_registration: "15.01.2020".into(),
        engine_capacity: "1100".into(),
        engine_id_number: "ENG123456".into(),
        expiry_date: "31.12.2026".into(),
        homologation_mark: "".into(),
        issuing_date: "01.01.2024".into(),
        maximum_net_power: "40".into(),
        maximum_permissible_laden_mass: "1500".into(),
        number_of_axles: "2".into(),
        number_of_seats: "5".into(),
        number_of_standing_places: "0".into(),
        owner_address: "Kneza Miloša 12, Beograd".into(),
        owner_name: "Petar".into(),
        owners_personal_no: "0101990710020".into(),
        owners_surname_or_business_name: "Petrović".into(),
        power_weight_ratio: "36".into(),
        registration_number_of_vehicle: "BG123AB".into(),
        serial_number: "SRB1234567890X".into(),
        state_issuing: "SRB".into(),
        type_approval_number: "e1".into(),
        type_of_fuel: "BENZIN".into(),
        unambiguous_number: "SRB1234".into(),
        users_address: "Kneza Miloša 12, Beograd".into(),
        users_name: "Petar".into(),
        users_personal_no: "0101990710020".into(),
        users_surname_or_business_name: "Petrović".into(),
        vehicle_category: "M1".into(),
        vehicle_id_number: "VX1234567890ABCDE".into(),
        vehicle_load: "0".into(),
        vehicle_make: "ZASTAVA".into(),
        vehicle_mass: "1100".into(),
        vehicle_type: "H".into(),
        year_of_production: "2020".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples() {
        assert_eq!(&SAMPLE_PORTRAIT_JPEG[..2], &[0xFF, 0xD8]);
        assert_eq!(id().file_stem(), "petar_petrović");
        assert_eq!(medical().file_stem(), "petar_petrović");
        assert_eq!(vehicle().file_stem(), "bg123ab_petrović_petar");
        let json = serde_json::to_string(&crate::Document::Id(Box::new(id()))).unwrap();
        assert!(json.contains("\"Portrait\":\"/9j/"));
    }
}
