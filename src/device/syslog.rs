//! `com.apple.syslog_relay` reader + Wallet card-hash extraction.
//!
//! When the user opens Apple Wallet or taps a pass, PassKit logs the pass's
//! on-disk directory (its "card hash"). We stream the device syslog and pull the
//! hash out with the same two patterns the original iCard used.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use regex::Regex;

use super::transport::Transport;

/// A hash + an optional pass name scraped from the same log region.
#[derive(Debug, Clone)]
pub struct CardHit {
    pub hash: String,
    pub name: String,
}

pub struct HashMatcher {
    path_re: Regex,
    kv_re: Regex,
    name_re: Regex,
}

impl HashMatcher {
    pub fn new() -> Self {
        HashMatcher {
            path_re: Regex::new(
                r#"(?:Cards|Passes/Cards)/([A-Za-z0-9+/_-]{27,44})(?:\.pkpass|\.cache|\.pkcache|/|\s|"|'|\)|,|$)"#,
            )
            .unwrap(),
            kv_re: Regex::new(
                r#"(?i)(?:card[_\s]?(?:hash|id)|pass[_\s]?(?:hash|id)|unique[_\s]?id)\s*[:=]\s*['"]?([A-Za-z0-9+=_-]{27,44})"#,
            )
            .unwrap(),
            name_re: Regex::new(r#"(?i)(?:organizationName|passTypeIdentifier|localizedName)[":=\s]+['"]?([^'"\n,}]{1,64})"#)
                .unwrap(),
        }
    }

    pub fn scan_line(&self, line: &str) -> Option<CardHit> {
        let hash = self
            .path_re
            .captures(line)
            .or_else(|| self.kv_re.captures(line))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())?;
        let name = self
            .name_re
            .captures(line)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        Some(CardHit { hash, name })
    }
}

/// Read lines from the syslog service pipe until `stop` is set, calling `on_line`
/// for each decoded line.
pub fn relay_loop(
    mut t: Box<dyn Transport>,
    stop: Arc<AtomicBool>,
    mut on_line: impl FnMut(&str),
) -> io::Result<()> {
    t.set_read_timeout(Some(std::time::Duration::from_millis(400))).ok();
    let mut buf = [0u8; 8192];
    let mut line = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        match t.read(&mut buf) {
            Ok(0) => break, // "Syslog socket closed by device."
            Ok(n) => {
                for &b in &buf[..n] {
                    if b == b'\n' || b == 0 {
                        if !line.is_empty() {
                            let s = String::from_utf8_lossy(&line);
                            on_line(s.trim_end_matches('\r'));
                            line.clear();
                        }
                    } else {
                        line.push(b);
                        if line.len() > 16 * 1024 {
                            line.clear();
                        }
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {
                continue
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
