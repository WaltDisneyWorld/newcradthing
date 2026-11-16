//! Apple Mobile Device Support detection.
//!
//! The original iCard links the data path through Apple's MobileDevice.dll /
//! CoreFoundation.dll / AirTrafficHost.dll (installed with 64-bit iTunes or the
//! standalone "Apple Mobile Device Support"). That library owns the device TLS
//! session, which is why it is required for services the device marks
//! SSL-only. This module locates that runtime and reports its status for the UI;
//! device enumeration and the unencrypted service path are handled in pure Rust
//! by the usbmux / lockdown / afc modules.

#[derive(Debug, Clone)]
#[allow(dead_code)] // Ready/Missing are only constructed on the Windows backend
pub enum SupportStatus {
    /// Apple Mobile Device Support found and the core DLLs are present.
    Ready { path: String },
    /// Not installed / not found.
    Missing,
    /// Not applicable on this platform.
    NotApplicable,
}

impl SupportStatus {
    pub fn describe(&self) -> String {
        match self {
            SupportStatus::Ready { path } => format!("Apple Support Runtime: {path}"),
            SupportStatus::Missing => {
                "Apple Mobile Device Support was not found. Install 64-bit iTunes package from Apple."
                    .to_string()
            }
            SupportStatus::NotApplicable => {
                "Apple Mobile Device Support: n/a on this OS (usbmux transport in use)".to_string()
            }
        }
    }
    pub fn is_ready(&self) -> bool {
        matches!(self, SupportStatus::Ready { .. })
    }
}

#[cfg(windows)]
pub fn detect() -> SupportStatus {
    use std::path::Path;
    const CANDIDATES: [&str; 2] = [
        r"C:\Program Files\Common Files\Apple\Mobile Device Support",
        r"C:\Program Files (x86)\Common Files\Apple\Mobile Device Support",
    ];
    for base in CANDIDATES {
        let dir = Path::new(base);
        if dir.join("MobileDevice.dll").exists() || dir.join("CoreFoundation.dll").exists() {
            // Try to load the core library to confirm it is usable.
            let mobiledevice = dir.join("MobileDevice.dll");
            let usable = unsafe { libloading::Library::new(&mobiledevice).is_ok() }
                || dir.join("CoreFoundation.dll").exists();
            if usable {
                return SupportStatus::Ready {
                    path: base.to_string(),
                };
            }
        }
    }
    SupportStatus::Missing
}

#[cfg(not(windows))]
pub fn detect() -> SupportStatus {
    SupportStatus::NotApplicable
}
