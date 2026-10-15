//! PC/SC reader access.
//!
//! Port of the reader handling in bas-celik `internal/gui/reader` and
//! `card/state.go` (MIT, (c) Nikola Ubavić).

use crate::apdu::to_hex;
use crate::{card::CardTransport, CardError, Result};
use pcsc::{
    Context, Disposition, Error as PcscError, Protocols, ReaderState, Scope, ShareMode, State,
    Transaction,
};
use std::ffi::{CStr, CString};
use std::ptr::NonNull;
use std::sync::Mutex;
use std::time::Duration;

/// Maps PC/SC errors to [`CardError`]: missing service / readers become
/// [`CardError::NoReader`], missing or removed cards [`CardError::NoCard`].
pub fn map_pcsc_error(e: PcscError) -> CardError {
    match e {
        PcscError::NoService
        | PcscError::ServiceStopped
        | PcscError::NoReadersAvailable
        | PcscError::UnknownReader
        | PcscError::ReaderUnavailable => CardError::NoReader,
        PcscError::NoSmartcard
        | PcscError::RemovedCard
        | PcscError::UnpoweredCard
        | PcscError::UnresponsiveCard => CardError::NoCard,
        other => CardError::Pcsc(other.to_string()),
    }
}

fn establish() -> Result<Context> {
    Context::establish(Scope::User).map_err(map_pcsc_error)
}

fn list_readers_in(ctx: &Context) -> std::result::Result<Vec<CString>, PcscError> {
    match ctx.list_readers_owned() {
        Ok(r) => Ok(r),
        Err(PcscError::NoReadersAvailable) => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// List connected reader names. Returns an empty list when there are no
/// readers or the PC/SC service is not running.
pub fn list_readers() -> Result<Vec<String>> {
    let ctx = match Context::establish(Scope::User) {
        Ok(c) => c,
        Err(PcscError::NoService | PcscError::ServiceStopped) => return Ok(Vec::new()),
        Err(e) => return Err(map_pcsc_error(e)),
    };
    match list_readers_in(&ctx) {
        Ok(r) => Ok(r
            .into_iter()
            .map(|n| n.to_string_lossy().into_owned())
            .collect()),
        Err(PcscError::NoService | PcscError::ServiceStopped) => Ok(Vec::new()),
        Err(e) => Err(map_pcsc_error(e)),
    }
}

/// Card handle plus (optionally) an open transaction on it.
struct Inner {
    /// Heap allocated card, owned by this struct (freed in `Drop for PcscCard`).
    card: NonNull<pcsc::Card>,
    /// Open transaction; borrows `*card` for as long as it exists.
    tx: Option<Transaction<'static>>,
    depth: u32,
}

impl Inner {
    fn card(&self) -> &pcsc::Card {
        match &self.tx {
            Some(tx) => tx,
            // SAFETY: `card` is valid until `PcscCard` is dropped and no
            // transaction (the only mutable borrow) is alive in this branch.
            None => unsafe { self.card.as_ref() },
        }
    }
}

/// A card connected through PC/SC (shared mode, T=0 or T=1).
pub struct PcscCard {
    inner: Mutex<Inner>,
    reader: String,
    _ctx: Context,
}

// SAFETY: `pcsc::Card` and `Transaction` are Send + Sync; the raw pointer is
// uniquely owned by this struct and only accessed under the mutex.
unsafe impl Send for PcscCard {}
unsafe impl Sync for PcscCard {}

impl PcscCard {
    pub fn connect(reader: &str) -> Result<Self> {
        let ctx = establish()?;
        let name = CString::new(reader).map_err(|_| CardError::NoReader)?;
        let card = ctx
            .connect(&name, ShareMode::Shared, Protocols::T0 | Protocols::T1)
            .map_err(map_pcsc_error)?;
        let card = NonNull::from(Box::leak(Box::new(card)));
        Ok(Self {
            inner: Mutex::new(Inner {
                card,
                tx: None,
                depth: 0,
            }),
            reader: reader.to_string(),
            _ctx: ctx,
        })
    }

    /// Name of the reader this card is connected through.
    pub fn reader(&self) -> &str {
        &self.reader
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn transmit_raw(card: &pcsc::Card, apdu: &[u8], buf: &mut [u8]) -> Result<Vec<u8>> {
        card.transmit(apdu, buf)
            .map(<[u8]>::to_vec)
            .map_err(map_pcsc_error)
    }
}

impl CardTransport for PcscCard {
    fn atr(&self) -> Result<Vec<u8>> {
        let inner = self.lock();
        let status = inner.card().status2_owned().map_err(map_pcsc_error)?;
        Ok(status.atr().to_vec())
    }

    /// Transmits an APDU. `61xx` (more data available) is followed by GET
    /// RESPONSE and `6Cxx` (wrong Le) by a resend with the correct Le, so T=0
    /// readers behave like T=1 ones.
    fn transmit(&self, apdu: &[u8]) -> Result<Vec<u8>> {
        let inner = self.lock();
        let card = inner.card();
        let mut buf = vec![0u8; pcsc::MAX_BUFFER_SIZE_EXTENDED];

        let mut rsp = Self::transmit_raw(card, apdu, &mut buf)?;

        if let [.., 0x6C, le] = rsp[..] {
            let mut retry = apdu.to_vec();
            if has_le(apdu) {
                *retry.last_mut().expect("APDU with Le is not empty") = le;
            } else {
                retry.push(le);
            }
            rsp = Self::transmit_raw(card, &retry, &mut buf)?;
        }

        let mut data = Vec::new();
        let mut guard = 0;
        while let [.., 0x61, le] = rsp[..] {
            data.extend_from_slice(&rsp[..rsp.len() - 2]);
            guard += 1;
            if guard > 256 {
                return Err(CardError::Pcsc("too many GET RESPONSE rounds".into()));
            }
            rsp = Self::transmit_raw(card, &[apdu[0] & 0x03, 0xC0, 0x00, 0x00, le], &mut buf)?;
        }
        if data.is_empty() {
            Ok(rsp)
        } else {
            data.extend_from_slice(&rsp);
            Ok(data)
        }
    }

    fn begin_transaction(&self) -> Result<()> {
        let mut inner = self.lock();
        if inner.depth == 0 {
            // SAFETY: the card outlives the transaction: `Drop for PcscCard`
            // drops the transaction before freeing the card, and the card is
            // only accessed through the transaction while it exists.
            let card: &'static mut pcsc::Card = unsafe { &mut *inner.card.as_ptr() };
            let tx = card.transaction().map_err(map_pcsc_error)?;
            inner.tx = Some(tx);
        }
        inner.depth += 1;
        Ok(())
    }

    fn end_transaction(&self) -> Result<()> {
        let mut inner = self.lock();
        if inner.depth == 0 {
            return Ok(());
        }
        inner.depth -= 1;
        if inner.depth == 0 {
            if let Some(tx) = inner.tx.take() {
                if let Err((tx, e)) = tx.end(Disposition::LeaveCard) {
                    drop(tx);
                    return Err(map_pcsc_error(e));
                }
            }
        }
        Ok(())
    }
}

impl Drop for PcscCard {
    fn drop(&mut self) {
        let inner = self.inner.get_mut().unwrap_or_else(|p| p.into_inner());
        drop(inner.tx.take());
        // SAFETY: the pointer came from `Box::leak` in `connect` and the
        // transaction borrowing it has just been dropped.
        let card = unsafe { Box::from_raw(inner.card.as_ptr()) };
        if let Err((_, e)) = card.disconnect(Disposition::LeaveCard) {
            log::debug!("disconnecting card: {e}");
        }
    }
}

/// Whether a short APDU carries an Le byte (case 2 or case 4).
fn has_le(apdu: &[u8]) -> bool {
    match apdu.len() {
        0..=4 => false,
        5 => true,
        n => n == 6 + apdu[4] as usize,
    }
}

/// Status of a reader as reported by [`ReaderMonitor`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderStatus {
    pub name: String,
    pub card_present: bool,
    /// ATR as lowercase hex, when a card is present.
    pub atr: Option<String>,
}

/// Formats reader state flags (port of bas-celik `card.FormatState`, used for logging).
pub fn format_state(state: State) -> String {
    let flags = [
        (State::IGNORE, "StateIgnore"),
        (State::CHANGED, "StateChanged"),
        (State::UNKNOWN, "StateUnknown"),
        (State::PRESENT, "StatePresent"),
        (State::ATRMATCH, "StateAtrmatch"),
        (State::EXCLUSIVE, "StateExclusive"),
        (State::MUTE, "StateMute"),
        (State::UNPOWERED, "StateUnpowered"),
    ];
    flags
        .iter()
        .filter(|(f, _)| state.contains(*f))
        .map(|(_, n)| *n)
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone)]
struct KnownReader {
    name: CString,
    state: State,
    atr: Vec<u8>,
}

impl KnownReader {
    fn status(&self) -> ReaderStatus {
        let present = self.state.contains(State::PRESENT);
        ReaderStatus {
            name: self.name.to_string_lossy().into_owned(),
            card_present: present,
            atr: (present && !self.atr.is_empty()).then(|| to_hex(&self.atr)),
        }
    }
}

/// Handle that interrupts a blocking [`ReaderMonitor::wait_change`] from another thread.
#[derive(Clone)]
pub struct MonitorCanceller(Context);

impl MonitorCanceller {
    pub fn cancel(&self) {
        let _ = self.0.cancel();
    }
}

/// Polling interval used when the platform has no PnP notifications or the
/// PC/SC service is not running.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Blocking watcher that waits for reader/card changes (SCardGetStatusChange).
///
/// Reader plug/unplug is detected through the `\\?PnP?\Notification`
/// pseudo-reader where supported; elsewhere the reader list is polled.
/// The monitor is `Send` and meant to live on a background thread.
pub struct ReaderMonitor {
    ctx: Option<Context>,
    readers: Vec<KnownReader>,
    pnp_supported: bool,
    pnp_state: State,
    initialized: bool,
}

impl ReaderMonitor {
    /// Creates the monitor. Never fails because the PC/SC service is missing;
    /// the connection is (re)established lazily.
    pub fn new() -> Result<Self> {
        let ctx = match Context::establish(Scope::User) {
            Ok(c) => Some(c),
            Err(PcscError::NoService | PcscError::ServiceStopped) => None,
            Err(e) => return Err(map_pcsc_error(e)),
        };
        Ok(Self {
            ctx,
            readers: Vec::new(),
            pnp_supported: true,
            pnp_state: State::UNAWARE,
            initialized: false,
        })
    }

    /// Returns a handle that can interrupt a blocking `wait_change` call.
    /// `None` while the PC/SC service is unavailable.
    pub fn canceller(&self) -> Option<MonitorCanceller> {
        self.ctx.clone().map(MonitorCanceller)
    }

    /// Current status of all known readers (as of the last `wait_change`).
    pub fn statuses(&self) -> Vec<ReaderStatus> {
        self.readers.iter().map(KnownReader::status).collect()
    }

    fn reset(&mut self) {
        self.ctx = None;
        self.readers.clear();
        self.pnp_state = State::UNAWARE;
    }

    /// Block up to `timeout` until something changes; returns current status of all readers.
    /// The first call returns immediately. When the PC/SC service is not
    /// available it sleeps (at most `timeout`, capped to one second) and
    /// returns [`CardError::NoReader`].
    pub fn wait_change(&mut self, timeout: Duration) -> Result<Vec<ReaderStatus>> {
        // A PnP event triggers one immediate re-scan of the reader list.
        let mut timeout = timeout;
        for _ in 0..2 {
            match self.wait_once(timeout) {
                Ok(true) => timeout = Duration::ZERO,
                Ok(false) => return Ok(self.statuses()),
                Err(e) => return Err(e),
            }
        }
        Ok(self.statuses())
    }

    /// One round of waiting. Returns `Ok(true)` if the reader list should be re-scanned.
    fn wait_once(&mut self, timeout: Duration) -> Result<bool> {
        if self.ctx.is_none() {
            match Context::establish(Scope::User) {
                Ok(c) => self.ctx = Some(c),
                Err(PcscError::NoService | PcscError::ServiceStopped) => {
                    std::thread::sleep(timeout.min(POLL_INTERVAL));
                    return Err(CardError::NoReader);
                }
                Err(e) => return Err(map_pcsc_error(e)),
            }
        }
        let ctx = self.ctx.clone().expect("context established above");

        let names = match list_readers_in(&ctx) {
            Ok(n) => n,
            Err(PcscError::NoService | PcscError::ServiceStopped | PcscError::InvalidHandle) => {
                self.reset();
                std::thread::sleep(timeout.min(POLL_INTERVAL));
                return Err(CardError::NoReader);
            }
            Err(e) => return Err(map_pcsc_error(e)),
        };

        let list_changed = !self.initialized
            || names.len() != self.readers.len()
            || names.iter().zip(&self.readers).any(|(n, r)| *n != r.name);
        self.initialized = true;

        let mut states: Vec<ReaderState> = names
            .iter()
            .map(|n| {
                let current = self
                    .readers
                    .iter()
                    .find(|r| r.name == *n)
                    .map_or(State::UNAWARE, |r| r.state);
                ReaderState::new(n.clone(), current)
            })
            .collect();

        let use_pnp = self.pnp_supported;
        if use_pnp {
            let current = if self.pnp_state == State::UNAWARE {
                // The high word carries the reader count (pcsclite and Windows).
                State::from_bits_retain(((names.len() as u32) << 16) as _)
            } else {
                self.pnp_state
            };
            states.push(ReaderState::new(pcsc::PNP_NOTIFICATION(), current));
        }

        let mut wait = if list_changed {
            Duration::ZERO
        } else {
            timeout
        };
        if !use_pnp {
            wait = wait.min(POLL_INTERVAL);
        }

        let result = if states.is_empty() {
            std::thread::sleep(wait);
            Err(PcscError::Timeout)
        } else {
            ctx.get_status_change(wait, &mut states)
        };

        match result {
            Ok(()) | Err(PcscError::Timeout) | Err(PcscError::Cancelled) => {}
            Err(PcscError::UnknownReader) if use_pnp => {
                // Either a reader vanished or PnP notifications are unsupported
                // (then fall back to polling the reader list).
                let same = list_readers_in(&ctx).map(|n| n == names).unwrap_or(false);
                if same {
                    log::debug!("GetStatusChange: unknown reader, disabling PnP notifications");
                    self.pnp_supported = false;
                }
                return Ok(true);
            }
            Err(PcscError::UnknownReader) => return Ok(true),
            Err(PcscError::NoService | PcscError::ServiceStopped | PcscError::InvalidHandle) => {
                self.reset();
                return Err(CardError::NoReader);
            }
            Err(e) => return Err(map_pcsc_error(e)),
        }

        let mut rescan = false;
        let mut new_readers = Vec::with_capacity(names.len());
        for rs in &mut states {
            let event = rs.event_state();
            if rs.name() == pcsc::PNP_NOTIFICATION() {
                if event.contains(State::CHANGED) {
                    rescan = true;
                }
                rs.sync_current_state();
                self.pnp_state = rs.current_state();
                continue;
            }
            let changed = event.contains(State::CHANGED);
            if changed && event.intersects(State::UNKNOWN | State::IGNORE) {
                rescan = true;
            }
            if changed {
                log::debug!("reader {:?}: {}", rs.name(), format_state(event));
            }
            let prev = self.readers.iter().find(|r| r.name.as_c_str() == rs.name());
            let atr = if changed || prev.is_none() {
                rs.atr().to_vec()
            } else {
                prev.map(|p| p.atr.clone()).unwrap_or_default()
            };
            let state = if changed || prev.is_none() {
                event - State::CHANGED
            } else {
                prev.map_or(event, |p| p.state)
            };
            new_readers.push(KnownReader {
                name: CStr::to_owned(rs.name()),
                state,
                atr,
            });
        }
        self.readers = new_readers;

        Ok(rescan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format() {
        assert_eq!(format_state(State::empty()), "");
        assert_eq!(format_state(State::PRESENT), "StatePresent");
        assert_eq!(format_state(State::UNAWARE), "");
        assert_eq!(
            format_state(State::MUTE | State::CHANGED),
            "StateChanged StateMute"
        );
        assert_eq!(
            format_state(
                State::IGNORE
                    | State::CHANGED
                    | State::UNKNOWN
                    | State::PRESENT
                    | State::ATRMATCH
                    | State::EXCLUSIVE
                    | State::MUTE
                    | State::UNPOWERED
            ),
            "StateIgnore StateChanged StateUnknown StatePresent StateAtrmatch StateExclusive StateMute StateUnpowered"
        );
    }

    #[test]
    fn le_detection() {
        assert!(!has_le(&[0, 0xA4, 0, 0]));
        assert!(has_le(&[0, 0xB0, 0, 0, 4]));
        assert!(!has_le(&[0, 0xA4, 0, 0, 2, 0x0F, 0x02]));
        assert!(has_le(&[0, 0xA4, 0, 0, 2, 0x0F, 0x02, 4]));
    }

    #[test]
    fn status() {
        let r = KnownReader {
            name: CString::new("R").unwrap(),
            state: State::PRESENT,
            atr: vec![0x3B, 0xAA],
        };
        assert_eq!(
            r.status(),
            ReaderStatus {
                name: "R".into(),
                card_present: true,
                atr: Some("3baa".into())
            }
        );
        let r = KnownReader {
            state: State::EMPTY,
            ..r
        };
        assert_eq!(r.status().atr, None);
    }

    fn assert_send<T: Send>() {}

    #[test]
    fn send() {
        assert_send::<ReaderMonitor>();
        assert_send::<PcscCard>();
    }

    /// Must not panic or hang without a PC/SC daemon / readers.
    #[test]
    fn no_service_is_graceful() {
        let readers = list_readers();
        assert!(readers.is_ok(), "{readers:?}");
        if let Ok(mut m) = ReaderMonitor::new() {
            match m.wait_change(Duration::from_millis(50)) {
                Ok(_) | Err(CardError::NoReader) => {}
                Err(e) => panic!("unexpected error {e}"),
            }
        }
        if readers.unwrap().is_empty() {
            assert!(matches!(
                PcscCard::connect("No such reader"),
                Err(CardError::NoReader)
            ));
        }
    }
}
