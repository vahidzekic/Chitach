//! RFZO "valid until" web service.
//!
//! Port of bas-celik `document/medical.go` (`UpdateValidUntilDateFromRfzo`,
//! `ParseValidUntilDateFromRfzoResponse`, `ParseApiKeyFromRfzoPage`)
//! (MIT, (c) Nikola Ubavić).

use crate::{document::MedicalDocument, CardError, Result};
use std::time::Duration;

pub const RFZO_SERVICE_URL: &str = "https://rfzo.rs/api_overa.php";
pub const RFZO_PAGE_URL: &str = "https://rfzo.rs/proveraUplateDoprinosa2.php";

const TIMEOUT: Duration = Duration::from_secs(20);

/// Errors specific to the RFZO service. They are converted into
/// [`CardError::Other`] (validation / data errors) or [`CardError::Network`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RfzoError {
    /// Card number doesn't have exactly 11 digits.
    #[error("invalid card number length")]
    InvalidCardNo,
    /// Insurance number doesn't have exactly 11 digits.
    #[error("invalid insurance number length")]
    InvalidInsuranceNo,
    /// RFZO returned no record for the given card and insurance number.
    #[error("no data found")]
    NoDataFound,
    /// The RFZO page no longer carries the key its script sends to the API.
    #[error("no API key found")]
    NoApiKeyFound,
    #[error("decoding json: {0}")]
    Json(String),
    #[error("{0}")]
    Network(String),
}

impl From<RfzoError> for CardError {
    fn from(e: RfzoError) -> Self {
        match e {
            RfzoError::Network(m) => CardError::Network(m),
            other => CardError::Other(other.to_string()),
        }
    }
}

/// Checks the card and insurant numbers required by the RFZO service.
pub fn validate(doc: &MedicalDocument) -> std::result::Result<(), RfzoError> {
    if doc.card_id.chars().count() != 11 {
        return Err(RfzoError::InvalidCardNo);
    }
    if doc.insurant_number.chars().count() != 11 {
        return Err(RfzoError::InvalidInsuranceNo);
    }
    Ok(())
}

/// Fetch insurance validity from rfzo.rs and update `doc.valid_until`.
pub fn update_valid_until(doc: &mut MedicalDocument) -> Result<()> {
    doc.valid_until = fetch_valid_until(doc)?;
    Ok(())
}

/// Fetches the "valid until" date from rfzo.rs (formatted like on the card: `DD.MM.YYYY.`).
pub fn fetch_valid_until(doc: &MedicalDocument) -> std::result::Result<String, RfzoError> {
    validate(doc)?;

    let agent = agent();
    let api_key = fetch_api_key(&agent)?;

    let body = agent
        .get(RFZO_SERVICE_URL)
        .query("bzk", &doc.card_id)
        .query("lbo", &doc.insurant_number)
        .header("X-API-Secret", &api_key)
        .call()
        .map_err(|e| RfzoError::Network(format!("requesting: {e}")))?
        .body_mut()
        .read_to_string()
        .map_err(|e| RfzoError::Network(format!("reading response body: {e}")))?;

    parse_valid_until_date_from_rfzo_response(&body)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .into()
}

/// The key is not documented anywhere; the check page passes it to the API
/// from its inline JavaScript, so take it from there.
fn fetch_api_key(agent: &ureq::Agent) -> std::result::Result<String, RfzoError> {
    let page = agent
        .get(RFZO_PAGE_URL)
        .call()
        .map_err(|e| RfzoError::Network(format!("requesting page: {e}")))?
        .body_mut()
        .read_to_string()
        .map_err(|e| RfzoError::Network(format!("reading page: {e}")))?;
    parse_api_key_from_rfzo_page(&page)
}

/// Extracts the date from the RFZO JSON response. The card itself stores
/// the date with a trailing dot, the API omits it, so one is appended.
pub fn parse_valid_until_date_from_rfzo_response(
    response: &str,
) -> std::result::Result<String, RfzoError> {
    #[derive(serde::Deserialize)]
    struct Record {
        #[serde(rename = "overena_do_poslednja", default)]
        valid_until: Option<String>,
    }

    let records: Vec<Record> =
        serde_json::from_str(response).map_err(|e| RfzoError::Json(e.to_string()))?;

    match records.into_iter().next().and_then(|r| r.valid_until) {
        Some(d) if !d.is_empty() => Ok(format!("{}.", d.strip_suffix('.').unwrap_or(&d))),
        _ => Err(RfzoError::NoDataFound),
    }
}

/// Finds the value of `'X-API-Secret': '...'` in the page (equivalent to the
/// regex `['"]X-API-Secret['"]\s*:\s*['"]([^'"]+)['"]`).
pub fn parse_api_key_from_rfzo_page(page: &str) -> std::result::Result<String, RfzoError> {
    const NAME: &str = "X-API-Secret";
    let is_quote = |c: char| c == '\'' || c == '"';

    for (pos, _) in page.match_indices(NAME) {
        if !page[..pos].ends_with(is_quote) {
            continue;
        }
        let rest = &page[pos + NAME.len()..];
        let Some(rest) = rest.strip_prefix(is_quote) else {
            continue;
        };
        let Some(rest) = rest
            .trim_start_matches(char::is_whitespace)
            .strip_prefix(':')
        else {
            continue;
        };
        let Some(rest) = rest
            .trim_start_matches(char::is_whitespace)
            .strip_prefix(is_quote)
        else {
            continue;
        };
        let Some(end) = rest.find(is_quote) else {
            continue;
        };
        if end > 0 {
            return Ok(rest[..end].to_string());
        }
    }

    Err(RfzoError::NoApiKeyFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_before_network() {
        let mut empty = MedicalDocument::default();
        assert_eq!(validate(&empty), Err(RfzoError::InvalidCardNo));
        assert!(
            matches!(update_valid_until(&mut empty), Err(CardError::Other(m)) if m == "invalid card number length")
        );

        let doc = MedicalDocument {
            card_id: "12345678901".into(),
            insurant_number: "12345678".into(),
            ..Default::default()
        };
        assert_eq!(validate(&doc), Err(RfzoError::InvalidInsuranceNo));
        assert_eq!(fetch_valid_until(&doc), Err(RfzoError::InvalidInsuranceNo));
    }

    #[test]
    fn parse_response() {
        assert_eq!(
            parse_valid_until_date_from_rfzo_response("[]"),
            Err(RfzoError::NoDataFound)
        );
        assert!(matches!(
            parse_valid_until_date_from_rfzo_response(""),
            Err(RfzoError::Json(_))
        ));
        assert_eq!(
            parse_valid_until_date_from_rfzo_response(
                r#"[{"overena_do_poslednja":"3.4.2025","ustanova_naziv":"Ispostava Stari Grad"}]"#
            ),
            Ok("3.4.2025.".into())
        );
        assert_eq!(
            parse_valid_until_date_from_rfzo_response(
                r#"[{"overena_do_poslednja":"31.12.2023."}]"#
            ),
            Ok("31.12.2023.".into())
        );
        assert_eq!(
            parse_valid_until_date_from_rfzo_response(r#"[{"overena_do_poslednja":""}]"#),
            Err(RfzoError::NoDataFound)
        );
        assert_eq!(
            parse_valid_until_date_from_rfzo_response(r#"[{"other":1}]"#),
            Err(RfzoError::NoDataFound)
        );
    }

    #[test]
    fn parse_api_key() {
        assert_eq!(
            parse_api_key_from_rfzo_page("<html></html>"),
            Err(RfzoError::NoApiKeyFound)
        );
        let page = r#"const response = await fetch(apiUrl, {
    headers: {
        'X-API-Secret': 'ab12cd34-0000-1111-2222-333344445555'
    }
});"#;
        assert_eq!(
            parse_api_key_from_rfzo_page(page),
            Ok("ab12cd34-0000-1111-2222-333344445555".into())
        );
        assert_eq!(
            parse_api_key_from_rfzo_page(r#"{"X-API-Secret" :"k1"}"#),
            Ok("k1".into())
        );
        assert_eq!(
            parse_api_key_from_rfzo_page(
                r#"X-API-Secret: 'no' then "X-API-Secret":'' and 'X-API-Secret':"yes""#
            ),
            Ok("yes".into())
        );
    }
}
