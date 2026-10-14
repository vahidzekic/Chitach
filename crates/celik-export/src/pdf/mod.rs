//! PDF rendering. Port of bas-celik (MIT, (c) Nikola Ubavić) `BuildPdf` methods.

mod canvas;
mod id;
mod medical;
mod vehicle;

use celik_core::Document;

use crate::{PdfOptions, Result};

pub fn build(doc: &Document, opts: PdfOptions) -> Result<Vec<u8>> {
    match doc {
        Document::Id(d) => id::build(d, opts.cyrillic_labels, &print_date()),
        Document::Medical(d) => medical::build(d),
        Document::Vehicle(d) => vehicle::build(d),
    }
}

/// Today's date as `DD.MM.YYYY.` (Go format `02.01.2006.`).
fn print_date() -> String {
    chrono::Local::now().format("%d.%m.%Y.").to_string()
}
