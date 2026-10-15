//! Document helper tests (ID and medical documents). PDF tests live in celik-export.

use celik_core::{Document, IdDocument, MedicalDocument, VehicleDocument};

fn id2() -> IdDocument {
    IdDocument {
        given_name: "Петар".into(),
        parent_given_name: "Арсеније".into(),
        surname: "Петровић".into(),
        street: "Његошева".into(),
        house_number: "9".into(),
        house_letter: "Б".into(),
        place: "Подгорица".into(),
        place_of_birth: "Његуши".into(),
        state_of_birth: "Црна Гора".into(),
        ..Default::default()
    }
}

fn id3() -> IdDocument {
    IdDocument {
        given_name: "Pablo Diego".into(),
        surname: "Ruiz Picasso".into(),
        house_number: "7".into(),
        house_letter: "A".into(),
        street: "Rue des Grands-Augustins".into(),
        apartment_number: "21".into(),
        floor: "6".into(),
        community: "Saint-Germain-des-Prés".into(),
        place: "Paris".into(),
        place_of_birth: "Málaga".into(),
        community_of_birth: "Andalucía".into(),
        state_of_birth: "Reino de España".into(),
        ..Default::default()
    }
}

#[test]
fn id_full_name() {
    assert_eq!(IdDocument::default().full_name(), "");
    assert_eq!(id2().full_name(), "Петар, Арсеније, Петровић");
    assert_eq!(id3().full_name(), "Pablo Diego, Ruiz Picasso");
}

#[test]
fn id_full_address() {
    let cases = [
        (IdDocument::default(), "", ""),
        (id2(), "Његошева 9Б, Подгорица", "Подгорица, Његошева 9Б"),
        (
            id3(),
            "Rue des Grands-Augustins 7A/6/21, Saint-Germain-des-Prés, Paris",
            "Paris, Saint-Germain-des-Prés, Rue des Grands-Augustins 7A/6/21",
        ),
    ];
    for (doc, expected, reversed) in cases {
        assert_eq!(doc.full_address(false), expected);
        assert_eq!(doc.full_address(true), reversed);
    }
}

#[test]
fn id_full_place_of_birth() {
    assert_eq!(IdDocument::default().full_place_of_birth(), "");
    assert_eq!(id2().full_place_of_birth(), "Његуши, Црна Гора");
    assert_eq!(
        id3().full_place_of_birth(),
        "Málaga, Andalucía, Reino de España"
    );
}

fn medical2() -> MedicalDocument {
    MedicalDocument {
        given_name_latin: "Петар".into(),
        parent_name_latin: "Арсеније".into(),
        family_name_latin: "Петровић".into(),
        street: "Његошева".into(),
        number: "9".into(),
        apartment: "5".into(),
        place: "Подгорица".into(),
        country: "Црна Гора".into(),
        personal_number: "01019950900200".into(),
        date_of_birth: "01.01.1995".into(),
        carrier_given_name_latin: "Arsenije".into(),
        carrier_family_name_latin: "Petrović".into(),
        carrier_given_name: "Арсеније".into(),
        carrier_family_name: "Петровић".into(),
        carrier_relationship: "Дете".into(),
        carrier_id_number: "19071950123456".into(),
        carrier_insurant_number: "87654321".into(),
        insurant_number: "12345678".into(),
        insurance_start_date: "29.03.2014".into(),
        card_id: "12345678901".into(),
        date_of_issue: "01.02.2024".into(),
        date_of_expiry: "01.02.2029".into(),
        valid_until: "01.02.2039".into(),
        permanently_valid: false,
        carrier_family_member: true,
        gender: "Мушки".into(),
        insurance_description: "Самостална занатске делатност".into(),
        insurance_basis_rzzo: "123".into(),
        taxpayer_name: "Arsenije Petrović".into(),
        taxpayer_residence: "Beograd".into(),
        taxpayer_id_number: "19071950123456".into(),
        ..Default::default()
    }
}

fn medical3() -> MedicalDocument {
    MedicalDocument {
        given_name_latin: "Pablo Diego".into(),
        family_name_latin: "Ruiz Picasso".into(),
        street: "Rue des Grands-Augustins".into(),
        place: "Paris".into(),
        ..Default::default()
    }
}

#[test]
fn medical_full_name() {
    assert_eq!(MedicalDocument::default().full_name(), "");
    assert_eq!(medical2().full_name(), "Петар, Арсеније, Петровић");
    assert_eq!(medical3().full_name(), "Pablo Diego, Ruiz Picasso");
}

#[test]
fn medical_full_street_address() {
    assert_eq!(MedicalDocument::default().full_street_address(), "");
    assert_eq!(
        medical2().full_street_address(),
        "Његошева, Број: 9 Стан: 5"
    );
    assert_eq!(medical3().full_street_address(), "Rue des Grands-Augustins");
}

#[test]
fn medical_full_place_address() {
    assert_eq!(MedicalDocument::default().full_place_address(), "");
    assert_eq!(medical2().full_place_address(), "Подгорица, Црна Гора");
    assert_eq!(medical3().full_place_address(), "Paris");
}

#[test]
fn file_stems() {
    assert_eq!(id3().file_stem(), "pablo diego_ruiz picasso");
    assert_eq!(medical3().file_stem(), "pablo diego_ruiz picasso");
    let v = VehicleDocument {
        registration_number_of_vehicle: "BG123AB".into(),
        owners_surname_or_business_name: "Petrović".into(),
        owner_name: "Petar".into(),
        ..Default::default()
    };
    assert_eq!(
        Document::Vehicle(Box::new(v)).file_stem(),
        "bg123ab_petrović_petar"
    );
}

#[test]
fn json_round_trip() {
    let doc = Document::Id(Box::new(IdDocument {
        portrait: vec![0xFF, 0xD8, 1],
        e_note: "x".into(),
        ..id2()
    }));
    let json = serde_json::to_value(&doc).unwrap();
    assert_eq!(json["kind"], "id");
    assert_eq!(json["data"]["Portrait"], "/9gB");
    assert_eq!(json["data"]["ENote"], "x");
    assert_eq!(json["data"]["GivenName"], "Петар");
    let back: Document = serde_json::from_value(json).unwrap();
    assert_eq!(back, doc);

    let m = serde_json::to_value(medical2()).unwrap();
    assert_eq!(m["InsurerID"], "");
    assert_eq!(m["InsuranceBasisRZZO"], "123");
    assert_eq!(m["CarrierFamilyMember"], true);
}
