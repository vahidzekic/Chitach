//! JSON export.

use celik_core::Document;

use crate::{ExportError, Result};

/// Serialize the document as compact JSON:
/// PascalCase field names in declaration order (Portrait first for ID cards,
/// as base64 JPEG) and HTML-safe escaping of `<`, `>`, `&`, U+2028 and U+2029.
pub fn build(doc: &Document) -> Result<Vec<u8>> {
    let raw = match doc {
        Document::Id(d) => serde_json::to_string(d),
        Document::Medical(d) => serde_json::to_string(d),
        Document::Vehicle(d) => serde_json::to_string(d),
    }
    .map_err(|e| ExportError::Json(e.to_string()))?;
    Ok(html_safe_escape(&raw).into_bytes())
}

/// These characters can only occur inside JSON strings, so a plain replacement is safe.
fn html_safe_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn escape() {
        assert_eq!(
            super::html_safe_escape(r#"{"A":"<a & b>"}"#),
            "{\"A\":\"\\u003ca \\u0026 b\\u003e\"}"
        );
    }
}
