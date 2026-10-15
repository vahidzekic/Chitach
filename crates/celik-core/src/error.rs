use thiserror::Error;

#[derive(Debug, Error)]
pub enum CardError {
    #[error("PC/SC error: {0}")]
    Pcsc(String),
    #[error("no smart card reader found")]
    NoReader,
    #[error("no card in reader")]
    NoCard,
    #[error("unknown card (ATR: {0})")]
    UnknownCard(String),
    #[error("card returned bad status: {0:02X}{1:02X}")]
    BadStatus(u8, u8),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("invalid PIN")]
    InvalidPin,
    #[error("wrong PIN, {0} tries left")]
    WrongPin(u8),
    #[error("PIN blocked")]
    PinBlocked,
    #[error("operation not supported by this card")]
    Unsupported,
    #[error("network error: {0}")]
    Network(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CardError>;

/// Low-level format errors of the TLV / BER parsers.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum FormatError {
    #[error("invalid length")]
    InvalidLength,
    #[error("invalid format")]
    InvalidFormat,
}

impl From<FormatError> for CardError {
    fn from(e: FormatError) -> Self {
        CardError::Parse(e.to_string())
    }
}
