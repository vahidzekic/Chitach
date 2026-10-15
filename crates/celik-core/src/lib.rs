//! Chitach core: smart-card protocol and document model.

pub mod apdu;
pub mod atr;
pub mod ber;
pub mod card;
pub mod document;
pub mod error;
pub mod localization;
pub mod pin;
#[cfg(feature = "pcsc")]
pub mod reader;
#[cfg(feature = "rfzo")]
pub mod rfzo;
pub mod sample;
pub mod tlv;
pub mod virtual_card;

pub use card::{detect_card, read_document, CardKind, CardTransport, Certificate, IdCardCrypto};
pub use document::{Document, IdDocument, MedicalDocument, VehicleDocument};
pub use error::{CardError, Result};
