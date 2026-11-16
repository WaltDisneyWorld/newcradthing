//! AFC (Apple File Conduit) client.
//!
//! AFC is the file-transfer protocol lockdown exposes as `com.apple.afc`
//! (jailed to the media sandbox) and, on some builds, house-arrest / crash-copy
//! variants. iCard uses it to back up and read the current Wallet card face and
//! to check for the on-device directory layout.

use std::io;

use super::transport::Transport;

const AFC_MAGIC: &[u8; 8] = b"CFA6LPAA";
const HEADER_LEN: u64 = 40;

#[allow(dead_code)]
mod op {
    pub const STATUS: u64 = 0x01;
    pub const DATA: u64 = 0x02;
    pub const READ_DIR: u64 = 0x03;
    pub const REMOVE_PATH: u64 = 0x08;
    pub const MAKE_DIR: u64 = 0x09;
    pub const GET_FILE_INFO: u64 = 0x0A;
    pub const FILE_OPEN: u64 = 0x0D;
    pub const FILE_OPEN_RES: u64 = 0x0E;
    pub const FILE_READ: u64 = 0x0F;
    pub const FILE_WRITE: u64 = 0x10;
    pub const FILE_CLOSE: u64 = 0x14;
}

#[allow(dead_code)]
pub mod mode {
    pub const RDONLY: u64 = 1;
    pub const RW: u64 = 2;
    pub const WRONLY: u64 = 3; // truncate + create
    pub const WR: u64 = 4;
    pub const APPEND: u64 = 5;
    pub const RDAPPEND: u64 = 6;
}

pub struct Afc {
    t: Box<dyn Transport>,
    packet_num: u64,
}

struct Reply {
    op: u64,
    payload: Vec<u8>,
}

impl Afc {
    pub fn new(t: Box<dyn Transport>) -> Self {
        Afc { t, packet_num: 0 }
    }

    fn send(&mut self, operation: u64, this_payload: &[u8], extra: &[u8]) -> io::Result<()> {
        let this_len = HEADER_LEN + this_payload.len() as u64;
        let entire_len = this_len + extra.len() as u64;
        let mut hdr = Vec::with_capacity(40);
        hdr.extend_from_slice(AFC_MAGIC);
        hdr.extend_from_slice(&entire_len.to_le_bytes());
        hdr.extend_from_slice(&this_len.to_le_bytes());
        hdr.extend_from_slice(&self.packet_num.to_le_bytes());
        hdr.extend_from_slice(&operation.to_le_bytes());
        self.packet_num += 1;
        self.t.write_all(&hdr)?;
        self.t.write_all(this_payload)?;
        if !extra.is_empty() {
            self.t.write_all(extra)?;
        }
        Ok(())
    }

    fn recv(&mut self) -> io::Result<Reply> {
        let mut hdr = [0u8; 40];
        self.t.read_exact(&mut hdr)?;
        if &hdr[0..8] != AFC_MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad AFC magic"));
        }
        let entire = u64::from_le_bytes(hdr[8..16].try_into().unwrap());
        let op = u64::from_le_bytes(hdr[32..40].try_into().unwrap());
        let payload_len = entire.saturating_sub(HEADER_LEN) as usize;
        let mut payload = vec![0u8; payload_len];
        self.t.read_exact(&mut payload)?;
        Ok(Reply { op, payload })
    }

    fn expect_status_ok(&mut self) -> io::Result<()> {
        let r = self.recv()?;
        if r.op == op::STATUS {
            let code = r
                .payload
                .get(0..8)
                .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
                .unwrap_or(1);
            if code == 0 {
                Ok(())
            } else {
                Err(io::Error::new(io::ErrorKind::Other, format!("AFC error status {code}")))
            }
        } else if r.op == op::DATA {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, format!("AFC unexpected op {}", r.op)))
        }
    }

    pub fn make_dir(&mut self, path: &str) -> io::Result<()> {
        self.send(op::MAKE_DIR, path.as_bytes(), &[])?;
        self.expect_status_ok()
    }

    pub fn remove_path(&mut self, path: &str) -> io::Result<()> {
        self.send(op::REMOVE_PATH, path.as_bytes(), &[])?;
        self.expect_status_ok()
    }

    /// List a directory; returns entry names (including `.` and `..`).
    #[allow(dead_code)]
    pub fn read_dir(&mut self, path: &str) -> io::Result<Vec<String>> {
        self.send(op::READ_DIR, path.as_bytes(), &[])?;
        let r = self.recv()?;
        if r.op != op::DATA {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("AFC read_dir failed for {path}")));
        }
        Ok(r.payload
            .split(|&b| b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect())
    }

    pub fn exists(&mut self, path: &str) -> bool {
        self.send(op::GET_FILE_INFO, path.as_bytes(), &[]).ok();
        matches!(self.recv(), Ok(r) if r.op == op::DATA)
    }

    fn file_open(&mut self, path: &str, open_mode: u64) -> io::Result<u64> {
        let mut p = Vec::new();
        p.extend_from_slice(&open_mode.to_le_bytes());
        p.extend_from_slice(path.as_bytes());
        p.push(0);
        self.send(op::FILE_OPEN, &p, &[])?;
        let r = self.recv()?;
        if r.op != op::FILE_OPEN_RES || r.payload.len() < 8 {
            return Err(io::Error::new(io::ErrorKind::Other, format!("AFC could not open {path}")));
        }
        Ok(u64::from_le_bytes(r.payload[0..8].try_into().unwrap()))
    }

    fn file_close(&mut self, handle: u64) -> io::Result<()> {
        self.send(op::FILE_CLOSE, &handle.to_le_bytes(), &[])?;
        self.expect_status_ok()
    }

    pub fn write_file(&mut self, path: &str, data: &[u8]) -> io::Result<()> {
        let h = self.file_open(path, mode::WRONLY)?;
        // AFC write: handle in the "this" segment, bytes as the trailing payload.
        let res = (|| {
            for chunk in data.chunks(64 * 1024) {
                self.send(op::FILE_WRITE, &h.to_le_bytes(), chunk)?;
                self.expect_status_ok()?;
            }
            Ok(())
        })();
        let close = self.file_close(h);
        res.and(close)
    }

    pub fn read_file(&mut self, path: &str) -> io::Result<Vec<u8>> {
        let h = self.file_open(path, mode::RDONLY)?;
        let mut out = Vec::new();
        let res = (|| {
            loop {
                let mut p = Vec::with_capacity(16);
                p.extend_from_slice(&h.to_le_bytes());
                p.extend_from_slice(&(64u64 * 1024).to_le_bytes());
                self.send(op::FILE_READ, &p, &[])?;
                let r = self.recv()?;
                if r.op != op::DATA || r.payload.is_empty() {
                    break;
                }
                out.extend_from_slice(&r.payload);
                if r.payload.len() < 64 * 1024 {
                    break;
                }
            }
            Ok::<_, io::Error>(())
        })();
        let close = self.file_close(h);
        res.and(close)?;
        Ok(out)
    }
}
