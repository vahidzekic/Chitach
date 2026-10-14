//! RFZO "valid until" web service (port of medical.go UpdateValidUntilDateFromRfzo).
use crate::{document::MedicalDocument, Result};

/// Fetch insurance validity from rfzo.rs and update `doc.valid_until`.
pub fn update_valid_until(_doc: &mut MedicalDocument) -> Result<()> { todo!() }
