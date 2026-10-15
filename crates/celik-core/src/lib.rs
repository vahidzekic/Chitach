//! Chitach core: smart-card protocol and document model.
//!
//! Port of the `card`, `document` (data part) and `localization` packages of
//! [Bas Celik](https://github.com/ubavic/bas-celik) (MIT, (c) Nikola Ubavic).

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
