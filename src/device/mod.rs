//! Device facade: enumeration + service connections used by the wallet layer.

pub mod afc;
pub mod apple;
pub mod lockdown;
pub mod syslog;
pub mod transport;
pub mod usbmux;

use std::io;

use crate::model::{DeviceInfo, TransportMode};

/// Known lockdown service names used by iCard.
pub mod service {
    pub const AFC: &str = "com.apple.afc";
    pub const SYSLOG_RELAY: &str = "com.apple.syslog_relay";
    pub const STREAMING_ZIP: &str = "com.apple.streaming_zip_conduit";
    #[allow(dead_code)]
    pub const CRASH_COPY_MOBILE: &str = "com.apple.crashreportcopymobile";
}

/// List devices, filtered by the chosen transport mode.
pub fn list_devices(mode: TransportMode) -> io::Result<Vec<DeviceInfo>> {
    use crate::model::ConnectionType;
    let mut devices = usbmux::list_devices()?;
    devices.retain(|d| match mode {
        TransportMode::Auto => true,
        TransportMode::UsbOnly => d.connection == ConnectionType::Usb,
        TransportMode::WifiOnly => d.connection == ConnectionType::Network,
    });

    // In Auto mode, if a UDID is reachable both ways, prefer the USB record.
    devices.sort_by_key(|d| match d.connection {
        ConnectionType::Usb => 0,
        ConnectionType::Network => 1,
    });
    devices.dedup_by(|a, b| a.udid == b.udid);

    // Enrich with name / version via lockdown GetValue (best effort).
    for d in devices.iter_mut() {
        if let Ok(mut ld) = lockdown::Lockdown::open(d.device_id) {
            d.name = ld.get_string("DeviceName").unwrap_or_default();
            d.ios_version = ld.get_string("ProductVersion").unwrap_or_default();
            d.product_type = ld.get_string("ProductType").unwrap_or_default();
        }
    }
    Ok(devices)
}

/// Open a connected, session-authenticated pipe to a named lockdown service.
pub fn open_service(device: &DeviceInfo, name: &str) -> io::Result<Box<dyn transport::Transport>> {
    let pair = lockdown::load_pair_record(&device.udid)?;
    let mut ld = lockdown::Lockdown::open(device.device_id)?;
    let session_ssl = ld.start_session(&pair)?;
    let started = ld.start_service(name, session_ssl)?;
    let pipe = usbmux::connect_port(device.device_id, started.port)?;
    if started.enable_ssl {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "Service {name} requires TLS, which the pure-Rust transport does not provide on \
                 this platform. Install Apple Mobile Device Support (Windows) to use it."
            ),
        ));
    }
    Ok(pipe)
}

/// Open an AFC session on the device.
pub fn open_afc(device: &DeviceInfo) -> io::Result<afc::Afc> {
    let pipe = open_service(device, service::AFC)?;
    Ok(afc::Afc::new(pipe))
}
