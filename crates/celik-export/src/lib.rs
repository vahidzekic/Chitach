//! Export of documents to PDF (layout identical to Bas Celik / official apps), JSON and XLSX.

use celik_core::Document;

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

pub fn to_pdf(_doc: &Document, _opts: PdfOptions) -> Result<Vec<u8>> { todo!() }
pub fn to_json(_doc: &Document) -> Result<Vec<u8>> { todo!() }
pub fn to_xlsx(_doc: &Document) -> Result<Vec<u8>> { todo!() }

/// Suggested file name, e.g. `petar_petrovic.pdf`.
pub fn file_name(doc: &Document, ext: &str) -> String { format!("{}.{}", doc.file_stem(), ext) }
