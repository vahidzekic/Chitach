//! XLSX export.

use std::fmt;

use celik_core::localization::{format_yes_no, Language};
use celik_core::Document;
use rust_xlsxwriter::Workbook;
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde_json::Value;

use crate::{ExportError, Result};

/// Field name / value pairs in declaration order.
struct OrderedFields(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for OrderedFields {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = OrderedFields;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<OrderedFields, A::Error> {
                let mut v = Vec::new();
                while let Some((k, val)) = map.next_entry::<String, Value>()? {
                    v.push((k, val));
                }
                Ok(OrderedFields(v))
            }
        }
        d.deserialize_map(V)
    }
}

/// Rows written to the sheet: label = document field name, value as string.
/// Booleans are formatted as English Yes/No; the portrait is omitted.
pub(crate) fn rows(doc: &Document) -> Result<Vec<(String, String)>> {
    let json = match doc {
        Document::Id(d) => serde_json::to_string(d),
        Document::Medical(d) => serde_json::to_string(d),
        Document::Vehicle(d) => serde_json::to_string(d),
    }
    .map_err(|e| ExportError::Xlsx(e.to_string()))?;
    let fields: OrderedFields =
        serde_json::from_str(&json).map_err(|e| ExportError::Xlsx(e.to_string()))?;
    Ok(fields
        .0
        .into_iter()
        .filter(|(k, _)| k != "Portrait")
        .filter_map(|(k, v)| match v {
            Value::String(s) => Some((k, s)),
            Value::Bool(b) => Some((k, format_yes_no(b, Language::En).to_string())),
            _ => None,
        })
        .collect())
}

pub fn build(doc: &Document) -> Result<Vec<u8>> {
    let err = |e: rust_xlsxwriter::XlsxError| ExportError::Xlsx(e.to_string());
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("Sheet1").map_err(err)?;
    ws.set_column_width(0, 30).map_err(err)?;
    ws.set_column_width(1, 60).map_err(err)?;
    for (row, (label, value)) in rows(doc)?.into_iter().enumerate() {
        ws.write_string(row as u32, 0, &label).map_err(err)?;
        ws.write_string(row as u32, 1, &value).map_err(err)?;
    }
    wb.save_to_buffer().map_err(err)
}
