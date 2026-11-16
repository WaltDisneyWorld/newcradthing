//! A byte-stream transport abstraction so the AFC / syslog / lockdown clients can
//! run over either a plain usbmux connection or an Apple-secured service pipe.

use std::io::{self, Read, Write};
use std::time::Duration;

pub trait Transport: Send {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()>;
    fn set_read_timeout(&mut self, _dur: Option<Duration>) -> io::Result<()> {
        Ok(())
    }

    fn read_exact(&mut self, mut buf: &mut [u8]) -> io::Result<()> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "connection closed by device",
                    ))
                }
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

/// Transport backed by a std TCP stream (usbmux on Windows listens on
/// 127.0.0.1:27015; a raw device-port Connect turns the same socket into a pipe
/// to the service on the phone).
pub struct TcpTransport(pub std::net::TcpStream);

impl Transport for TcpTransport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        Read::read(&mut self.0, buf)
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        Write::write_all(&mut self.0, buf)
    }
    fn set_read_timeout(&mut self, dur: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(dur)
    }
}

#[cfg(unix)]
pub struct UnixTransport(pub std::os::unix::net::UnixStream);

#[cfg(unix)]
impl Transport for UnixTransport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        Read::read(&mut self.0, buf)
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        Write::write_all(&mut self.0, buf)
    }
    fn set_read_timeout(&mut self, dur: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(dur)
    }
}
