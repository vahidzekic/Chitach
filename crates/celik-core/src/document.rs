//! Document data model. Field names serialize in PascalCase so that JSON
//! export uses the established field names and order.

use serde::{Deserialize, Serialize};

pub const ID_TYPE_APOLLO: &str = "";
pub const ID_TYPE_ID: &str = "ID";
pub const ID_TYPE_IDENTITY_FOREIGNER: &str = "IF";
pub const ID_TYPE_RESIDENCE_PERMIT: &str = "RP";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IdDocument {
    /// JPEG bytes of the portrait. Serialized as base64 string.
    #[serde(with = "crate::document::b64")]
    pub portrait: Vec<u8>,
    pub doc_reg_no: String,
    pub document_type: String,
    pub issuing_date: String,
    pub expiry_date: String,
    pub issuing_authority: String,
    pub document_serial_number: String,
    pub chip_serial_number: String,
    pub document_name: String,
    pub personal_number: String,
    pub surname: String,
    pub given_name: String,
    pub parent_given_name: String,
    pub sex: String,
    pub place_of_birth: String,
    pub community_of_birth: String,
    pub state_of_birth: String,
    pub state_of_birth_code: String,
    pub date_of_birth: String,
    pub status_of_foreigner: String,
    pub nationality_full: String,
    pub purpose_of_stay: String,
    #[serde(rename = "ENote")]
    pub e_note: String,
    pub state: String,
    pub community: String,
    pub place: String,
    pub street: String,
    pub house_number: String,
    pub house_letter: String,
    pub entrance: String,
    pub floor: String,
    pub apartment_number: String,
    pub address_date: String,
    pub address_label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MedicalDocument {
    pub insurer_name: String,
    #[serde(rename = "InsurerID")]
    pub insurer_id: String,
    pub card_id: String,
    pub date_of_issue: String,
    pub date_of_expiry: String,
    pub chip_serial_number: String,
    pub print_language: String,
    pub personal_number: String,
    pub family_name_latin: String,
    pub given_name_latin: String,
    pub parent_name_latin: String,
    pub family_name: String,
    pub given_name: String,
    pub parent_name: String,
    pub gender: String,
    pub insurant_number: String,
    pub date_of_birth: String,
    pub apartment: String,
    pub number: String,
    pub entrance: String,
    pub street: String,
    pub street_code: String,
    pub place: String,
    pub post_number: String,
    pub municipality: String,
    pub country: String,
    pub valid_until: String,
    pub permanently_valid: bool,
    pub carrier_given_name_latin: String,
    pub carrier_family_name_latin: String,
    pub carrier_given_name: String,
    pub carrier_family_name: String,
    pub carrier_id_number: String,
    pub carrier_insurant_number: String,
    pub carrier_family_member: bool,
    pub carrier_relationship: String,
    #[serde(rename = "InsuranceBasisRZZO")]
    pub insurance_basis_rzzo: String,
    pub insurance_start_date: String,
    pub insurance_description: String,
    pub taxpayer_name: String,
    pub taxpayer_residence: String,
    pub taxpayer_number: String,
    pub taxpayer_id_number: String,
    pub taxpayer_activity_code: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct VehicleDocument {
    pub authority_issuing: String,
    pub colour_of_vehicle: String,
    pub commercial_description: String,
    pub competent_authority: String,
    pub date_of_first_registration: String,
    pub engine_capacity: String,
    pub engine_id_number: String,
    pub expiry_date: String,
    pub homologation_mark: String,
    pub issuing_date: String,
    pub maximum_net_power: String,
    pub maximum_permissible_laden_mass: String,
    pub number_of_axles: String,
    pub number_of_seats: String,
    pub number_of_standing_places: String,
    pub owner_address: String,
    pub owner_name: String,
    pub owners_personal_no: String,
    pub owners_surname_or_business_name: String,
    pub power_weight_ratio: String,
    pub registration_number_of_vehicle: String,
    pub serial_number: String,
    pub state_issuing: String,
    pub type_approval_number: String,
    pub type_of_fuel: String,
    pub unambiguous_number: String,
    pub users_address: String,
    pub users_name: String,
    pub users_personal_no: String,
    pub users_surname_or_business_name: String,
    pub vehicle_category: String,
    pub vehicle_id_number: String,
    pub vehicle_load: String,
    pub vehicle_make: String,
    pub vehicle_mass: String,
    pub vehicle_type: String,
    pub year_of_production: String,
}

/// Any document Chitach can read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "lowercase")]
pub enum Document {
    Id(Box<IdDocument>),
    Medical(Box<MedicalDocument>),
    Vehicle(Box<VehicleDocument>),
}

use crate::localization::join_with_comma;

impl IdDocument {
    pub fn full_name(&self) -> String {
        join_with_comma(&[&self.given_name, &self.parent_given_name, &self.surname])
    }

    pub fn full_address(&self, reverse: bool) -> String {
        let mut street = self.street.clone();
        if !self.house_number.is_empty()
            || !self.house_letter.is_empty()
            || !self.entrance.is_empty()
        {
            street.push(' ');
            street.push_str(&self.house_number);
            street.push_str(&self.house_letter);
            if !self.floor.is_empty() {
                street.push('/');
                street.push_str(&self.floor);
            }
            if !self.apartment_number.is_empty() {
                street.push('/');
                street.push_str(&self.apartment_number);
            }
        }
        if reverse {
            join_with_comma(&[&self.place, &self.community, &street])
        } else {
            join_with_comma(&[&street, &self.community, &self.place])
        }
    }

    pub fn full_place_of_birth(&self) -> String {
        join_with_comma(&[
            &self.place_of_birth,
            &self.community_of_birth,
            &self.state_of_birth,
        ])
    }

    /// Base file name without extension (e.g. `petar_petrovic`).
    pub fn file_stem(&self) -> String {
        format!("{}_{}", self.given_name, self.surname).to_lowercase()
    }
}

impl MedicalDocument {
    pub fn full_name(&self) -> String {
        join_with_comma(&[
            &self.given_name_latin,
            &self.parent_name_latin,
            &self.family_name_latin,
        ])
    }

    pub fn full_street_address(&self) -> String {
        let mut a = self.street.clone();
        if !self.number.is_empty() {
            a.push_str(", Број: ");
            a.push_str(&self.number);
        }
        if !self.apartment.is_empty() {
            a.push_str(" Стан: ");
            a.push_str(&self.apartment);
        }
        a
    }

    pub fn full_place_address(&self) -> String {
        join_with_comma(&[&self.place, &self.municipality, &self.country])
    }

    pub fn file_stem(&self) -> String {
        format!("{}_{}", self.given_name_latin, self.family_name_latin).to_lowercase()
    }
}

impl VehicleDocument {
    pub fn file_stem(&self) -> String {
        format!(
            "{}_{}_{}",
            self.registration_number_of_vehicle,
            self.owners_surname_or_business_name,
            self.owner_name
        )
        .to_lowercase()
    }
}

impl Document {
    pub fn file_stem(&self) -> String {
        match self {
            Document::Id(d) => d.file_stem(),
            Document::Medical(d) => d.file_stem(),
            Document::Vehicle(d) => d.file_stem(),
        }
    }
}

pub(crate) mod b64 {
    use base64::Engine;
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(v))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(serde::de::Error::custom)
    }
}
