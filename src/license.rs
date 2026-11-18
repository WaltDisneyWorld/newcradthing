//! Licensing: HWID-bound activation + a local license store/manager.
//!
//! A license *key* is the same 16-symbol checksummed code the activation gate
//! validates. Each key may be bound to one machine's **HWID** (hardware
//! fingerprint). The License Manager (admin-gated) can generate keys, view their
//! bindings, reset a key's HWID, revoke/restore and delete.
//!
//! This is an entirely local scheme — no server, no network. The store lives at
//! `%LOCALAPPDATA%\ProCard\licenses.json`.

use serde::{Deserialize, Serialize};

use crate::activation;

/// Master code that unlocks the License Manager panel (and HWID resets).
pub const ADMIN_CODE: &str = "PRDCARDADMNKY239";

#[allow(dead_code)]
pub fn admin_code_display() -> String {
    group(ADMIN_CODE)
}

/// Format a 16-char code as XXXX-XXXX-XXXX-XXXX.
pub fn group(code: &str) -> String {
    let c: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if c.len() == 16 {
        format!("{}-{}-{}-{}", &c[0..4], &c[4..8], &c[8..12], &c[12..16])
    } else {
        c
    }
}

// --------------------------------------------------------------------------
// HWID (hardware fingerprint)
// --------------------------------------------------------------------------

/// A stable per-machine identifier, hashed and grouped (XXXX-XXXX-XXXX-XXXX).
pub fn hwid() -> String {
    let raw = format!("{}|{}", machine_id(), std::env::consts::ARCH);
    let h = fnv1a64(raw.as_bytes());
    let hex = format!("{h:016X}");
    format!("{}-{}-{}-{}", &hex[0..4], &hex[4..8], &hex[8..12], &hex[12..16])
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Best-effort stable machine id across platforms; falls back to a persisted
/// install salt if no OS identifier is available.
fn machine_id() -> String {
    #[cfg(target_os = "linux")]
    {
        for p in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
            if let Ok(s) = std::fs::read_to_string(p) {
                let s = s.trim();
                if !s.is_empty() {
                    return s.to_string();
                }
            }
        }
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        if let Ok(out) = Command::new("reg")
            .args([
                "query",
                r"HKLM\SOFTWARE\Microsoft\Cryptography",
                "/v",
                "MachineGuid",
            ])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(line) = text.lines().find(|l| l.contains("MachineGuid")) {
                if let Some(val) = line.split_whitespace().last() {
                    return val.to_string();
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(out) = Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(line) = text.lines().find(|l| l.contains("IOPlatformUUID")) {
                if let Some(v) = line.split('"').nth(3) {
                    return v.to_string();
                }
            }
        }
    }
    persisted_salt()
}

/// A random id generated once and stored, used when no OS machine id is found.
fn persisted_salt() -> String {
    let path = crate::app::data_dir().join("machine.salt");
    if let Ok(s) = std::fs::read_to_string(&path) {
        if !s.trim().is_empty() {
            return s.trim().to_string();
        }
    }
    let seed = format!(
        "{:?}-{}-{}",
        std::time::SystemTime::now(),
        std::process::id(),
        std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default()
    );
    let salt = format!("{:016X}", fnv1a64(seed.as_bytes()));
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, &salt);
    salt
}

// --------------------------------------------------------------------------
// License store
// --------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseRecord {
    pub key: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub hwid: Option<String>,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub revoked: bool,
}

impl LicenseRecord {
    pub fn status(&self, current_hwid: &str) -> LicenseStatus {
        if self.revoked {
            LicenseStatus::Revoked
        } else if self.hwid.as_deref() == Some(current_hwid) {
            LicenseStatus::ActiveHere
        } else if self.hwid.is_some() {
            LicenseStatus::BoundElsewhere
        } else {
            LicenseStatus::Free
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseStatus {
    Free,
    ActiveHere,
    BoundElsewhere,
    Revoked,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LicenseStore {
    #[serde(default)]
    pub records: Vec<LicenseRecord>,
}

pub enum ActivateOutcome {
    BoundHere,
    AlreadyBound,
}

impl LicenseStore {
    fn path() -> std::path::PathBuf {
        crate::app::data_dir().join("licenses.json")
    }

    pub fn load() -> Self {
        std::fs::read(Self::path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn find(&self, key: &str) -> Option<&LicenseRecord> {
        let k = activation::normalize(key);
        self.records.iter().find(|r| r.key == k)
    }

    fn find_mut(&mut self, key: &str) -> Option<&mut LicenseRecord> {
        let k = activation::normalize(key);
        self.records.iter_mut().find(|r| r.key == k)
    }

    /// Validate + bind a key to this machine. Creates a record on first use so
    /// standalone/demo keys work. Returns an outcome or a human error.
    pub fn activate(&mut self, key: &str, hwid: &str) -> Result<ActivateOutcome, String> {
        let k = activation::normalize(key);
        if !activation::validate(&k) {
            return Err("Invalid license code.".into());
        }
        if let Some(rec) = self.find_mut(&k) {
            if rec.revoked {
                return Err("This license has been revoked.".into());
            }
            match &rec.hwid {
                Some(h) if h == hwid => return Ok(ActivateOutcome::AlreadyBound),
                Some(_) => {
                    return Err(
                        "License is already bound to another device. Reset its HWID in the License Manager.".into(),
                    )
                }
                None => {
                    rec.hwid = Some(hwid.to_string());
                    self.save();
                    return Ok(ActivateOutcome::BoundHere);
                }
            }
        }
        // Unknown but valid key — register and bind it here.
        self.records.push(LicenseRecord {
            key: k,
            label: "Activated".into(),
            hwid: Some(hwid.to_string()),
            created: now_iso(),
            revoked: false,
        });
        self.save();
        Ok(ActivateOutcome::BoundHere)
    }

    /// Is this key valid and bound to this machine right now?
    pub fn is_active(&self, key: &str, hwid: &str) -> bool {
        let k = activation::normalize(key);
        if !activation::validate(&k) {
            return false;
        }
        match self.find(&k) {
            Some(r) => !r.revoked && r.hwid.as_deref() == Some(hwid),
            None => false,
        }
    }

    /// Mint a new, valid, free license key.
    pub fn generate(&mut self, label: &str) -> String {
        let key = loop {
            let seed = random_seed(self.records.len());
            if let Some(k) = activation::mint(&seed) {
                if self.find(&k).is_none() {
                    break k;
                }
            }
        };
        self.records.push(LicenseRecord {
            key: key.clone(),
            label: if label.trim().is_empty() { "Generated".into() } else { label.trim().to_string() },
            hwid: None,
            created: now_iso(),
            revoked: false,
        });
        self.save();
        key
    }

    pub fn reset_hwid(&mut self, key: &str) {
        if let Some(r) = self.find_mut(key) {
            r.hwid = None;
            self.save();
        }
    }

    pub fn set_revoked(&mut self, key: &str, revoked: bool) {
        if let Some(r) = self.find_mut(key) {
            r.revoked = revoked;
            self.save();
        }
    }

    pub fn delete(&mut self, key: &str) {
        let k = activation::normalize(key);
        self.records.retain(|r| r.key != k);
        self.save();
    }
}

/// Deterministic-ish random 15-symbol base-32 seed for `activation::mint`.
fn random_seed(counter: usize) -> String {
    let alphabet = activation::ALPHABET;
    let mut state = fnv1a64(
        format!(
            "{:?}-{}-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default(),
            std::process::id(),
            counter
        )
        .as_bytes(),
    );
    let mut out = String::with_capacity(15);
    for _ in 0..15 {
        // xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.push(alphabet[(state % 32) as usize] as char);
    }
    out
}

fn now_iso() -> String {
    let t = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        t.year(),
        t.month() as u8,
        t.day(),
        t.hour(),
        t.minute()
    )
}
