//! Vehicle registration PDF. Port of bas-celik (MIT, (c) Nikola Ubavić)
//! `VehicleDocument.BuildPdf` (`document/vehicle.go`).

use celik_core::VehicleDocument;

use super::canvas::{Canvas, FontStyle, Info};
use crate::Result;

const LEFT_MARGIN: f64 = 28.0;
const RIGHT_MARGIN: f64 = 535.0;
const TEXT_LEFT_MARGIN: f64 = 38.0;

struct W {
    pdf: Canvas,
}

impl W {
    fn new_line(&mut self) {
        let y = self.pdf.get_y() + 20.0;
        self.pdf.set_xy(TEXT_LEFT_MARGIN, y);
    }

    fn tab(&mut self) {
        let y = self.pdf.get_y();
        self.pdf.set_xy(343.0, y);
    }

    fn gap(&mut self) {
        let (x, y) = (self.pdf.get_x(), self.pdf.get_y());
        self.pdf.set_xy(x + 10.0, y);
    }

    fn cell(&mut self, s: &str) {
        self.pdf.cell(s);
    }

    fn put_data(&mut self, label: &str, data: &str) {
        self.cell(&format!("{label}: {data}"));
    }

    fn put_underline(&mut self, s: &str, size: f64) {
        self.pdf.set_font(FontStyle::Regular, true, size);
        self.pdf.cell(s);
        self.pdf.set_font(FontStyle::Bold, false, 12.0);
    }

    fn put_paragraph(&mut self, data: &str) {
        let texts: Vec<&str> = data.split(',').collect();
        if texts.len() == 2 {
            self.cell(&format!("{},", texts[0]));
            let y = self.pdf.get_y() + 14.0;
            self.pdf.set_xy(TEXT_LEFT_MARGIN, y);
            self.cell(texts[1].trim());
            let y = self.pdf.get_y() + 20.0;
            self.pdf.set_xy(TEXT_LEFT_MARGIN, y);
            return;
        }

        let texts = self.pdf.split_text_with_word_wrap(data, 500.0);
        for (i, text) in texts.iter().enumerate() {
            self.cell(text);
            if i < texts.len() - 1 {
                let y = self.pdf.get_y() + 14.0;
                self.pdf.set_xy(TEXT_LEFT_MARGIN, y);
            }
        }
        let y = self.pdf.get_y() + 20.0;
        self.pdf.set_xy(TEXT_LEFT_MARGIN, y);
    }
}

fn dash(s: &str) -> &str {
    if s.is_empty() {
        "-"
    } else {
        s
    }
}

pub fn build(doc: &VehicleDocument) -> Result<Vec<u8>> {
    let mut w = W { pdf: Canvas::new()? };

    w.pdf.set_font(FontStyle::Bold, false, 29.0);
    w.pdf.set_xy(TEXT_LEFT_MARGIN, 35.0);
    w.cell("Čitač saobraćajne dozvole");

    w.pdf.set_line_width(2.9);
    w.pdf.line(LEFT_MARGIN, 72.0, RIGHT_MARGIN, 72.0);

    w.pdf.set_xy(TEXT_LEFT_MARGIN, 90.0);
    w.pdf.set_font_size(21.0);
    w.cell(&format!("Registarska oznaka: {}", doc.registration_number_of_vehicle));

    w.pdf.set_xy(TEXT_LEFT_MARGIN, 145.0);
    w.pdf.set_font_size(12.0);

    w.put_data("Datum izdavanja", &doc.issuing_date);
    w.tab();
    w.put_underline(&format!("Važi do: {}", doc.expiry_date), 12.0);
    w.new_line();

    w.put_data("Saobraćajnu izdao", &doc.state_issuing);
    w.tab();
    w.put_data("Zabrana otuđenja", "");
    w.new_line();
    w.put_paragraph(&format!("{},\n{}", doc.authority_issuing, doc.competent_authority));

    w.put_data("Broj saobraćajne", &doc.unambiguous_number);
    w.new_line();

    w.put_data("Serijski broj", &doc.serial_number);
    w.new_line();

    w.pdf.set_xy(TEXT_LEFT_MARGIN, 272.0);
    w.put_underline("Podaci o vlasniku", 20.0);
    let y = w.pdf.get_y() + 25.0;
    w.pdf.set_xy(TEXT_LEFT_MARGIN, y);

    for (label, value) in [
        ("Vlasnik", &doc.owners_surname_or_business_name),
        ("Ime vlasnika", &doc.owner_name),
        ("Adresa vlasnika", &doc.owner_address),
        ("Jmbg vlasnika", &doc.owners_personal_no),
        ("Korisnik", &doc.users_surname_or_business_name),
        ("Ime korisnika", &doc.users_name),
        ("Adresa korisnika", &doc.users_address),
        ("Jmbg korisnika", &doc.users_personal_no),
    ] {
        w.put_data(label, value);
        w.new_line();
    }

    let y = w.pdf.get_y() + 6.0;
    w.pdf.set_xy(TEXT_LEFT_MARGIN, y);
    w.put_underline("Podaci o vozilu", 20.0);
    let y = w.pdf.get_y() + 25.0;
    w.pdf.set_xy(TEXT_LEFT_MARGIN, y);

    w.put_data("Datum prve registracije", &doc.date_of_first_registration);
    w.tab();
    w.put_data("Godina proizvodnje", &doc.year_of_production);
    w.new_line();

    w.put_data("Marka", &doc.vehicle_make);
    w.gap();
    w.put_data("Model", &doc.commercial_description);
    w.new_line();

    w.put_data("Tip", dash(&doc.vehicle_type));
    w.gap();
    w.put_data("Homologacijska oznaka", dash(&doc.homologation_mark));
    w.new_line();

    w.put_data("Boja", &doc.colour_of_vehicle);
    w.tab();
    w.put_data("Broj osovina", &doc.number_of_axles);
    w.new_line();

    w.put_data("Broj šasije", &doc.vehicle_id_number);
    w.tab();
    w.put_data("Zapremina motora", &doc.engine_capacity);
    w.new_line();

    w.put_data("Broj motora", &doc.engine_id_number);
    w.tab();
    w.put_data("Masa", &doc.vehicle_mass);
    w.new_line();

    w.put_data("Snaga motora", &doc.maximum_net_power);
    w.tab();
    w.put_data("Nosivost", &doc.vehicle_load);
    w.new_line();

    w.put_data("Odnos snaga/masa", &doc.power_weight_ratio);
    w.tab();
    w.cell("Najveća dozvoljena");
    w.new_line();

    w.put_data("Kategorija", &doc.vehicle_category);
    w.tab();
    w.put_data("masa", &doc.maximum_permissible_laden_mass);
    w.new_line();

    w.put_data("Pogonsko gorivo", &doc.type_of_fuel);
    w.new_line();

    w.put_data("Broj mesta za sedenje", &doc.number_of_seats);
    w.tab();
    w.put_data("Broj mesta za stajanje", &doc.number_of_standing_places);
    w.new_line();

    w.pdf.finish(Info {
        title: format!("{} {}", doc.vehicle_make, doc.commercial_description),
        subject: "Saobraćajna dozvola".into(),
    })
}
