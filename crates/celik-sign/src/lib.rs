//! PKCS#11 module handling, XML-DSig signing and Smartbox server.
//! Port of bas-celik `internal/pkcs11`, `internal/xmlsign`, `internal/smartbox`.

pub mod pkcs11;
pub mod smartbox;
pub mod xmlsign;

#[derive(Debug, thiserror::Error)]
pub enum SignError {
    #[error("pkcs11: {0}")]
    Pkcs11(String),
    #[error("no module loaded")]
    NoModules,
    #[error("wrong PIN")]
    WrongPin,
    #[error("xml: {0}")]
    Xml(String),
    #[error("server: {0}")]
    Server(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, SignError>;
