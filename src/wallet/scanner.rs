//! Syslog-driven card-hash scanner.
//!
//! Starts `com.apple.syslog_relay`, watches the stream for Wallet / PassKit
//! events and reports the first card hash it sees.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::device::{self, syslog};
use crate::model::DeviceInfo;

/// A captured card hash handed to the UI thread.
pub struct ScanHit {
    pub hash: String,
    pub name: String,
}

pub enum ScanEvent {
    Log(String),
    Hit(syslog::CardHit),
    Finished(String),
    Error(String),
}

/// Run a syslog scan until `stop` is set. Blocking; intended for a worker thread.
pub fn run_scan(device: &DeviceInfo, stop: Arc<AtomicBool>, mut emit: impl FnMut(ScanEvent)) {
    emit(ScanEvent::Log(
        "Starting com.apple.syslog_relay service on device...".into(),
    ));
    let pipe = match device::open_service(device, device::service::SYSLOG_RELAY) {
        Ok(p) => p,
        Err(e) => {
            emit(ScanEvent::Error(format!(
                "Failed to start com.apple.syslog_relay service: {e}"
            )));
            return;
        }
    };
    emit(ScanEvent::Log(
        "Syslog relay established. Listening for Wallet & PassKit events...".into(),
    ));
    emit(ScanEvent::Log(
        "Tip: Open Apple Wallet on your iPhone or tap your card to trigger events.".into(),
    ));

    let matcher = syslog::HashMatcher::new();
    let found = Arc::new(AtomicBool::new(false));
    let found2 = found.clone();
    let result = syslog::relay_loop(pipe, stop.clone(), |line| {
        if let Some(hit) = matcher.scan_line(line) {
            if !found2.swap(true, std::sync::atomic::Ordering::Relaxed) {
                emit(ScanEvent::Log(format!("Found card pass! Name: '{}'", hit.name)));
                emit(ScanEvent::Hit(hit));
            }
        }
    });

    match result {
        Ok(()) => emit(ScanEvent::Finished("Syslog scan finished".into())),
        Err(e) => emit(ScanEvent::Error(format!("Syslog socket closed by device: {e}"))),
    }
}
