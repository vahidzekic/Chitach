//! Export of documents to PDF (layout matches the official application's output), JSON and XLSX.

use celik_core::Document;

mod json;
mod pdf;
mod xlsx;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("pdf: {0}")]
    Pdf(String),
    #[error("xlsx: {0}")]
    Xlsx(String),
    #[error("json: {0}")]
    Json(String),
}

pub type Result<T> = std::result::Result<T, ExportError>;

#[derive(Debug, Clone, Copy, Default)]
pub struct PdfOptions {
    /// Use Cyrillic labels in the ID card PDF (ignored for other documents).
    pub cyrillic_labels: bool,
}

/// Render the document to a single-page A4 PDF (layout of the official forms).
pub fn to_pdf(doc: &Document, opts: PdfOptions) -> Result<Vec<u8>> {
    pdf::build(doc, opts)
}

/// Serialize the document to compact JSON with stable field order.
pub fn to_json(doc: &Document) -> Result<Vec<u8>> {
    json::build(doc)
}

/// Export the document as an XLSX workbook (column A: field name, column B: value).
pub fn to_xlsx(doc: &Document) -> Result<Vec<u8>> {
    xlsx::build(doc)
}

/// Suggested file name, e.g. `petar_petrovic.pdf`.
pub fn file_name(doc: &Document, ext: &str) -> String {
    format!("{}.{}", doc.file_stem(), ext)
}
