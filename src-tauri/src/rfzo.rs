//! RFZO "valid until" update for the medical insurance card.

use celik_core::Document;

use crate::{
    error::{card_error, catch, CmdResult},
    state::Shared,
};

/// Fetch the insurance validity for the current medical document (blocking).
/// Returns the updated document; the shared state is updated too.
pub fn update(shared: &Shared) -> CmdResult<Document> {
    let doc = shared.current_document();
    let Some(Document::Medical(mut med)) = doc else {
        return Err("error.notMedicalCard".into());
    };
    let before = med.clone();
    if shared.mock {
        let date = chrono::Local::now().date_naive() + chrono::Duration::days(365);
        med.valid_until = date.format("%d.%m.%Y").to_string();
    } else {
        catch("error.rfzo", || {
            celik_core::rfzo::update_valid_until(&mut med).map_err(|e| card_error("error.rfzo", &e))
        })?;
    }
    let updated = Document::Medical(med);
    let mut core = shared.core();
    // Only apply if the same document is still shown.
    if matches!(&core.app.document, Some(Document::Medical(m)) if *m == before) {
        core.replace_document(updated.clone());
    }
    Ok(updated)
}
