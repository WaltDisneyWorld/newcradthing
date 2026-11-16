//! Shared data types used across the UI, device and wallet layers.

use serde::{Deserialize, Serialize};

/// Which transport iCard should use to reach the iPhone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportMode {
    /// Prefer USB, fall back to WiFi (network) pairing.
    Auto,
    /// USB only.
    UsbOnly,
    /// WiFi (network) only.
    WifiOnly,
}

impl Default for TransportMode {
    fn default() -> Self {
        TransportMode::Auto
    }
}

impl TransportMode {
    pub fn all() -> [TransportMode; 3] {
        [
            TransportMode::Auto,
            TransportMode::UsbOnly,
            TransportMode::WifiOnly,
        ]
    }
}

/// The physical link a usbmux device record arrived over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Usb,
    Network,
}

/// A connected iOS device, as reported by usbmuxd / Apple Mobile Device Support.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub udid: String,
    pub name: String,
    pub product_type: String,
    pub ios_version: String,
    pub connection: ConnectionType,
    /// usbmux device id, used when opening a service connection.
    pub device_id: u32,
}

impl DeviceInfo {
    pub fn label(&self) -> String {
        if self.name.is_empty() {
            self.udid.clone()
        } else {
            format!("{} ({})", self.name, short_udid(&self.udid))
        }
    }
}

pub fn short_udid(udid: &str) -> String {
    if udid.len() > 12 {
        format!("{}…{}", &udid[..6], &udid[udid.len() - 4..])
    } else {
        udid.to_string()
    }
}

/// A card hash the user has captured / entered, persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedCard {
    pub hash: String,
    /// Friendly name seen in the Wallet syslog (e.g. the pass's organization).
    #[serde(default)]
    pub name: String,
    /// ISO-8601 timestamp of when it was first captured.
    #[serde(default)]
    pub captured_at: String,
}

/// The localisation / dialer-keypad language the passcode theme targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeypadLanguage {
    /// Pick the newest TelephonyUI variant automatically.
    Auto,
    /// iOS 18+.
    TelephonyUi10,
    /// iOS 16–17.
    TelephonyUi9,
    /// Legacy.
    TelephonyUi8,
}

impl Default for KeypadLanguage {
    fn default() -> Self {
        KeypadLanguage::Auto
    }
}

impl KeypadLanguage {
    pub fn all() -> [KeypadLanguage; 4] {
        [
            KeypadLanguage::Auto,
            KeypadLanguage::TelephonyUi10,
            KeypadLanguage::TelephonyUi9,
            KeypadLanguage::TelephonyUi8,
        ]
    }

    /// The on-device TelephonyUI bundle directory this variant writes into.
    pub fn bundle(self) -> &'static str {
        match self {
            KeypadLanguage::Auto | KeypadLanguage::TelephonyUi10 => "TelephonyUI-10",
            KeypadLanguage::TelephonyUi9 => "TelephonyUI-9",
            KeypadLanguage::TelephonyUi8 => "TelephonyUI-8",
        }
    }
}

/// UI language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    English,
    Spanish,
}

impl Default for Language {
    fn default() -> Self {
        Language::English
    }
}

/// Persisted user settings (`settings.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub transport: TransportMode,
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub keypad_language: KeypadLanguage,
    #[serde(default)]
    pub generate_bold_variant: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            transport: TransportMode::Auto,
            language: Language::English,
            keypad_language: KeypadLanguage::Auto,
            generate_bold_variant: true,
        }
    }
}

/// The fixed canvas every Wallet card face is rendered to.
pub const CARD_CANVAS_W: u32 = 1536;
pub const CARD_CANVAS_H: u32 = 969;
