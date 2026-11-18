//! Local activation gate.
//!
//! iCard gates its device-writing actions behind an activation code checked
//! entirely on the machine (no network, no account). State persists to
//! `%LOCALAPPDATA%\AirCard\activation.json`. This reproduces the original's
//! "local gate" behaviour: a self-checking code format, validated offline.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Activation {
    #[serde(default)]
    pub activated: bool,
    #[serde(default)]
    pub code: String,
}

fn store_path() -> std::path::PathBuf {
    crate::app::data_dir().join("activation.json")
}

impl Activation {
    pub fn load() -> Self {
        std::fs::read(store_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            let path = store_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, json);
        }
    }

    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.activated && validate(&self.code)
    }

    /// Try to activate with a code. Returns true on success and persists.
    #[allow(dead_code)]
    pub fn try_activate(&mut self, code: &str) -> bool {
        if validate(code) {
            self.activated = true;
            self.code = normalize(code);
            self.save();
            true
        } else {
            false
        }
    }
}

pub(crate) const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

pub(crate) fn normalize(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// A code is 16 base-32 symbols (grouped as XXXX-XXXX-XXXX-XXXX). The final
/// symbol is a checksum over the first 15, so the gate can verify a code offline
/// without a key list.
pub(crate) fn validate(code: &str) -> bool {
    let norm = normalize(code);
    if norm.len() != 16 {
        return false;
    }
    let mut values = Vec::with_capacity(16);
    for ch in norm.bytes() {
        match ALPHABET.iter().position(|&a| a == ch) {
            Some(v) => values.push(v as u32),
            None => return false,
        }
    }
    let (check, body) = values.split_last().unwrap();
    // Weighted checksum (Luhn-like over base 32).
    let mut sum = 0u32;
    for (i, v) in body.iter().enumerate() {
        let w = if i % 2 == 0 { 3 } else { 1 };
        sum = sum.wrapping_add(v.wrapping_mul(w));
    }
    (sum % 32) == *check
}

/// Produce a valid activation code from a 15-symbol seed (used by tests and to
/// mint demo codes).
#[allow(dead_code)]
pub fn mint(seed15: &str) -> Option<String> {
    let norm = normalize(seed15);
    if norm.len() != 15 {
        return None;
    }
    let mut values = Vec::with_capacity(15);
    for ch in norm.bytes() {
        values.push(ALPHABET.iter().position(|&a| a == ch)? as u32);
    }
    let mut sum = 0u32;
    for (i, v) in values.iter().enumerate() {
        let w = if i % 2 == 0 { 3 } else { 1 };
        sum = sum.wrapping_add(v.wrapping_mul(w));
    }
    let check = ALPHABET[(sum % 32) as usize] as char;
    let mut out = norm;
    out.push(check);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minted_codes_validate() {
        let code = mint("ABCDEFGHJKLMNPQ").unwrap();
        assert!(validate(&code));
        assert!(!validate("ABCD-EFGH-JKLM-NPQR")); // wrong checksum
        assert!(!validate("too-short"));
    }
}
