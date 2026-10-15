//! Integration tests for celik-export, using sample documents with known expected output.

use celik_core::{Document, IdDocument, MedicalDocument, VehicleDocument};
use celik_export::{file_name, to_json, to_pdf, to_xlsx, PdfOptions};

static PORTRAIT: &[u8] = include_bytes!("fixtures/portrait.jpg");

fn s(v: &str) -> String {
    v.to_string()
}

fn id_doc() -> IdDocument {
    IdDocument {
        portrait: PORTRAIT.to_vec(),
        doc_reg_no: s("012345678"),
        document_type: s("ID"),
        issuing_date: s("01.03.2020."),
        expiry_date: s("01.03.2030."),
        issuing_authority: s("ПУ ЗА ГРАД БЕОГРАД"),
        personal_number: s("0101990710006"),
        surname: s("ПЕТРОВИЋ"),
        given_name: s("ПЕТАР"),
        parent_given_name: s("МАРКО"),
        sex: s("M"),
        place_of_birth: s("БЕОГРАД"),
        community_of_birth: s("САВСКИ ВЕНАЦ"),
        state_of_birth: s("РЕПУБЛИКА СРБИЈА"),
        state_of_birth_code: s("SRB"),
        date_of_birth: s("01.01.1990."),
        state: s("SRB"),
        community: s("САВСКИ ВЕНАЦ"),
        place: s("БЕОГРАД"),
        street: s("КНЕЗА МИЛОША"),
        house_number: s("12"),
        house_letter: s("А"),
        floor: s("3"),
        apartment_number: s("15"),
        address_date: s("15.05.2021."),
        address_label: s("Пребивалиште и адреса стана"),
        ..Default::default()
    }
}

fn foreigner_doc(kind: &str) -> IdDocument {
    IdDocument {
        document_type: s(kind),
        nationality_full: s("Руска Федерација"),
        purpose_of_stay: s("РАД"),
        e_note: s("Напомена <тест> & више"),
        document_name: s("ДОЗВОЛА ЗА ПРИВРЕМЕНИ БОРАВАК И РАД"),
        ..id_doc()
    }
}

fn medical_doc() -> MedicalDocument {
    MedicalDocument {
        insurer_name: s("Републички фонд за здравствено осигурање"),
        insurer_id: s("61"),
        card_id: s("12345678901"),
        date_of_issue: s("01.02.2019"),
        date_of_expiry: s("01.02.2029"),
        print_language: s("српски"),
        personal_number: s("0101990710006"),
        family_name_latin: s("PETROVIĆ"),
        given_name_latin: s("PETAR"),
        parent_name_latin: s("MARKO"),
        family_name: s("ПЕТРОВИЋ"),
        given_name: s("ПЕТАР"),
        parent_name: s("МАРКО"),
        gender: s("Мушки"),
        insurant_number: s("10987654321"),
        date_of_birth: s("01.01.1990"),
        apartment: s("15"),
        number: s("12"),
        street: s("КНЕЗА МИЛОША"),
        place: s("БЕОГРАД"),
        municipality: s("САВСКИ ВЕНАЦ"),
        country: s("СРБИЈА"),
        valid_until: s("31.12.2026"),
        carrier_given_name_latin: s("PETAR"),
        carrier_family_name_latin: s("PETROVIĆ"),
        carrier_given_name: s("ПЕТАР"),
        carrier_family_name: s("ПЕТРОВИЋ"),
        carrier_id_number: s("0101990710006"),
        carrier_insurant_number: s("10987654321"),
        insurance_basis_rzzo: s("11"),
        insurance_start_date: s("01.02.2015"),
        insurance_description: s(
            "Запослени у правном лицу са веома дугачким описом основа осигурања који прелази у следећи ред",
        ),
        taxpayer_name: s("ПРЕДУЗЕЋЕ ДОО"),
        taxpayer_residence: s("БЕОГРАД"),
        taxpayer_number: s("12345678"),
        taxpayer_id_number: s("100000000"),
        taxpayer_activity_code: s("6201"),
        ..Default::default()
    }
}

fn vehicle_doc() -> VehicleDocument {
    VehicleDocument {
        authority_issuing: s("MUP R. SRBIJE"),
        colour_of_vehicle: s("CRNA"),
        commercial_description: s("GOLF"),
        competent_authority: s("PU ZA GRAD BEOGRAD"),
        date_of_first_registration: s("15.06.2015"),
        engine_capacity: s("1598"),
        engine_id_number: s("CLH123456"),
        expiry_date: s("14.06.2027"),
        issuing_date: s("15.06.2026"),
        maximum_net_power: s("77"),
        maximum_permissible_laden_mass: s("1820"),
        number_of_axles: s("2"),
        number_of_seats: s("5"),
        owner_address: s("BEOGRAD, KNEZA MILOŠA 12"),
        owner_name: s("PETAR"),
        owners_personal_no: s("0101990710006"),
        owners_surname_or_business_name: s("PETROVIĆ"),
        power_weight_ratio: s("0.06"),
        registration_number_of_vehicle: s("BG123AB"),
        serial_number: s("000123456"),
        state_issuing: s("SRB"),
        type_of_fuel: s("DIZEL"),
        unambiguous_number: s("SRB000123456"),
        vehicle_category: s("M1"),
        vehicle_id_number: s("WVWZZZ1KZFW123456"),
        vehicle_load: s("500"),
        vehicle_make: s("VOLKSWAGEN"),
        vehicle_mass: s("1320"),
        vehicle_type: s("1K"),
        year_of_production: s("2015"),
        ..Default::default()
    }
}

fn all_docs() -> Vec<(&'static str, Document)> {
    vec![
        ("id", Document::Id(Box::new(id_doc()))),
        ("if", Document::Id(Box::new(foreigner_doc("IF")))),
        ("rp", Document::Id(Box::new(foreigner_doc("RP")))),
        ("med", Document::Medical(Box::new(medical_doc()))),
        ("veh", Document::Vehicle(Box::new(vehicle_doc()))),
    ]
}

/// Optionally write outputs for visual inspection: `CELIK_EXPORT_OUT=/some/dir cargo test`.
fn dump(name: &str, data: &[u8]) {
    if let Ok(dir) = std::env::var("CELIK_EXPORT_OUT") {
        std::fs::write(std::path::Path::new(&dir).join(name), data).unwrap();
    }
}

/// Concatenated text of page 1 (via ToUnicode maps).
fn pdf_text(pdf: &[u8]) -> String {
    let doc = lopdf::Document::load_mem(pdf).expect("pdf parses");
    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    assert_eq!(pages, vec![1]);
    doc.extract_text(&pages).expect("text extraction")
}

fn info_string(pdf: &[u8], key: &[u8]) -> String {
    let doc = lopdf::Document::load_mem(pdf).unwrap();
    let info_ref = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
    let info = doc.get_dictionary(info_ref).unwrap();
    let raw = info.get(key).unwrap().as_str().unwrap();
    lopdf::decode_text_string(info.get(key).unwrap())
        .unwrap_or_else(|_| String::from_utf8_lossy(raw).into())
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn pdfs_parse_and_contain_text() {
    for (name, doc) in all_docs() {
        let pdf = to_pdf(&doc, PdfOptions::default()).unwrap();
        dump(&format!("rs_{name}.pdf"), &pdf);
        assert!(pdf.starts_with(b"%PDF-"));
        let text = squash(&pdf_text(&pdf));
        let expected: &[&str] = match name {
            "id" => &[
                "ČITAČELEKTRONSKELIČNEKARTE:ŠTAMPAPODATAKA",
                "Prezime:",
                "ПЕТРОВИЋ",
                "Mestorođenja",
                "БЕОГРАД,САВСКИВЕНАЦ,КНЕЗАМИЛОША12А/3/15",
                "JMBG:",
                "0101990710006",
                "Datumštampe:",
                "skraćujesenaprvadvakaraktera",
            ],
            "if" => &[
                "Podaciostrancu",
                "Državljanstvo:",
                "РускаФедерација",
                "SRB,БЕОГРАД",
            ],
            "rp" => &["Napomena:", "Напомена<тест>&више", "Nazivdokumenta:"],
            "med" => &[
                "ПРЕГЛЕДКАРТИЦЕЗДРАВСТВЕНОГОСИГУРАЊА(КЗО)",
                "ПЕТРОВИЋ(PETROVIĆ)",
                "КНЕЗАМИЛОША,Број:12Стан:15",
                "Трајнооверена:Не",
                "Делатност:6201",
            ],
            "veh" => &[
                "Čitačsaobraćajnedozvole",
                "Registarskaoznaka:BG123AB",
                "Važido:14.06.2027",
                "MUPR.SRBIJE,",
                "PUZAGRADBEOGRAD",
                "Brojšasije:WVWZZZ1KZFW123456",
                "Homologacijskaoznaka:-",
            ],
            _ => unreachable!(),
        };
        for e in expected {
            assert!(text.contains(e), "{name}: missing {e:?} in {text}");
        }
    }
}

#[test]
fn id_pdf_cyrillic_labels() {
    let doc = Document::Id(Box::new(id_doc()));
    let pdf = to_pdf(
        &doc,
        PdfOptions {
            cyrillic_labels: true,
        },
    )
    .unwrap();
    dump("rs_id_cyr.pdf", &pdf);
    let text = squash(&pdf_text(&pdf));
    assert!(text.contains("ЧИТАЧЕЛЕКТРОНСКЕЛИЧНЕКАРТЕ:ШТАМПАПОДАТАКА"));
    assert!(text.contains("Презиме:"));
    assert!(text.contains("ЈМБГ:"));
    assert!(!text.contains("Prezime:"));
}

#[test]
fn pdf_metadata() {
    let pdf = to_pdf(
        &Document::Medical(Box::new(medical_doc())),
        PdfOptions::default(),
    )
    .unwrap();
    assert_eq!(info_string(&pdf, b"Title"), "PETAR PETROVIĆ");
    assert_eq!(info_string(&pdf, b"Author"), "Chitach");
    assert_eq!(info_string(&pdf, b"Creator"), "Chitach");
}

#[test]
fn pdf_without_portrait() {
    let mut d = id_doc();
    d.portrait.clear();
    let pdf = to_pdf(&Document::Id(Box::new(d)), PdfOptions::default()).unwrap();
    assert!(pdf_text(&pdf).contains("Prezime:"));
}

#[test]
fn json_roundtrip_and_shape() {
    for (name, doc) in all_docs() {
        let json = to_json(&doc).unwrap();
        dump(&format!("rs_{name}.json"), &json);
        let text = std::str::from_utf8(&json).unwrap();
        assert!(!text.contains('\n'));
        let back = match &doc {
            Document::Id(_) => {
                assert!(text.starts_with("{\"Portrait\":\"/9j/"), "{text}");
                Document::Id(serde_json::from_slice(&json).unwrap())
            }
            Document::Medical(_) => {
                assert!(text.starts_with("{\"InsurerName\":"));
                assert!(text.contains("\"InsurerID\":\"61\""));
                assert!(text.contains("\"InsuranceBasisRZZO\":\"11\""));
                assert!(text.contains("\"PermanentlyValid\":false"));
                Document::Medical(serde_json::from_slice(&json).unwrap())
            }
            Document::Vehicle(_) => {
                assert!(text.starts_with("{\"AuthorityIssuing\":"));
                Document::Vehicle(serde_json::from_slice(&json).unwrap())
            }
        };
        assert_eq!(back, doc);
    }
}

#[test]
fn json_html_safe_escaping() {
    let json = to_json(&Document::Id(Box::new(foreigner_doc("RP")))).unwrap();
    let text = String::from_utf8(json).unwrap();
    assert!(
        text.contains(r#""ENote":"Напомена \u003cтест\u003e \u0026 више""#),
        "{text}"
    );
}

#[test]
fn json_field_order() {
    let json =
        String::from_utf8(to_json(&Document::Vehicle(Box::new(vehicle_doc()))).unwrap()).unwrap();
    assert!(json.starts_with(r#"{"AuthorityIssuing":"MUP R. SRBIJE","ColourOfVehicle":"CRNA","#));
    assert!(json.ends_with(r#""VehicleType":"1K","YearOfProduction":"2015"}"#));
}

#[test]
fn xlsx_is_zip_with_sheet() {
    for (name, doc) in all_docs() {
        let x = to_xlsx(&doc).unwrap();
        dump(&format!("rs_{name}.xlsx"), &x);
        assert!(x.starts_with(b"PK\x03\x04"), "{name}");
    }
}

#[test]
fn file_names() {
    assert_eq!(
        file_name(&Document::Id(Box::new(id_doc())), "pdf"),
        "петар_петровић.pdf"
    );
    assert_eq!(
        file_name(&Document::Medical(Box::new(medical_doc())), "json"),
        "petar_petrović.json"
    );
    assert_eq!(
        file_name(&Document::Vehicle(Box::new(vehicle_doc())), "xlsx"),
        "bg123ab_petrović_petar.xlsx"
    );
}
