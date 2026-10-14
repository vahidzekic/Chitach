//! Document export (PDF / JSON / XLSX) and auto-save after reading.
//!
//! Auto-save is a port of bas-celik `internal/gui/autoSave.go` (MIT, (c) Nikola Ubavić).

use std::path::{Path, PathBuf};

use celik_core::Document;
use celik_export::PdfOptions;
use serde::Deserialize;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::{
    error::{catch, with_detail, CmdResult},
    events::{notify, notify_error, Level},
    settings::{AutoSave, Settings},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Pdf,
    Json,
    Xlsx,
}

impl Format {
    pub fn ext(self) -> &'static str {
        match self {
            Format::Pdf => "pdf",
            Format::Json => "json",
            Format::Xlsx => "xlsx",
        }
    }
}

/// Render a document in the given format.
pub fn render(doc: &Document, format: Format, cyrillic_labels: bool) -> CmdResult<Vec<u8>> {
    catch("error.export", || {
        let res = match format {
            Format::Pdf => celik_export::to_pdf(doc, PdfOptions { cyrillic_labels }),
            Format::Json => celik_export::to_json(doc),
            Format::Xlsx => celik_export::to_xlsx(doc),
        };
        res.map_err(|e| with_detail("error.export", e))
    })
}

/// Render and write to `path`, appending the extension if missing.
pub fn export_to(
    doc: &Document,
    format: Format,
    path: &Path,
    cyrillic: bool,
) -> CmdResult<PathBuf> {
    let mut path = path.to_path_buf();
    let has_ext = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(format.ext()));
    if !has_ext {
        let mut s = path.into_os_string();
        s.push(".");
        s.push(format.ext());
        path = PathBuf::from(s);
    }
    let bytes = render(doc, format, cyrillic)?;
    std::fs::write(&path, bytes).map_err(|e| with_detail("error.saveFile", e))?;
    Ok(path)
}

pub fn suggest_file_name(doc: &Document, format: Format) -> String {
    let name = celik_export::file_name(doc, format.ext());
    // Keep file names portable.
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_whitespace() => '_',
            c => c,
        })
        .collect()
}

/// File name used by auto-save: `YYYY-MM-DD_HH-MM_<name>.pdf`.
pub fn auto_save_name(doc: &Document, now: chrono::DateTime<chrono::Local>) -> String {
    format!(
        "{}_{}",
        now.format("%Y-%m-%d_%H-%M"),
        suggest_file_name(doc, Format::Pdf)
    )
}

/// Save the PDF to the auto-save directory, optionally open it, and emit `notify`.
pub fn auto_save(app: &AppHandle, doc: &Document, settings: &Settings) {
    if settings.auto_save == AutoSave::Off || settings.auto_save_dir.trim().is_empty() {
        return;
    }
    let dir = PathBuf::from(settings.auto_save_dir.trim());
    match std::fs::metadata(&dir) {
        Err(e) => return notify_error(app, &with_detail("autoSave.error.general", e)),
        Ok(m) if !m.is_dir() => {
            return notify(
                app,
                Level::Error,
                "autoSave.error.nonDirectory",
                Some(dir.display().to_string()),
            )
        }
        Ok(_) => {}
    }
    let path = dir.join(auto_save_name(doc, chrono::Local::now()));
    let bytes = match render(doc, Format::Pdf, settings.pdf_cyrillic_labels) {
        Ok(b) => b,
        Err(e) => {
            let detail = e.split_once('|').map(|(_, d)| d.to_string()).unwrap_or(e);
            return notify(app, Level::Error, "autoSave.error.general", Some(detail));
        }
    };
    if let Err(e) = std::fs::write(&path, bytes) {
        return notify(
            app,
            Level::Error,
            "autoSave.error.general",
            Some(e.to_string()),
        );
    }
    if settings.auto_save == AutoSave::SaveAndOpen {
        if let Err(e) = app.opener().open_path(path.to_string_lossy(), None::<&str>) {
            return notify(
                app,
                Level::Error,
                "autoSave.error.fileOpen",
                Some(e.to_string()),
            );
        }
    }
    notify(
        app,
        Level::Success,
        "autoSave.done",
        Some(path.display().to_string()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn format_deserializes() {
        let f: Format = serde_json::from_str("\"xlsx\"").unwrap();
        assert_eq!(f, Format::Xlsx);
        assert_eq!(f.ext(), "xlsx");
    }

    #[test]
    fn auto_save_file_name() {
        let doc = Document::Id(Box::new(celik_core::IdDocument {
            given_name: "Petar".into(),
            surname: "Petrović".into(),
            ..Default::default()
        }));
        let now = chrono::Local.with_ymd_and_hms(2026, 3, 4, 5, 6, 7).unwrap();
        assert_eq!(
            auto_save_name(&doc, now),
            "2026-03-04_05-06_petar_petrović.pdf"
        );
    }
}
