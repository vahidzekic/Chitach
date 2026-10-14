//! Application state shared between the poller thread and Tauri commands.
//!
//! The card/reader state machine ([`Core`]) is pure (no Tauri, no PC/SC) so it can
//! be unit-tested. Behaviour follows bas-celik `internal/gui/poller.go`
//! (Port of bas-celik (MIT, (c) Nikola Ubavić)).

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

use celik_core::Document;
use serde::Serialize;

use crate::settings::{Mode, Settings};

/// Status of the card shown in the UI (`CardStatus` in the API contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CardStatus {
    NoReader,
    Waiting,
    Reading,
    Ready,
    UnknownCard,
    Error,
}

/// One connected reader (`ReaderStatus` in the API contract).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderInfo {
    pub name: String,
    pub card_present: bool,
    pub atr: Option<String>,
}

impl From<celik_core::reader::ReaderStatus> for ReaderInfo {
    fn from(r: celik_core::reader::ReaderStatus) -> Self {
        ReaderInfo {
            name: r.name,
            card_present: r.card_present,
            atr: r.atr,
        }
    }
}

/// `AppState` of the API contract, emitted as the `app-state` event.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub status: CardStatus,
    pub readers: Vec<ReaderInfo>,
    pub selected_reader: Option<String>,
    pub document: Option<Document>,
    pub atr: Option<String>,
    pub error: Option<String>,
    pub read_id: u64,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            status: CardStatus::NoReader,
            readers: Vec::new(),
            selected_reader: None,
            document: None,
            atr: None,
            error: None,
            read_id: 0,
        }
    }
}

/// Identifies a physical card in a reader (reader name + ATR).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardKey {
    pub reader: String,
    pub atr: Option<String>,
}

/// Result of reading a card.
#[derive(Debug, Clone)]
pub enum ReadOutcome {
    Document(Document),
    /// Card is not a supported document; contains the ATR (hex).
    Unknown(String),
    /// Error string (translation key convention, see `error.rs`).
    Error(String),
}

/// Card/reader state machine.
#[derive(Debug, Default)]
pub struct Core {
    pub app: AppState,
    /// Reader chosen by the user; `None` = automatic.
    pub preferred_reader: Option<String>,
    /// Card currently represented by `app` (read or being read).
    loaded: Option<CardKey>,
    reload_requested: bool,
}

/// Pick a reader: the preferred one if it exists, otherwise the first with a card,
/// otherwise the first reader.
pub fn pick_reader(readers: &[ReaderInfo], preferred: Option<&str>) -> Option<String> {
    if let Some(p) = preferred {
        if readers.iter().any(|r| r.name == p) {
            return Some(p.to_string());
        }
    }
    readers
        .iter()
        .find(|r| r.card_present)
        .or_else(|| readers.first())
        .map(|r| r.name.clone())
}

impl Core {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask for the current card to be read again on the next update.
    pub fn request_reload(&mut self) {
        self.reload_requested = true;
    }

    pub fn reload_pending(&self) -> bool {
        self.reload_requested
    }

    pub fn select_reader(&mut self, name: Option<String>) {
        self.preferred_reader = name;
    }

    /// Apply a fresh reader list. Returns the card that has to be read, if any.
    /// The caller must then call [`Core::begin_read`] and later [`Core::finish_read`].
    pub fn update_readers(&mut self, readers: Vec<ReaderInfo>) -> Option<CardKey> {
        let selected = pick_reader(&readers, self.preferred_reader.as_deref());
        let current = selected
            .as_ref()
            .and_then(|s| readers.iter().find(|r| &r.name == s))
            .cloned();
        self.app.readers = readers;
        self.app.selected_reader = selected;

        let Some(reader) = current else {
            self.clear(CardStatus::NoReader);
            self.reload_requested = false;
            return None;
        };
        if !reader.card_present {
            self.clear(CardStatus::Waiting);
            self.reload_requested = false;
            return None;
        }
        let key = CardKey {
            reader: reader.name,
            atr: reader.atr,
        };
        if self.reload_requested || self.loaded.as_ref() != Some(&key) {
            self.reload_requested = false;
            self.loaded = Some(key.clone());
            return Some(key);
        }
        None
    }

    fn clear(&mut self, status: CardStatus) {
        self.app.status = status;
        self.app.document = None;
        self.app.atr = None;
        self.app.error = None;
        self.loaded = None;
    }

    /// Mark the start of reading `key`.
    pub fn begin_read(&mut self, key: &CardKey) {
        self.loaded = Some(key.clone());
        self.app.status = CardStatus::Reading;
        self.app.document = None;
        self.app.error = None;
        self.app.atr = key.atr.clone();
    }

    /// Apply a read result. Ignored (returns false) if the card changed meanwhile.
    pub fn finish_read(&mut self, key: &CardKey, outcome: ReadOutcome) -> bool {
        if self.loaded.as_ref() != Some(key) || self.app.status != CardStatus::Reading {
            return false;
        }
        match outcome {
            ReadOutcome::Document(doc) => {
                self.app.status = CardStatus::Ready;
                self.app.document = Some(doc);
                self.app.read_id += 1;
            }
            ReadOutcome::Unknown(atr) => {
                self.app.status = CardStatus::UnknownCard;
                self.app.atr = Some(atr);
            }
            ReadOutcome::Error(e) => {
                self.app.status = CardStatus::Error;
                self.app.error = Some(e);
            }
        }
        true
    }

    /// Replace the current document (e.g. after an RFZO update) without a new read.
    pub fn replace_document(&mut self, doc: Document) {
        if self.app.status == CardStatus::Ready {
            self.app.document = Some(doc);
        }
    }
}

/// State managed by Tauri (`app.manage(Shared)`).
pub struct Shared {
    pub core: Mutex<Core>,
    pub settings: RwLock<Settings>,
    /// Mode the app was started in (mode changes need a restart).
    pub mode: Mode,
    /// `CHITACH_MOCK=1`: no PC/SC, sample documents.
    pub mock: bool,
    /// Where settings are persisted (`None` if the config dir is unknown).
    pub settings_path: Option<PathBuf>,
}

impl Shared {
    pub fn new(settings: Settings, mock: bool, settings_path: Option<PathBuf>) -> Self {
        Shared {
            core: Mutex::new(Core::new()),
            mode: settings.mode,
            settings: RwLock::new(settings),
            mock,
            settings_path,
        }
    }

    pub fn core(&self) -> MutexGuard<'_, Core> {
        self.core.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn settings(&self) -> RwLockReadGuard<'_, Settings> {
        self.settings.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn settings_mut(&self) -> RwLockWriteGuard<'_, Settings> {
        self.settings.write().unwrap_or_else(|e| e.into_inner())
    }

    pub fn snapshot(&self) -> AppState {
        self.core().app.clone()
    }

    pub fn current_document(&self) -> Option<Document> {
        self.core().app.document.clone()
    }
}

/// Lock helper ignoring poisoning.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use celik_core::MedicalDocument;

    fn reader(name: &str, card: bool, atr: Option<&str>) -> ReaderInfo {
        ReaderInfo {
            name: name.into(),
            card_present: card,
            atr: atr.map(Into::into),
        }
    }

    fn doc() -> Document {
        Document::Medical(Box::new(MedicalDocument::default()))
    }

    #[test]
    fn no_readers_is_no_reader() {
        let mut c = Core::new();
        assert_eq!(c.update_readers(vec![]), None);
        assert_eq!(c.app.status, CardStatus::NoReader);
        assert_eq!(c.app.selected_reader, None);
    }

    #[test]
    fn reader_without_card_is_waiting() {
        let mut c = Core::new();
        assert_eq!(c.update_readers(vec![reader("A", false, None)]), None);
        assert_eq!(c.app.status, CardStatus::Waiting);
        assert_eq!(c.app.selected_reader.as_deref(), Some("A"));
    }

    #[test]
    fn insert_read_ready_remove() {
        let mut c = Core::new();
        let readers = vec![reader("A", false, None), reader("B", true, Some("3B"))];
        let key = c.update_readers(readers.clone()).expect("read requested");
        assert_eq!(key.reader, "B");
        assert_eq!(c.app.selected_reader.as_deref(), Some("B"));
        c.begin_read(&key);
        assert_eq!(c.app.status, CardStatus::Reading);
        assert_eq!(c.app.atr.as_deref(), Some("3B"));
        assert!(c.finish_read(&key, ReadOutcome::Document(doc())));
        assert_eq!(c.app.status, CardStatus::Ready);
        assert_eq!(c.app.read_id, 1);
        assert!(c.app.document.is_some());

        // Same card again: no new read.
        assert_eq!(c.update_readers(readers), None);
        assert_eq!(c.app.status, CardStatus::Ready);

        // Removal -> automatic selection falls back to first reader, waiting.
        assert_eq!(
            c.update_readers(vec![reader("A", false, None), reader("B", false, None)]),
            None
        );
        assert_eq!(c.app.status, CardStatus::Waiting);
        assert!(c.app.document.is_none());
    }

    #[test]
    fn reload_rereads_same_card() {
        let mut c = Core::new();
        let readers = vec![reader("A", true, Some("3B"))];
        let key = c.update_readers(readers.clone()).unwrap();
        c.begin_read(&key);
        c.finish_read(&key, ReadOutcome::Document(doc()));
        c.request_reload();
        assert!(c.reload_pending());
        assert_eq!(c.update_readers(readers.clone()), Some(key.clone()));
        c.begin_read(&key);
        c.finish_read(&key, ReadOutcome::Document(doc()));
        assert_eq!(c.app.read_id, 2);
        assert_eq!(c.update_readers(readers), None);
    }

    #[test]
    fn unknown_and_error_outcomes() {
        let mut c = Core::new();
        let key = c
            .update_readers(vec![reader("A", true, Some("3B00"))])
            .unwrap();
        c.begin_read(&key);
        c.finish_read(&key, ReadOutcome::Unknown("3B00".into()));
        assert_eq!(c.app.status, CardStatus::UnknownCard);
        assert_eq!(c.app.atr.as_deref(), Some("3B00"));
        assert_eq!(c.app.read_id, 0);

        let key = c
            .update_readers(vec![reader("A", true, Some("3B01"))])
            .unwrap();
        c.begin_read(&key);
        c.finish_read(&key, ReadOutcome::Error("error.readingCard|x".into()));
        assert_eq!(c.app.status, CardStatus::Error);
        assert_eq!(c.app.error.as_deref(), Some("error.readingCard|x"));
        // Error state is kept until the card changes.
        assert_eq!(
            c.update_readers(vec![reader("A", true, Some("3B01"))]),
            None
        );
        assert_eq!(c.app.status, CardStatus::Error);
    }

    #[test]
    fn stale_read_result_is_ignored() {
        let mut c = Core::new();
        let key = c
            .update_readers(vec![reader("A", true, Some("3B"))])
            .unwrap();
        c.begin_read(&key);
        // Card removed while reading.
        c.update_readers(vec![reader("A", false, None)]);
        assert!(!c.finish_read(&key, ReadOutcome::Document(doc())));
        assert_eq!(c.app.status, CardStatus::Waiting);
        assert!(c.app.document.is_none());
    }

    #[test]
    fn manual_reader_selection() {
        let mut c = Core::new();
        let readers = vec![reader("A", true, Some("1")), reader("B", true, Some("2"))];
        let key = c.update_readers(readers.clone()).unwrap();
        assert_eq!(key.reader, "A");
        c.begin_read(&key);
        c.finish_read(&key, ReadOutcome::Document(doc()));
        c.select_reader(Some("B".into()));
        let key = c.update_readers(readers.clone()).unwrap();
        assert_eq!(key.reader, "B");
        // Unknown preferred reader falls back to auto selection.
        c.select_reader(Some("Z".into()));
        assert_eq!(pick_reader(&readers, Some("Z")).as_deref(), Some("A"));
    }

    #[test]
    fn app_state_serializes_camel_case() {
        let s = AppState::default();
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["status"], "noReader");
        assert!(v.get("selectedReader").is_some());
        assert!(v.get("readId").is_some());
        let r = serde_json::to_value(reader("A", true, None)).unwrap();
        assert_eq!(r["cardPresent"], true);
        let st = serde_json::to_value(CardStatus::UnknownCard).unwrap();
        assert_eq!(st, "unknownCard");
    }
}
