//! Minimal lockdownd client (device port 62078).
//!
//! lockdownd is the on-device configuration daemon. To start a data service
//! (AFC, syslog_relay, streaming_zip_conduit, …) we: open the lockdown port via
//! usbmux, `QueryType`, start a trusted session using the host pairing record,
//! then `StartService`. Services flagged `EnableServiceSSL` must then be wrapped
//! in TLS using the pairing record's certificates.

use std::io;
use std::path::PathBuf;

use super::transport::Transport;
use super::usbmux;

const LOCKDOWN_PORT: u16 = 62078;

#[allow(dead_code)]
pub struct StartedService {
    pub name: String,
    pub port: u16,
    pub enable_ssl: bool,
}

/// The host-side pairing record for a device (written by iTunes / Apple Mobile
/// Device Support when the user taps "Trust This Computer").
#[derive(Clone)]
#[allow(dead_code)] // `raw` retains the full record for the TLS backend
pub struct PairRecord {
    pub host_id: String,
    pub system_buid: String,
    pub raw: plist::Dictionary,
}

pub struct Lockdown {
    t: Box<dyn Transport>,
}

fn send(t: &mut dyn Transport, v: &plist::Value) -> io::Result<()> {
    let mut body = Vec::new();
    plist::to_writer_xml(&mut body, v)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    t.write_all(&(body.len() as u32).to_be_bytes())?;
    t.write_all(&body)?;
    Ok(())
}

fn recv(t: &mut dyn Transport) -> io::Result<plist::Dictionary> {
    let mut len = [0u8; 4];
    t.read_exact(&mut len)?;
    let n = u32::from_be_bytes(len) as usize;
    if n == 0 || n > 8 * 1024 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bad lockdown frame length"));
    }
    let mut body = vec![0u8; n];
    t.read_exact(&mut body)?;
    let v: plist::Value = plist::from_bytes(&body)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    v.into_dictionary()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "lockdown: expected dictionary"))
}

fn req(request: &str) -> plist::Dictionary {
    let mut d = plist::Dictionary::new();
    d.insert("Request".into(), request.into());
    d.insert("Label".into(), "ProCard".into());
    d
}

fn check_ok(d: &plist::Dictionary) -> io::Result<()> {
    if let Some(err) = d.get("Error").and_then(|v| v.as_string()) {
        return Err(io::Error::new(io::ErrorKind::Other, format!("lockdown error: {err}")));
    }
    Ok(())
}

impl Lockdown {
    pub fn open(device_id: u32) -> io::Result<Self> {
        let t = usbmux::connect_port(device_id, LOCKDOWN_PORT)?;
        let mut ld = Lockdown { t };
        // Verify we are actually talking to lockdownd.
        send(&mut *ld.t, &plist::Value::Dictionary(req("QueryType")))?;
        let reply = recv(&mut *ld.t)?;
        check_ok(&reply)?;
        Ok(ld)
    }

    pub fn get_string(&mut self, key: &str) -> io::Result<String> {
        let mut d = req("GetValue");
        d.insert("Key".into(), key.into());
        send(&mut *self.t, &plist::Value::Dictionary(d))?;
        let reply = recv(&mut *self.t)?;
        check_ok(&reply)?;
        Ok(reply
            .get("Value")
            .and_then(|v| v.as_string())
            .unwrap_or("")
            .to_string())
    }

    /// Start a trusted lockdown session from the stored pairing record.
    pub fn start_session(&mut self, pair: &PairRecord) -> io::Result<bool> {
        let mut d = req("StartSession");
        d.insert("HostID".into(), pair.host_id.clone().into());
        d.insert("SystemBUID".into(), pair.system_buid.clone().into());
        send(&mut *self.t, &plist::Value::Dictionary(d))?;
        let reply = recv(&mut *self.t)?;
        check_ok(&reply)?;
        let ssl = reply
            .get("EnableSessionSSL")
            .and_then(|v| v.as_boolean())
            .unwrap_or(false);
        // NOTE: when `ssl` is true the lockdown connection itself must now be
        // upgraded to TLS using the pairing record certs before StartService is
        // sent. The Windows Apple-backed path (device/apple) delegates this to
        // MobileDevice.dll. The pure-Rust path returns a clear error from
        // `start_service` in that case rather than speaking cleartext.
        Ok(ssl)
    }

    pub fn start_service(&mut self, service: &str, session_ssl: bool) -> io::Result<StartedService> {
        if session_ssl {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "This device requires a TLS lockdown session, which the pure-Rust transport does \
                 not provide. Use the Apple Mobile Device Support backend (Windows) for this device.",
            ));
        }
        let mut d = req("StartService");
        d.insert("Service".into(), service.into());
        send(&mut *self.t, &plist::Value::Dictionary(d))?;
        let reply = recv(&mut *self.t)?;
        check_ok(&reply)?;
        let port = reply
            .get("Port")
            .and_then(|v| v.as_unsigned_integer())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "StartService: no Port"))?
            as u16;
        let enable_ssl = reply
            .get("EnableServiceSSL")
            .and_then(|v| v.as_boolean())
            .unwrap_or(false);
        Ok(StartedService {
            name: service.to_string(),
            port,
            enable_ssl,
        })
    }
}

/// Where Apple Mobile Device Support / iTunes keeps host pairing records.
fn lockdown_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        // %ALLUSERSPROFILE%\Apple\Lockdown
        if let Ok(pd) = std::env::var("ALLUSERSPROFILE") {
            return Some(PathBuf::from(pd).join("Apple").join("Lockdown"));
        }
        None
    }
    #[cfg(target_os = "macos")]
    {
        Some(PathBuf::from("/var/db/lockdown"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Some(PathBuf::from("/var/lib/lockdown"))
    }
}

/// Load the host pairing record for a given UDID, if present.
pub fn load_pair_record(udid: &str) -> io::Result<PairRecord> {
    let dir = lockdown_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "lockdown directory not found"))?;
    let path = dir.join(format!("{udid}.plist"));
    let raw: plist::Dictionary = plist::from_file(&path).map_err(|e| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "No pairing record for this device ({}). Connect by USB and tap \
                 \"Trust This Computer\" once. [{e}]",
                crate::model::short_udid(udid)
            ),
        )
    })?;
    let host_id = raw
        .get("HostID")
        .and_then(|v| v.as_string())
        .unwrap_or("")
        .to_string();
    let system_buid = raw
        .get("SystemBUID")
        .and_then(|v| v.as_string())
        .unwrap_or("")
        .to_string();
    Ok(PairRecord {
        host_id,
        system_buid,
        raw,
    })
}
