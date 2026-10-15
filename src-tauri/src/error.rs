//! Error strings for the UI.
//!
//! Convention: `error.<key>` is a translation key; optional parameters follow
//! `:` (e.g. `error.wrongPin:2`) and an optional untranslated technical detail
//! follows `|` (e.g. `error.readingCard|PC/SC error: ...`).

use celik_core::CardError;
use celik_sign::SignError;

pub type CmdResult<T> = Result<T, String>;

/// `key|detail`.
pub fn with_detail(key: &str, detail: impl std::fmt::Display) -> String {
    format!("{key}|{detail}")
}

/// Map a card error to a UI error string; `context` is the key used for generic errors.
pub fn card_error(context: &str, e: &CardError) -> String {
    match e {
        CardError::NoReader => "error.noReader".into(),
        CardError::NoCard => "error.noCard".into(),
        CardError::UnknownCard(atr) => with_detail("error.unknownCard", atr),
        CardError::WrongPin(n) => format!("error.wrongPin:{n}"),
        CardError::PinBlocked => "error.pinBlocked".into(),
        CardError::InvalidPin => "error.invalidPin".into(),
        CardError::Unsupported => "error.unsupported".into(),
        CardError::Network(d) => with_detail("error.network", d),
        other => with_detail(context, other),
    }
}

pub fn sign_error(context: &str, e: &SignError) -> String {
    match e {
        SignError::WrongPin => "error.wrongPin".into(),
        SignError::NoModules => "error.noModules".into(),
        other => with_detail(context, other),
    }
}

/// Text of a caught panic payload.
pub fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic".into()
    }
}

/// Run `f`, converting a panic (e.g. an unimplemented code path) into an error string.
pub fn catch<T>(context: &str, f: impl FnOnce() -> CmdResult<T>) -> CmdResult<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(p) => Err(with_detail(context, panic_message(p.as_ref()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping() {
        assert_eq!(
            card_error("error.readingCard", &CardError::WrongPin(2)),
            "error.wrongPin:2"
        );
        assert_eq!(
            card_error("error.readingCard", &CardError::Parse("x".into())),
            "error.readingCard|parse error: x"
        );
        assert_eq!(
            card_error("error.x", &CardError::NoReader),
            "error.noReader"
        );
    }

    #[test]
    fn catches_panics() {
        let r: CmdResult<()> = catch("error.ctx", || panic!("boom"));
        assert_eq!(r.unwrap_err(), "error.ctx|boom");
    }
}
