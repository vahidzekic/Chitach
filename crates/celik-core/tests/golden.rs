//! Golden tests ported from bas-celik `card/*_golden_test.go` (MIT, (c) Nikola Ubavić),
//! plus end-to-end reads through the virtual card.
//!
//! All values are fabricated; only the tag layouts mirror real cards.

use celik_core::atr::*;
use celik_core::ber::encode_record as ber;
use celik_core::card::gemalto::{AID_IF, AID_RP};
use celik_core::card::id::parse_id_document;
use celik_core::card::medical::parse_medical_document;
use celik_core::card::vehicle::parse_vehicle_document;
use celik_core::tlv::encode_record as tlv;
use celik_core::virtual_card::VirtualCard;
use celik_core::{detect_card, read_document, CardError, CardKind, Document, IdCardCrypto};

const PORTRAIT: &[u8] = celik_core::sample::SAMPLE_PORTRAIT_JPEG;

fn concat(records: &[Vec<u8>]) -> Vec<u8> {
    records.concat()
}

// ---------------------------------------------------------------- ID ----

fn id_files() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let document = concat(&[
        tlv(1546, b"123456789"),
        tlv(1547, b"ID"),
        tlv(1548, b"RS12345678"),
        tlv(1549, b"01012020"),
        tlv(1550, b"01012030"),
        tlv(1551, b"MUP Republike Srbije"),
    ]);
    let personal = concat(&[
        tlv(1558, b"0101990710020"),
        tlv(1559, "Petrović".as_bytes()),
        tlv(1560, b"Petar"),
        tlv(1561, b"Marko"),
        tlv(1562, b"M"),
        tlv(1563, b"Beograd"),
        tlv(1565, b"SRB"),
        tlv(1566, b"01011990"),
        tlv(1567, b"SRB"),
    ]);
    let residence = concat(&[
        tlv(1568, b"SRB"),
        tlv(1569, "Vračar".as_bytes()),
        tlv(1570, b"Beograd"),
        tlv(1571, "Kneza Miloša".as_bytes()),
        tlv(1572, b"12"),
        tlv(1575, b"3"),
        tlv(1578, b"5"),
        tlv(1580, b"15062021"),
        tlv(1581, b"stan"),
    ]);
    (document, personal, residence)
}

fn check_id(id: &celik_core::IdDocument) {
    let cases = [
        ("DocRegNo", &id.doc_reg_no, "123456789"),
        ("DocumentType", &id.document_type, "ID"),
        (
            "DocumentSerialNumber",
            &id.document_serial_number,
            "RS12345678",
        ),
        ("IssuingDate", &id.issuing_date, "01.01.2020."),
        ("ExpiryDate", &id.expiry_date, "01.01.2030."),
        (
            "IssuingAuthority",
            &id.issuing_authority,
            "MUP Republike Srbije",
        ),
        ("PersonalNumber", &id.personal_number, "0101990710020"),
        ("Surname", &id.surname, "Petrović"),
        ("GivenName", &id.given_name, "Petar"),
        ("ParentGivenName", &id.parent_given_name, "Marko"),
        ("Sex", &id.sex, "M"),
        ("PlaceOfBirth", &id.place_of_birth, "Beograd"),
        ("StateOfBirth", &id.state_of_birth, "SRB"),
        ("DateOfBirth", &id.date_of_birth, "01.01.1990."),
        ("StateOfBirthCode", &id.state_of_birth_code, "SRB"),
        ("State", &id.state, "SRB"),
        ("Community", &id.community, "Vračar"),
        ("Place", &id.place, "Beograd"),
        ("Street", &id.street, "Kneza Miloša"),
        ("HouseNumber", &id.house_number, "12"),
        ("Floor", &id.floor, "3"),
        ("ApartmentNumber", &id.apartment_number, "5"),
        ("AddressDate", &id.address_date, "15.06.2021."),
        ("AddressLabel", &id.address_label, "stan"),
    ];
    for (field, got, expected) in cases {
        assert_eq!(got, expected, "{field}");
    }
    assert_eq!(id.portrait, PORTRAIT, "Portrait");
}

#[test]
fn gemalto_get_document() {
    let (d, p, r) = id_files();
    check_id(&parse_id_document(&d, &p, &r, PORTRAIT).unwrap());
}

#[test]
fn id_photo_must_be_jpeg() {
    let (d, p, r) = id_files();
    assert!(matches!(
        parse_id_document(&d, &p, &r, b"not a jpeg"),
        Err(CardError::Parse(_))
    ));
}

fn expect_id(doc: Document) -> celik_core::IdDocument {
    match doc {
        Document::Id(id) => *id,
        other => panic!("expected ID document, got {other:?}"),
    }
}

#[test]
fn gemalto_end_to_end_all_atrs() {
    let (d, p, r) = id_files();
    for atr in [GEMALTO_ATR_1, GEMALTO_ATR_2, GEMALTO_ATR_3, GEMALTO_ATR_4] {
        let card = VirtualCard::gemalto_id(atr, &d, &p, &r, PORTRAIT);
        assert_eq!(detect_card(&card).unwrap(), CardKind::GemaltoId);
        check_id(&expect_id(read_document(&card).unwrap()));
        assert_eq!(card.transaction_depth(), 0);
    }
}

#[test]
fn gemalto_foreigner_and_residence_permit() {
    for (aid, kind) in [(AID_IF, "IF"), (AID_RP, "RP")] {
        let document = concat(&[
            tlv(1546, b"987654321"),
            tlv(1547, kind.as_bytes()),
            tlv(1550, b"01012030"),
        ]);
        let personal = concat(&[
            tlv(1558, b"1234567890123"),
            tlv(1559, b"Smith"),
            tlv(1560, b"John"),
            tlv(1583, b"United Kingdom"),
            tlv(1582, b"Rad"),
            tlv(1684, b"Napomena"),
        ]);
        let residence = concat(&[tlv(1570, b"Novi Sad")]);
        let card = VirtualCard::gemalto_with_aid(
            GEMALTO_ATR_3,
            aid,
            &document,
            &personal,
            &residence,
            PORTRAIT,
        );
        let id = expect_id(read_document(&card).unwrap());
        assert_eq!(id.document_type, kind);
        assert_eq!(id.nationality_full, "United Kingdom");
        assert_eq!(id.purpose_of_stay, "Rad");
        assert_eq!(id.e_note, "Napomena");
        assert_eq!(id.place, "Novi Sad");
        assert_eq!(id.expiry_date, "01.01.2030.");
    }
}

#[test]
fn purpose_of_stay_prefers_tag_1683() {
    let personal = concat(&[tlv(1683, b"Studije"), tlv(1582, b"Rad")]);
    let (d, _, r) = id_files();
    let id = parse_id_document(&d, &personal, &r, PORTRAIT).unwrap();
    assert_eq!(id.purpose_of_stay, "Studije");
}

#[test]
fn apollo_end_to_end() {
    let (d, p, r) = id_files();
    let card = VirtualCard::apollo_id(APOLLO_ATR, &d, &p, &r, PORTRAIT);
    assert_eq!(detect_card(&card).unwrap(), CardKind::ApolloId);
    check_id(&expect_id(read_document(&card).unwrap()));
}

#[test]
fn large_portrait_is_read_in_chunks() {
    let (d, p, r) = id_files();
    let mut big = PORTRAIT.to_vec();
    big.extend(std::iter::repeat_n(0xAB, 5000));
    let card = VirtualCard::gemalto_id(GEMALTO_ATR_2, &d, &p, &r, &big);
    assert_eq!(expect_id(read_document(&card).unwrap()).portrait, big);
}

// ----------------------------------------------------------- Medical ----

/// UTF-16 LE with BOM, like Go's `unicode.UTF16(LittleEndian, UseBOM)` encoder.
fn u16(s: &str) -> Vec<u8> {
    let mut out = vec![0xFF, 0xFE];
    out.extend(s.encode_utf16().flat_map(u16::to_le_bytes));
    out
}

fn medical_files() -> [Vec<u8>; 4] {
    let document = concat(&[
        tlv(1553, &u16("Републички фонд за здравствено осигурање")),
        tlv(1554, b"06110101000000"),
        tlv(1555, b"12345678901"),
        tlv(1557, b"01012020"),
        tlv(1558, b"01012030"),
        tlv(1559, b"1234567890123456"),
        tlv(1560, b"SR"),
    ]);
    let fixed_personal = concat(&[
        tlv(1569, b"12345678901"),
        tlv(1570, &u16("Перић")),
        tlv(1571, &u16("Perić")),
        tlv(1572, &u16("Петар")),
        tlv(1573, &u16("Petar")),
        tlv(1574, b"01011990"),
    ]);
    let variable_personal = concat(&[tlv(1586, b"31122025"), tlv(1587, b"1")]);
    let variable_admin = concat(&[
        tlv(1601, &u16("Марко")),
        tlv(1602, &u16("Marko")),
        tlv(1603, b"02"),
        tlv(1604, b"0101990710020"),
        tlv(1605, &u16("Кнеза Милоша")),
        tlv(1606, b"11000"),
        tlv(1607, &u16("Врачар")),
        tlv(1608, &u16("Београд")),
        tlv(1609, b"00123"),
        tlv(1610, &u16("5")),
        tlv(1614, b"123"),
        tlv(1615, &u16("Самостална делатност")),
        tlv(1624, b"15062021"),
        tlv(1626, &u16("Србија")),
        tlv(1630, &u16("Петар Перић")),
        tlv(1631, &u16("Б")),
        tlv(1633, b"0101990710020"),
    ]);
    [document, fixed_personal, variable_personal, variable_admin]
}

fn check_medical(m: &celik_core::MedicalDocument) {
    let cases = [
        (
            "InsurerName",
            &m.insurer_name,
            "Републички фонд за здравствено осигурање",
        ),
        ("InsurerID", &m.insurer_id, "06110101000000"),
        ("CardId", &m.card_id, "12345678901"),
        ("DateOfIssue", &m.date_of_issue, "01.01.2020."),
        ("DateOfExpiry", &m.date_of_expiry, "01.01.2030."),
        (
            "ChipSerialNumber",
            &m.chip_serial_number,
            "1234567890123456",
        ),
        ("PrintLanguage", &m.print_language, "SR"),
        ("InsurantNumber", &m.insurant_number, "12345678901"),
        ("FamilyName", &m.family_name, "Перић"),
        ("FamilyNameLatin", &m.family_name_latin, "Perić"),
        ("GivenName", &m.given_name, "Петар"),
        ("GivenNameLatin", &m.given_name_latin, "Petar"),
        ("DateOfBirth", &m.date_of_birth, "01.01.1990."),
        ("ValidUntil", &m.valid_until, "31.12.2025."),
        ("ParentName", &m.parent_name, "Марко"),
        ("ParentNameLatin", &m.parent_name_latin, "Marko"),
        ("Gender", &m.gender, "Женско"),
        ("PersonalNumber", &m.personal_number, "0101990710020"),
        ("Street", &m.street, "Кнеза Милоша"),
        ("PostNumber", &m.post_number, "11000"),
        ("Municipality", &m.municipality, "Врачар"),
        ("Place", &m.place, "Београд"),
        ("StreetCode", &m.street_code, "00123"),
        ("Number", &m.number, "5"),
        ("InsuranceBasisRZZO", &m.insurance_basis_rzzo, "123"),
        (
            "InsuranceDescription",
            &m.insurance_description,
            "Самостална делатност",
        ),
        ("InsuranceStartDate", &m.insurance_start_date, "15.06.2021."),
        ("Country", &m.country, "Србија"),
        ("TaxpayerName", &m.taxpayer_name, "Петар Перић"),
        ("TaxpayerResidence", &m.taxpayer_residence, "Б"),
        ("TaxpayerIdNumber", &m.taxpayer_id_number, "0101990710020"),
    ];
    for (field, got, expected) in cases {
        assert_eq!(got, expected, "{field}");
    }
    assert!(m.permanently_valid, "PermanentlyValid");
}

#[test]
fn medical_get_document() {
    let [a, b, c, d] = medical_files();
    check_medical(&parse_medical_document(&a, &b, &c, &d).unwrap());
}

#[test]
fn medical_end_to_end() {
    let f = medical_files();
    for atr in [
        MEDICAL_ATR_1,
        MEDICAL_ATR_2,
        GEMALTO_ATR_2,
        GEMALTO_ATR_3,
        GEMALTO_ATR_4,
    ] {
        let card = VirtualCard::medical(atr, [&f[0], &f[1], &f[2], &f[3]]);
        assert_eq!(
            detect_card(&card).unwrap(),
            CardKind::Medical,
            "ATR {}",
            atr_to_string(atr)
        );
        match read_document(&card).unwrap() {
            Document::Medical(m) => check_medical(&m),
            other => panic!("expected medical document, got {other:?}"),
        }
    }
}

#[test]
fn medical_gender_male() {
    let [a, b, c, _] = medical_files();
    let admin = tlv(1603, b"01");
    let m = parse_medical_document(&a, &b, &c, &admin).unwrap();
    assert_eq!(m.gender, "Мушко");
    assert_eq!(m.taxpayer_id_number, "");
}

// ----------------------------------------------------------- Vehicle ----

fn vehicle_files() -> [Vec<u8>; 4] {
    let file0 = ber(
        &[0x71],
        &concat(&[
            ber(&[0x80], &[0x01, 0x00, 0x00]),
            ber(&[0x9F, 0x33], b"SRB"),
            ber(&[0x9F, 0x35], b"MUP Republike Srbije"),
            ber(&[0x9F, 0x36], b"PU Beograd"),
            ber(&[0x9F, 0x37], b"0"),
            ber(&[0x9F, 0x38], b"SRB1234"),
            ber(&[0x81], b"BG123AB"),
            ber(&[0x82], b"20200115"),
            ber(
                &[0xA1],
                &concat(&[
                    ber(
                        &[0xA2],
                        &concat(&[
                            ber(&[0x83], "Petrović".as_bytes()),
                            ber(&[0x84], b"Petar"),
                            ber(&[0x85], "Kneza Miloša 1, Beograd".as_bytes()),
                        ]),
                    ),
                    ber(&[0x86], b"1"),
                ]),
            ),
            ber(
                &[0xA3],
                &concat(&[
                    ber(&[0x87], b"ZASTAVA"),
                    ber(&[0x88], b"H"),
                    ber(&[0x89], b"101 SKALA"),
                ]),
            ),
            ber(&[0x8A], b"VX1234567890ABCDE"),
            ber(&[0xA4], &ber(&[0x8B], b"1500")),
            ber(&[0x8C], b"1100"),
            ber(&[0x8D], b"20251231"),
            ber(&[0x8E], b"20240101"),
            ber(&[0x8F], b"e1"),
            ber(
                &[0xA5],
                &concat(&[
                    ber(&[0x90], b"1100"),
                    ber(&[0x91], b"40"),
                    ber(&[0x92], b"BENZIN"),
                ]),
            ),
            ber(&[0x93], b"36"),
            ber(&[0xA6], &concat(&[ber(&[0x94], b"5"), ber(&[0x95], b"0")])),
        ]),
    );
    let file1 = ber(
        &[0x72],
        &concat(&[
            ber(&[0x80], &[0x01, 0x00, 0x00]),
            ber(&[0x98], b"M1"),
            ber(&[0x99], b"2"),
            ber(&[0xA5], &ber(&[0x9E], b"ENG123456")),
            ber(&[0x9F, 0x24], b"PLAVA"),
        ]),
    );
    let file2 = ber(
        &[0x72],
        &concat(&[
            ber(&[0x80], &[0x01, 0x00, 0x00]),
            ber(&[0xC2], b"0101990710020"),
            ber(&[0xC4], b"0"),
            ber(&[0xC5], b"2020"),
        ]),
    );
    let file3 = ber(
        &[0x72],
        &concat(&[
            ber(&[0x80], &[0x01, 0x00, 0x00]),
            ber(&[0xC9], b"SRB1234567890X"),
        ]),
    );
    [file0, file1, file2, file3]
}

fn check_vehicle(v: &celik_core::VehicleDocument) {
    let cases = [
        ("StateIssuing", &v.state_issuing, "SRB"),
        (
            "CompetentAuthority",
            &v.competent_authority,
            "MUP Republike Srbije",
        ),
        ("AuthorityIssuing", &v.authority_issuing, "PU Beograd"),
        ("UnambiguousNumber", &v.unambiguous_number, "SRB1234"),
        (
            "RegistrationNumberOfVehicle",
            &v.registration_number_of_vehicle,
            "BG123AB",
        ),
        (
            "DateOfFirstRegistration",
            &v.date_of_first_registration,
            "15.01.2020",
        ),
        (
            "OwnersSurnameOrBusinessName",
            &v.owners_surname_or_business_name,
            "Petrović",
        ),
        ("OwnerName", &v.owner_name, "Petar"),
        ("OwnerAddress", &v.owner_address, "Kneza Miloša 1, Beograd"),
        ("VehicleMake", &v.vehicle_make, "ZASTAVA"),
        ("VehicleType", &v.vehicle_type, "H"),
        (
            "CommercialDescription",
            &v.commercial_description,
            "101 SKALA",
        ),
        ("VehicleIdNumber", &v.vehicle_id_number, "VX1234567890ABCDE"),
        (
            "MaximumPermissibleLadenMass",
            &v.maximum_permissible_laden_mass,
            "1500",
        ),
        ("VehicleMass", &v.vehicle_mass, "1100"),
        ("ExpiryDate", &v.expiry_date, "31.12.2025"),
        ("IssuingDate", &v.issuing_date, "01.01.2024"),
        ("TypeApprovalNumber", &v.type_approval_number, "e1"),
        ("EngineCapacity", &v.engine_capacity, "1100"),
        ("MaximumNetPower", &v.maximum_net_power, "40"),
        ("TypeOfFuel", &v.type_of_fuel, "BENZIN"),
        ("PowerWeightRatio", &v.power_weight_ratio, "36"),
        ("NumberOfSeats", &v.number_of_seats, "5"),
        ("NumberOfStandingPlaces", &v.number_of_standing_places, "0"),
        ("VehicleCategory", &v.vehicle_category, "M1"),
        ("NumberOfAxles", &v.number_of_axles, "2"),
        ("EngineIdNumber", &v.engine_id_number, "ENG123456"),
        ("ColourOfVehicle", &v.colour_of_vehicle, "PLAVA"),
        ("OwnersPersonalNo", &v.owners_personal_no, "0101990710020"),
        ("VehicleLoad", &v.vehicle_load, "0"),
        ("YearOfProduction", &v.year_of_production, "2020"),
        ("SerialNumber", &v.serial_number, "SRB1234567890X"),
    ];
    for (field, got, expected) in cases {
        assert_eq!(got, expected, "{field}");
    }
}

#[test]
fn vehicle_get_document() {
    check_vehicle(&parse_vehicle_document(&vehicle_files()).unwrap());
}

#[test]
fn vehicle_end_to_end() {
    let f = vehicle_files();
    for atr in [
        VEHICLE_ATR_0,
        VEHICLE_ATR_2,
        VEHICLE_ATR_3,
        VEHICLE_ATR_4,
        GEMALTO_ATR_1,
        GEMALTO_ATR_4,
    ] {
        let card = VirtualCard::vehicle(atr, [&f[0], &f[1], &f[2], &f[3]]);
        assert_eq!(
            detect_card(&card).unwrap(),
            CardKind::Vehicle,
            "ATR {}",
            atr_to_string(atr)
        );
        match read_document(&card).unwrap() {
            Document::Vehicle(v) => check_vehicle(&v),
            other => panic!("expected vehicle document, got {other:?}"),
        }
    }
}

#[test]
fn vehicle_users_fallback_to_file_72() {
    let mut f = vehicle_files();
    f[3] = ber(
        &[0x72],
        &ber(
            &[0xA1],
            &ber(
                &[0xA9],
                &concat(&[ber(&[0x83], b"Firma d.o.o."), ber(&[0x85], b"Adresa 1")]),
            ),
        ),
    );
    let v = parse_vehicle_document(&f).unwrap();
    assert_eq!(v.users_surname_or_business_name, "Firma d.o.o.");
    assert_eq!(v.users_name, "");
    assert_eq!(v.users_address, "Adresa 1");
}

// ----------------------------------------------------------- Unknown ----

#[test]
fn unknown_cards() {
    let card = VirtualCard::new(vec![0x3B, 0x01, 0x02]);
    assert_eq!(detect_card(&card).unwrap(), CardKind::Unknown);
    assert!(matches!(read_document(&card), Err(CardError::UnknownCard(atr)) if atr == "3b0102"));
    assert_eq!(card.transaction_depth(), 0);

    // Known ATR, but nothing answers the probes.
    let card = VirtualCard::new(GEMALTO_ATR_2);
    assert_eq!(detect_card(&card).unwrap(), CardKind::Unknown);
}

// ------------------------------------------------------------ Crypto ----

const TEST_CERT: &[u8] = include_bytes!("data/test_cert.der");

fn crypto_card() -> VirtualCard {
    let (d, p, r) = id_files();
    VirtualCard::gemalto_id(GEMALTO_ATR_3, &d, &p, &r, PORTRAIT)
        .with_pin("1234", 3)
        .with_certificate(&[0x71, 0x02], TEST_CERT)
        .with_certificate(&[0x71, 0x03], TEST_CERT)
}

#[test]
fn crypto_requires_gemalto() {
    let f = medical_files();
    let card = VirtualCard::medical(MEDICAL_ATR_1, [&f[0], &f[1], &f[2], &f[3]]);
    assert!(matches!(
        IdCardCrypto::new(&card),
        Err(CardError::Unsupported)
    ));
}

#[test]
fn pin_tries_and_change() {
    let card = crypto_card();
    let crypto = IdCardCrypto::new(&card).unwrap();
    assert_eq!(crypto.pin_tries_left().unwrap(), 3);

    assert!(matches!(
        crypto.change_pin("12", "5678"),
        Err(CardError::InvalidPin)
    ));
    assert!(matches!(
        crypto.change_pin("1234", "56a8"),
        Err(CardError::InvalidPin)
    ));

    assert!(matches!(
        crypto.change_pin("0000", "5678"),
        Err(CardError::WrongPin(2))
    ));
    assert_eq!(crypto.pin_tries_left().unwrap(), 2);

    crypto.change_pin("1234", "5678").unwrap();
    assert_eq!(crypto.pin_tries_left().unwrap(), 3);
    assert!(matches!(
        crypto.change_pin("1234", "1111"),
        Err(CardError::WrongPin(2))
    ));
    assert!(matches!(
        crypto.change_pin("1234", "1111"),
        Err(CardError::WrongPin(1))
    ));
    assert!(matches!(
        crypto.change_pin("1234", "1111"),
        Err(CardError::PinBlocked)
    ));
    assert!(matches!(
        crypto.change_pin("5678", "1111"),
        Err(CardError::PinBlocked)
    ));
    assert_eq!(crypto.pin_tries_left().unwrap(), 0);
    assert_eq!(card.transaction_depth(), 0);
}

#[test]
fn certificates() {
    let card = crypto_card();
    let certs = IdCardCrypto::new(&card).unwrap().certificates().unwrap();
    assert_eq!(certs.len(), 2);
    let c = &certs[0];
    assert_eq!(c.der, TEST_CERT);
    assert_eq!(c.label, "CERT #123456");
    assert_eq!(c.serial_number().as_deref(), Some("123456789012345"));
    assert_eq!(
        c.subject_common_name().as_deref(),
        Some("Petar Petrovic 1234567890123")
    );
    assert!(c.subject().unwrap().contains("CN=Petar Petrovic"));
    assert!(c.not_after().unwrap() > c.not_before().unwrap());

    // One broken certificate file: the other one is still returned.
    let (d, p, r) = id_files();
    let card = VirtualCard::gemalto_id(GEMALTO_ATR_3, &d, &p, &r, PORTRAIT)
        .with_certificate(&[0x71, 0x02], TEST_CERT)
        .with_file(&[0x71, 0x03], vec![0x08, 0x00, 1, 2, 3, 4, 5, 6, 7, 8]);
    let certs = IdCardCrypto::new(&card).unwrap().certificates().unwrap();
    assert_eq!(certs.len(), 1);

    // No certificates at all.
    let card = VirtualCard::gemalto_id(GEMALTO_ATR_3, &d, &p, &r, PORTRAIT);
    assert!(IdCardCrypto::new(&card).unwrap().certificates().is_err());
}
