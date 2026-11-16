//! Pure-Rust usbmux client.
//!
//! usbmuxd (on Windows the "Apple Mobile Device Service", reachable at
//! 127.0.0.1:27015; on Unix the socket /var/run/usbmuxd) multiplexes TCP ports
//! on attached iOS devices over one host-side connection. We use it to:
//!   * enumerate attached devices (`ListDevices`), and
//!   * open a pipe to a device TCP port (`Connect`), e.g. lockdown on 62078.

use std::io::{self};
use std::time::Duration;

use crate::model::{ConnectionType, DeviceInfo};

use super::transport::{TcpTransport, Transport};

const USBMUX_TAG_PLIST: u32 = 8;
const PROG_NAME: &str = "iCard";

fn client_version() -> String {
    format!("iCard-{}", env!("CARGO_PKG_VERSION"))
}

/// Open a fresh connection to the local usbmux endpoint.
fn connect_muxd() -> io::Result<Box<dyn Transport>> {
    // Windows / Apple Mobile Device Support: TCP 127.0.0.1:27015.
    // Unix libusbmuxd: the unix socket, falling back to TCP 27015.
    #[cfg(unix)]
    {
        if let Ok(s) = std::os::unix::net::UnixStream::connect("/var/run/usbmuxd") {
            return Ok(Box::new(super::transport::UnixTransport(s)));
        }
    }
    let stream = std::net::TcpStream::connect(("127.0.0.1", 27015)).map_err(|e| {
        io::Error::new(
            e.kind(),
            "Could not connect to Apple Mobile Device Service (usbmuxd) at 127.0.0.1:27015. \
             Please ensure iTunes or Apple Mobile Device Support is installed and the service is running.",
        )
    })?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    Ok(Box::new(TcpTransport(stream)))
}

/// Frame + send a plist request on a usbmux connection.
fn send_plist(t: &mut dyn Transport, value: &plist::Value, tag: u32) -> io::Result<()> {
    let mut body = Vec::new();
    plist::to_writer_xml(&mut body, value)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let total = 16u32 + body.len() as u32;
    let mut hdr = Vec::with_capacity(16);
    hdr.extend_from_slice(&total.to_le_bytes());
    hdr.extend_from_slice(&1u32.to_le_bytes()); // protocol version
    hdr.extend_from_slice(&USBMUX_TAG_PLIST.to_le_bytes()); // message type: plist
    hdr.extend_from_slice(&tag.to_le_bytes());
    t.write_all(&hdr)?;
    t.write_all(&body)?;
    Ok(())
}

/// Read one framed plist reply.
fn recv_plist(t: &mut dyn Transport) -> io::Result<plist::Value> {
    let mut hdr = [0u8; 16];
    t.read_exact(&mut hdr)?;
    let total = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]) as usize;
    if total < 16 || total > 16 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid usbmux response length: {total}"),
        ));
    }
    let mut body = vec![0u8; total - 16];
    t.read_exact(&mut body)?;
    plist::from_bytes(&body)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Expected dictionary in usbmux response: {e}")))
}

fn base_request(message_type: &str) -> plist::Dictionary {
    let mut d = plist::Dictionary::new();
    d.insert("MessageType".into(), message_type.into());
    d.insert("ClientVersionString".into(), client_version().into());
    d.insert("ProgName".into(), PROG_NAME.into());
    d.insert("kLibUSBMuxVersion".into(), 3i64.into());
    d
}

/// Enumerate attached iOS devices.
pub fn list_devices() -> io::Result<Vec<DeviceInfo>> {
    let mut t = connect_muxd()?;
    let req = plist::Value::Dictionary(base_request("ListDevices"));
    send_plist(&mut *t, &req, 1)?;
    let reply = recv_plist(&mut *t)?;

    let dict = reply
        .as_dictionary()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Expected dictionary in usbmux response"))?;
    let list = dict
        .get("DeviceList")
        .and_then(|v| v.as_array())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Expected DeviceList array in usbmux response"))?;

    let mut out = Vec::new();
    for entry in list {
        let Some(e) = entry.as_dictionary() else { continue };
        let device_id = e.get("DeviceID").and_then(|v| v.as_unsigned_integer()).unwrap_or(0) as u32;
        let Some(props) = e.get("Properties").and_then(|v| v.as_dictionary()) else { continue };
        let udid = props
            .get("SerialNumber")
            .and_then(|v| v.as_string())
            .unwrap_or("")
            .to_string();
        let conn = match props.get("ConnectionType").and_then(|v| v.as_string()) {
            Some("Network") => ConnectionType::Network,
            _ => ConnectionType::Usb,
        };
        out.push(DeviceInfo {
            udid,
            name: String::new(),
            product_type: String::new(),
            ios_version: String::new(),
            connection: conn,
            device_id,
        });
    }
    Ok(out)
}

/// Open a raw pipe to a device TCP port. On success the returned transport is a
/// byte pipe straight to the service listening on `port` on the phone.
pub fn connect_port(device_id: u32, port: u16) -> io::Result<Box<dyn Transport>> {
    let mut t = connect_muxd()?;
    let mut d = base_request("Connect");
    d.insert("DeviceID".into(), (device_id as i64).into());
    // PortNumber is big-endian in the usbmux Connect message.
    d.insert("PortNumber".into(), (port.to_be() as i64).into());
    send_plist(&mut *t, &plist::Value::Dictionary(d), 1)?;
    let reply = recv_plist(&mut *t)?;
    let num = reply
        .as_dictionary()
        .and_then(|r| r.get("Number"))
        .and_then(|v| v.as_signed_integer())
        .unwrap_or(-1);
    if num != 0 {
        return Err(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("usbmux Connect to port {port} failed with code {num}"),
        ));
    }
    Ok(t)
}
