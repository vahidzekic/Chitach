//! ID card PDF.

use celik_core::document::{
    ID_TYPE_APOLLO, ID_TYPE_ID, ID_TYPE_IDENTITY_FOREIGNER, ID_TYPE_RESIDENCE_PERMIT,
};
use celik_core::localization::{cyrillic_to_latin, join_with_comma};
use celik_core::IdDocument;

use super::canvas::{Canvas, FontStyle, Image, Info};
use crate::Result;

struct IdPdfWriter<'a> {
    pdf: Canvas,
    left_margin: f64,
    right_margin: f64,
    text_left_margin: f64,
    doc: &'a IdDocument,
    cyrillic_labels: bool,
    print_date: String,
}

impl IdPdfWriter<'_> {
    fn line(&mut self, width: f64) {
        if width > 0.0 {
            self.pdf.set_line_width(width);
        }
        let y = self.pdf.get_y();
        self.pdf.line(self.left_margin, y, self.right_margin, y);
    }

    fn move_y(&mut self, y: f64) {
        let (x, cy) = (self.pdf.get_x(), self.pdf.get_y());
        self.pdf.set_xy(x, cy + y);
    }

    fn cell(&mut self, s: &str, transliterate: bool) {
        if transliterate && !self.cyrillic_labels {
            self.pdf.cell(&cyrillic_to_latin(s));
        } else {
            self.pdf.cell(s);
        }
    }

    fn put_data(&mut self, label: &str, data: &str) {
        let y = self.pdf.get_y();
        let label = if self.cyrillic_labels {
            label.to_string()
        } else {
            cyrillic_to_latin(label)
        };

        self.pdf.set_x(self.text_left_margin);
        let texts = self.pdf.split_text_with_word_wrap(&label, 120.0);
        for (i, text) in texts.iter().enumerate() {
            self.cell(text, false);
            if i < texts.len() - 1 {
                let ny = self.pdf.get_y() + 12.0;
                self.pdf.set_xy(self.text_left_margin, ny);
            }
        }
        let y1 = self.pdf.get_y();

        self.pdf.set_xy(self.text_left_margin + 128.0, y);
        let texts = self.pdf.split_text_with_word_wrap(data, 350.0);
        for (i, text) in texts.iter().enumerate() {
            self.cell(text, false);
            if i < texts.len() - 1 {
                let ny = self.pdf.get_y() + 12.0;
                self.pdf.set_xy(self.text_left_margin + 128.0, ny);
            }
        }
        let y2 = self.pdf.get_y();

        self.pdf.set_xy(self.text_left_margin, y1.max(y2) + 24.67);
    }

    fn address_label(&self) -> &'static str {
        if self.doc.address_label == "prebivalište" {
            "Пребивалиште:"
        } else {
            "Пребивалиште\nи адреса стана:"
        }
    }

    /// Header line, title, portrait with frame. `title_y`, `line_y`, `image_y` differ per document type.
    fn header(&mut self, title_y: f64, line_y: f64, image_y: f64) {
        self.pdf.set_y(59.041);
        self.line(0.83);
        self.pdf.set_xy(self.text_left_margin + 1.0, title_y);
        self.pdf.set_char_spacing(-0.2);
        self.cell("ЧИТАЧ ЕЛЕКТРОНСКЕ ЛИЧНЕ КАРТЕ: ШТАМПА ПОДАТАКА", true);
        self.pdf.set_char_spacing(-0.1);
        self.pdf.set_y(line_y);
        self.line(0.0);

        let image_height = 159.0;
        if let Some(img) = Image::from_bytes(&self.doc.portrait) {
            self.pdf
                .image(img, self.left_margin, image_y, 119.9, image_height);
        }
        self.pdf.set_line_width(0.48);
        self.pdf
            .rectangle_stroke(self.left_margin, image_y, 179.0, image_y + image_height);
    }

    fn section_title(&mut self, title: &str) {
        self.move_y(-8.67);
        self.line(0.0);
        self.move_y(9.0);
        self.cell(title, true);
        self.move_y(16.0);
        self.line(0.0);
        self.move_y(9.0);
    }

    fn print_date(&mut self) {
        self.move_y(-8.67);
        self.line(0.0);
        self.move_y(3.0);
        self.line(0.0);
        self.move_y(9.0);
        let s = format!("Датум штампе: {}", self.print_date);
        self.cell(&s, true);
        self.move_y(19.0);
    }

    fn footnotes(&mut self, lines: &[&str]) {
        for (i, l) in lines.iter().enumerate() {
            if i > 0 {
                self.pdf.set_x(self.left_margin);
                self.move_y(9.7);
            }
            self.cell(l, false);
        }
    }

    fn print_regular_id(&mut self) {
        self.header(68.5, 88.0, 102.8);

        self.pdf.set_y(276.0);
        self.line(1.08);
        self.move_y(8.0);
        self.pdf.set_x(self.text_left_margin);
        self.pdf.set_font_size(11.1);

        self.cell("Подаци о грађанину", true);
        self.move_y(16.0);
        self.line(0.0);
        self.move_y(9.0);

        let d = self.doc;
        self.put_data("Презиме:", &d.surname);
        self.put_data("Име:", &d.given_name);
        self.put_data("Име једног родитеља:", &d.parent_given_name);
        self.put_data("Датум рођења:", &d.date_of_birth);
        self.put_data("Место рођења\nопштина и држава:", &d.full_place_of_birth());
        self.put_data(self.address_label(), &d.full_address(true));
        self.put_data("Датум промене адресе:", &d.address_date);
        self.put_data("ЈМБГ:", &d.personal_number);
        self.put_data("Пол:", &d.sex);

        self.section_title("Подаци о документу");
        self.put_data("Документ издаје:", &d.issuing_authority);
        self.put_data("Број документа:", &d.doc_reg_no);
        self.put_data("Датум издавања:", &d.issuing_date);
        self.put_data("Важи до:", &d.expiry_date);

        self.print_date();

        if self.pdf.get_y() < 700.0 {
            self.pdf.set_y(730.6);
        }
        self.line(0.83);
        self.pdf.set_font_size(9.0);
        self.move_y(10.0);
        self.pdf.set_x(self.left_margin);

        if self.cyrillic_labels {
            self.footnotes(&[
                "1. У чипу личне карте, подаци о имену и презимену имаоца личне карте исписани су на националном писму онако",
                "како су исписани на самом обрасцу личне карте, док су остали подаци исписани латиничким писмом.",
                "2. Ако се име лица састоји од две речи чија је укупна дужина између 20 и 30 карактера или презимена од две речи",
                "чија је укупна дужина између 30 и 36 карактера, у чипу личне карте издате пре 18.08.2014. године, друга реч у",
                "имену или презимену скраћује се на прва два карактера",
            ]);
        } else {
            self.footnotes(&[
                "1. U čipu lične karte, podaci o imenu i prezimenu imaoca lične karte ispisani su na nacionalnom pismu onako kako su",
                "ispisani na samom obrascu lične karte, dok su ostali podaci ispisani latiničkim pismom.",
                "2. Ako se ime lica sastoji od dve reči čija je ukupna dužina između 20 i 30 karaktera ili prezimena od dve reči čija je",
                "ukupna dužina između 30 i 36 karaktera, u čipu lične karte izdate pre 18.08.2014. godine, druga reč u imenu ili prezimenu",
                "skraćuje se na prva dva karaktera",
            ]);
        }

        self.move_y(15.7);
        self.line(0.0);
    }

    fn foreigner_start(&mut self) {
        self.header(64.95, 79.8, 86.0);
        self.pdf.set_y(250.0);
        self.line(1.08);
        self.move_y(8.0);
        self.pdf.set_x(self.text_left_margin);
        self.pdf.set_font_size(11.1);
        self.cell("Подаци о странцу", true);
        self.move_y(16.0);
        self.line(0.0);
        self.move_y(9.0);
    }

    fn foreigner_end(&mut self, cyr: &[&str], lat: &[&str]) {
        self.print_date();
        self.line(0.83);
        self.pdf.set_font_size(9.0);
        self.move_y(4.0);
        self.pdf.set_x(self.left_margin);
        if self.cyrillic_labels {
            self.footnotes(cyr);
        } else {
            self.footnotes(lat);
        }
        self.move_y(9.7);
        self.line(0.0);
    }

    fn print_foreigner_id(&mut self) {
        self.foreigner_start();
        let d = self.doc;
        self.put_data("Презиме:", &d.surname);
        self.put_data("Име:", &d.given_name);
        self.put_data("Држављанство:", &d.nationality_full);
        self.put_data("Датум рођења:", &d.date_of_birth);
        self.put_data("Основ боравка:", &d.purpose_of_stay);
        self.put_data(
            self.address_label(),
            &join_with_comma(&[&d.state, &d.full_address(true)]),
        );
        self.put_data("Датум промене адресе:", &d.address_date);
        self.put_data("Евиденцијски број\nстранца:", &d.personal_number);
        self.put_data("Пол:", &d.sex);

        self.section_title("Подаци о документу");
        self.put_data("Документ издаје:", &d.issuing_authority);
        self.put_data("Број документа:", &d.doc_reg_no);
        self.put_data("Датум издавања:", &d.issuing_date);
        self.put_data("Важи до:", &d.expiry_date);

        self.foreigner_end(
            &[
                "1. У чипу личне карте за странце, подаци о имену и презимену странца исписани су онако како су исписани на",
                "самом обрасцу личне карте за странца латиничним писмом.",
                "2. Ако се име или презиме странца састоји од две или више речи чија дужина прелази 30 карактера за име, односно",
                "односно 36 карактера за презиме, у чип се уписује пуно име и презиме странца, а на обрасцу личне карте за",
                "странца се уписује до 30 карактера за име, односно 36 карактера за презиме.",
            ],
            &[
                "1. U čipu lične karte za strance, podaci o imenu i prezimenu stranca ispisani su onako kako su ispisani na samom",
                "obrascu lične karte za stranca latiničnim pismom.",
                "2. Ako se ime ili prezime stranca sastoji od dve ili više reči čija dužina prelazi 30 karaktera za ime, odnosno 36",
                "karaktera za prezime, u čip se upisuje puno ime i prezime stranca, a na obrascu lične karte za stranca se upisuje do",
                "30 karaktera za ime, odnosno 36 karaktera za prezime.",
            ],
        );
    }

    fn print_residence_permit(&mut self) {
        self.foreigner_start();
        let d = self.doc;
        self.put_data("Презиме:", &d.surname);
        self.put_data("Име:", &d.given_name);
        self.put_data("Држављанство:", &d.nationality_full);
        self.put_data("Датум рођења:", &d.date_of_birth);
        self.put_data("Место рођења,\nопштина и држава:", &d.full_place_of_birth());
        self.put_data(self.address_label(), &d.full_address(true));
        self.put_data("Датум промене адресе:", &d.address_date);
        self.put_data("Евиденцијски број\nстранца:", &d.personal_number);
        self.put_data("Пол:", &d.sex);
        self.put_data("Основ боравка:", &d.purpose_of_stay);
        self.put_data("Напомена:", &d.e_note);

        self.section_title("Подаци о документу");
        self.put_data("Назив документа:", &d.document_name);
        self.put_data("Документ издаје:", &d.issuing_authority);
        self.put_data("Број документа:", &d.doc_reg_no);
        self.put_data("Датум издавања:", &d.issuing_date);
        self.put_data("Важи до:", &d.expiry_date);

        self.foreigner_end(
            &[
                "1. У чипу дозволе за привремени боравак и рад, подаци о имену и презимену имаоца дозволе исписани су онако",
                "како су исписани на самом обрасцу дозволе за привремени боравак латиничним писмом.",
                "2. Ако се име или презиме странца састоји од две или више речи чија дужина прелази 30 карактера за име,",
                "односно 36 карактера за презиме у чип се уписује пуно име странца, а на обрасцу дозволе за привремени боравак",
                "се уписује до 30 карактера за име, односно 36 карактера за презиме.",
            ],
            &[
                "1. U čipu dozvole za privremeni boravak i rad, podaci o imenu i prezimenu imaoca dozvole ispisani su onako kako su",
                "ispisani na samom obrascu dozvole za privremeni boravak latiničnim pismom.",
                "2. Ako se ime ili prezime stranca sastoji od dve ili više reči čija dužina prelazi 30 karaktera za ime, odnosno 36 karaktera",
                "za prezime u čip se upisuje puno ime stranca, a na obrascu dozvole za privremeni boravak se upisuje do 30 karaktera za",
                "ime, odnosno 36 karaktera za prezime.",
            ],
        );
    }
}

pub fn build(doc: &IdDocument, cyrillic_labels: bool, print_date: &str) -> Result<Vec<u8>> {
    let mut pdf = Canvas::new()?;
    pdf.set_font(FontStyle::Regular, false, 13.5);

    let mut w = IdPdfWriter {
        pdf,
        left_margin: 58.8,
        right_margin: 535.0,
        text_left_margin: 67.3,
        doc,
        cyrillic_labels,
        print_date: print_date.to_string(),
    };

    match doc.document_type.as_str() {
        ID_TYPE_APOLLO | ID_TYPE_ID => w.print_regular_id(),
        ID_TYPE_IDENTITY_FOREIGNER => w.print_foreigner_id(),
        ID_TYPE_RESIDENCE_PERMIT => w.print_residence_permit(),
        // Unknown document types use the regular layout.
        _ => w.print_regular_id(),
    }

    w.pdf.finish(Info {
        title: format!("{} {}", doc.given_name, doc.surname),
        subject: "Lična karta".into(),
    })
}
