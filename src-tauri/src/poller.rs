//! Background thread watching card readers and reading inserted cards.

use std::time::Duration;

use celik_core::{reader::ReaderMonitor, CardError, CardTransport, Document};
use tauri::{AppHandle, Manager};

use crate::{
    error::{card_error, catch},
    events::emit_state,
    export,
    state::{CardKey, ReadOutcome, ReaderInfo, Shared},
};

pub const MOCK_READER: &str = "Mock Reader";
pub const MOCK_ATR: &str =
    "3B FF 94 00 00 81 31 80 43 80 31 80 65 B0 85 02 01 F3 12 0F FF 82 90 00 79";

const POLL: Duration = Duration::from_millis(500);

/// Start the poller thread.
pub fn spawn(app: AppHandle) {
    let mock = app.state::<Shared>().mock;
    let res = std::thread::Builder::new()
        .name("chitach-poller".into())
        .spawn(move || if mock { run_mock(app) } else { run_pcsc(app) });
    if let Err(e) = res {
        log::error!("cannot start poller thread: {e}");
    }
}

/// Apply a reader list, emit the change and read the card if needed.
fn handle(
    app: &AppHandle,
    readers: Vec<ReaderInfo>,
    read: &mut dyn FnMut(&CardKey) -> ReadOutcome,
) {
    let shared = app.state::<Shared>();
    let key = {
        let mut core = shared.core();
        let before = core.app.clone();
        let key = core.update_readers(readers);
        if let Some(k) = &key {
            core.begin_read(k);
        }
        let changed = core.app != before;
        drop(core);
        if changed {
            emit_state(app);
        }
        key
    };
    let Some(key) = key else { return };

    let outcome = read(&key);
    let doc = match &outcome {
        ReadOutcome::Document(d) => Some(d.clone()),
        _ => None,
    };
    let applied = shared.core().finish_read(&key, outcome);
    if !applied {
        return;
    }
    emit_state(app);
    if let Some(doc) = doc {
        after_read(app, doc);
    }
}

/// Auto-save and bring the window back from the tray.
fn after_read(app: &AppHandle, doc: Document) {
    let settings = app.state::<Shared>().settings().clone();
    if settings.run_in_background && settings.auto_save != crate::settings::AutoSave::SaveAndOpen {
        crate::tray::show_main_window(app);
    }
    let app = app.clone();
    std::thread::spawn(move || export::auto_save(&app, &doc, &settings));
}

fn run_pcsc(app: AppHandle) {
    let mut monitor: Option<ReaderMonitor> = None;
    loop {
        let m = match monitor.as_mut() {
            Some(m) => m,
            None => match catch("error.pcsc", || {
                ReaderMonitor::new().map_err(|e| card_error("error.pcsc", &e))
            }) {
                Ok(m) => monitor.insert(m),
                Err(e) => {
                    log::debug!("PC/SC unavailable: {e}");
                    handle(&app, Vec::new(), &mut read_pcsc);
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
            },
        };
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m.wait_change(POLL)));
        match res {
            Ok(Ok(list)) => {
                let readers = list.into_iter().map(ReaderInfo::from).collect();
                handle(&app, readers, &mut read_pcsc);
            }
            Ok(Err(CardError::NoReader)) => {
                handle(&app, Vec::new(), &mut read_pcsc);
                std::thread::sleep(POLL);
            }
            Ok(Err(e)) => {
                log::warn!("reader monitor: {e}");
                monitor = None;
                handle(&app, Vec::new(), &mut read_pcsc);
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(_) => {
                monitor = None;
                handle(&app, Vec::new(), &mut read_pcsc);
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    }
}

/// Map the result of `read_document` to a [`ReadOutcome`].
pub fn outcome_from(result: Result<Document, String>, atr: Option<&str>) -> ReadOutcome {
    match result {
        Ok(doc) => ReadOutcome::Document(doc),
        Err(e) => match e.strip_prefix("error.unknownCard") {
            Some(rest) => {
                let from_err = rest.strip_prefix('|').unwrap_or("").to_string();
                let atr = if from_err.is_empty() {
                    atr.unwrap_or_default().to_string()
                } else {
                    from_err
                };
                ReadOutcome::Unknown(atr)
            }
            None => ReadOutcome::Error(e),
        },
    }
}

fn read_pcsc(key: &CardKey) -> ReadOutcome {
    let result = catch("error.readingCard", || {
        let card = celik_core::reader::PcscCard::connect(&key.reader)
            .map_err(|e| card_error("error.readingCard", &e))?;
        read_with_transaction(&card).map_err(|e| card_error("error.readingCard", &e))
    });
    if let Err(e) = &result {
        log::warn!("reading card in {}: {e}", key.reader);
    }
    outcome_from(result, key.atr.as_deref())
}

fn read_with_transaction(card: &dyn CardTransport) -> celik_core::Result<Document> {
    card.begin_transaction()?;
    let res = celik_core::read_document(card);
    let _ = card.end_transaction();
    res
}

/// Sample document number `n` (cycles id -> medical -> vehicle).
pub fn mock_document(n: u64) -> Document {
    match n % 3 {
        0 => Document::Id(Box::new(celik_core::sample::id())),
        1 => Document::Medical(Box::new(celik_core::sample::medical())),
        _ => Document::Vehicle(Box::new(celik_core::sample::vehicle())),
    }
}

fn run_mock(app: AppHandle) {
    let mut counter: u64 = 0;
    loop {
        let readers = vec![ReaderInfo {
            name: MOCK_READER.into(),
            card_present: true,
            atr: Some(MOCK_ATR.into()),
        }];
        handle(&app, readers, &mut |_key| {
            std::thread::sleep(Duration::from_millis(400));
            let n = counter;
            counter += 1;
            outcome_from(
                catch("error.readingCard", || Ok(mock_document(n))),
                Some(MOCK_ATR),
            )
        });
        std::thread::sleep(Duration::from_millis(300));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_card_outcome_uses_atr() {
        match outcome_from(Err("error.unknownCard|3B01".into()), Some("XX")) {
            ReadOutcome::Unknown(a) => assert_eq!(a, "3B01"),
            o => panic!("{o:?}"),
        }
        match outcome_from(Err("error.unknownCard".into()), Some("XX")) {
            ReadOutcome::Unknown(a) => assert_eq!(a, "XX"),
            o => panic!("{o:?}"),
        }
        match outcome_from(Err("error.readingCard|x".into()), None) {
            ReadOutcome::Error(e) => assert_eq!(e, "error.readingCard|x"),
            o => panic!("{o:?}"),
        }
    }
}
