//! Medical insurance card PDF.

use celik_core::localization::{format_yes_no, Language};
use celik_core::MedicalDocument;

use super::canvas::{Canvas, FontStyle, Image, Info};
use crate::{ExportError, Result};

static RFZO_LOGO: &[u8] = include_bytes!("../../../../assets/rfzo.png");

const LEFT_MARGIN: f64 = 30.464;
const RIGHT_MARGIN: f64 = 535.0;
const TEXT_LEFT_MARGIN: f64 = 38.243;

fn section(pdf: &mut Canvas, name: &str) {
    let y = pdf.get_y() + 8.0;
    pdf.line(LEFT_MARGIN, y, RIGHT_MARGIN, y);
    pdf.set_xy(TEXT_LEFT_MARGIN, y + 12.0);
    pdf.cell(name);
    pdf.line(LEFT_MARGIN, y + 32.0, RIGHT_MARGIN, y + 32.0);
    pdf.set_xy(TEXT_LEFT_MARGIN, y + 41.0);
}

fn put_data(pdf: &mut Canvas, label: &str, data: &str) {
    pdf.cell(label);
    let y = pdf.get_y();
    pdf.set_xy(TEXT_LEFT_MARGIN + 144.0, y);
    let texts = pdf.split_text_with_word_wrap(data, 350.0);
    for (i, text) in texts.iter().enumerate() {
        pdf.cell(text);
        if i < texts.len() - 1 {
            let y = pdf.get_y() + 11.0;
            pdf.set_xy(TEXT_LEFT_MARGIN + 144.0, y);
        }
    }
    let y = pdf.get_y() + 14.0;
    pdf.set_xy(TEXT_LEFT_MARGIN, y);
}

fn with_latin(cyr: &str, lat: &str) -> String {
    format!("{cyr} ({lat})")
}

pub fn build(doc: &MedicalDocument) -> Result<Vec<u8>> {
    let mut pdf = Canvas::new()?;
    pdf.set_font(FontStyle::Regular, false, 11.0);
    pdf.set_line_width(0.58);

    let logo = Image::from_bytes(RFZO_LOGO)
        .ok_or_else(|| ExportError::Pdf("decoding RFZO logo".into()))?;
    pdf.image(logo, 36.0, 14.0, 144.0, 51.0);

    pdf.set_xy(218.6, 49.6);
    pdf.cell("ПРЕГЛЕД КАРТИЦЕ ЗДРАВСТВЕНОГ ОСИГУРАЊА (КЗО)");

    pdf.set_y(68.0);
    let p = &mut pdf;
    section(p, "Општи подаци о осигуранику");
    put_data(
        p,
        "Име:",
        &with_latin(&doc.given_name, &doc.given_name_latin),
    );
    put_data(
        p,
        "Име једног родитеља:",
        &with_latin(&doc.parent_name, &doc.parent_name_latin),
    );
    put_data(
        p,
        "Презиме:",
        &with_latin(&doc.family_name, &doc.family_name_latin),
    );
    put_data(p, "Датум рођења:", &doc.date_of_birth);
    put_data(p, "Место, општина и држава:", &doc.full_place_address());
    put_data(p, "Улица:", &doc.full_street_address());
    put_data(p, "Пол:", &doc.gender);
    put_data(p, "Језик:", &doc.print_language);
    put_data(p, "ЛБО:", &doc.insurant_number);
    put_data(p, "ЈМБГ:", &doc.personal_number);

    section(p, "Подаци о картици здравственог осигурања");
    put_data(p, "Датум издавања:", &doc.date_of_issue);
    put_data(p, "Датум важења:", &doc.date_of_expiry);
    put_data(p, "Оверена до:", &doc.valid_until);
    put_data(
        p,
        "Трајно оверена:",
        format_yes_no(doc.permanently_valid, Language::SrCyrillic),
    );

    section(p, "Подаци о носиоцу осигурања");
    put_data(
        p,
        "Име:",
        &with_latin(&doc.carrier_given_name, &doc.carrier_given_name_latin),
    );
    put_data(
        p,
        "Презиме:",
        &with_latin(&doc.carrier_family_name, &doc.carrier_family_name_latin),
    );
    put_data(p, "ЛБО:", &doc.carrier_insurant_number);
    put_data(p, "ЈМБГ:", &doc.carrier_id_number);
    put_data(
        p,
        "Члан породице:",
        format_yes_no(doc.carrier_family_member, Language::SrCyrillic),
    );
    put_data(p, "Сродство:", &doc.carrier_relationship);

    section(p, "Подаци о осигурању");
    put_data(p, "Основ осигурања:", &doc.insurance_basis_rzzo);
    put_data(p, "Датум почетка осигурања:", &doc.insurance_start_date);
    put_data(p, "Опис:", &doc.insurance_description);

    section(p, "Подаци о обвезнику плаћања доприноса");
    put_data(p, "Назив:", &doc.taxpayer_name);
    put_data(p, "Седиште:", &doc.taxpayer_residence);
    put_data(p, "Регистарски број:", &doc.taxpayer_number);
    put_data(p, "ПИБ/ЈМБГ:", &doc.taxpayer_id_number);
    put_data(p, "Делатност:", &doc.taxpayer_activity_code);

    pdf.finish(Info {
        title: format!("{} {}", doc.given_name_latin, doc.family_name_latin),
        subject: "Kartica zdravstvenog osiguranja".into(),
    })
}
