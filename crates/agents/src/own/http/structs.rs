use std::{
    io::{self, Read},
    time::{Duration, Instant},
};

use super::super::message::Cancel;
use super::types::{HttpError, MAX_HEAD, Mode};
use super::traits::Wire;

#[derive(Clone, Debug)]
pub struct HttpOptions {
    pub connect_timeout: Duration,
    /// How long a reply may send nothing at all (the API sends a ping every few seconds).
    pub idle_timeout: Duration,
}

impl Default for HttpOptions {
    fn default() -> Self {
        Self { connect_timeout: Duration::from_secs(15), idle_timeout: Duration::from_secs(120) }
    }
}

pub struct HttpRequest {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub(super) struct Url {
    pub(super) tls: bool,
    pub(super) host: String,
    pub(super) port: u16,
    /// Path and query.
    pub(super) target: String,
}

/// The reading side: our own buffer over the wire, so a wake in the middle of a line loses nothing.
pub(super) struct Conn {
    pub(super) wire: Box<dyn Wire>,
    pub(super) buf: Vec<u8>,
    pub(super) pos: usize,
    pub(super) cancel: Cancel,
    pub(super) idle: Duration,
    pub(super) last_data: Instant,
}

impl Conn {
    /// Reads more into the buffer. `false` at the end of the stream.
    fn fill(&mut self) -> io::Result<bool> {
        if self.pos > 0 && self.pos == self.buf.len() {
            self.buf.clear();
            self.pos = 0;
        }
        let mut chunk = [0u8; 16 * 1024];
        loop {
            if self.cancel.is_set() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "stopped"));
            }
            match self.wire.read(&mut chunk) {
                Ok(0) => return Ok(false),
                Ok(n) => {
                    self.buf.extend_from_slice(&chunk[..n]);
                    self.last_data = Instant::now();
                    return Ok(true);
                }
                Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
                    if self.last_data.elapsed() > self.idle {
                        return Err(io::Error::new(io::ErrorKind::TimedOut, "the server sent nothing for too long"));
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }

    fn available(&self) -> &[u8] {
        &self.buf[self.pos..]
    }

    /// One line, without its line end. `None` at a clean end of stream before any byte.
    pub(super) fn line(&mut self) -> io::Result<Option<String>> {
        let mut scanned = 0;
        loop {
            if let Some(at) = self.available()[scanned..].iter().position(|b| *b == b'\n') {
                let end = scanned + at;
                let raw = &self.available()[..end];
                let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
                let text = String::from_utf8_lossy(raw).into_owned();
                self.pos += end + 1;
                return Ok(Some(text));
            }
            scanned = self.available().len();
            if scanned > MAX_HEAD {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "a line of the response is too long"));
            }
            if !self.fill()? {
                return if scanned == 0 { Ok(None) } else { Err(io::ErrorKind::UnexpectedEof.into()) };
            }
        }
    }
}

/// The body of a response, as a reader.
pub struct Body {
    pub(super) conn: Conn,
    pub(super) mode: Mode,
}

impl Body {
    pub(super) fn io_error(e: io::Error) -> HttpError {
        match e.kind() {
            io::ErrorKind::Interrupted => HttpError::Cancelled,
            _ => HttpError::Io(e.to_string()),
        }
    }
}

impl Read for Body {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        loop {
            match &mut self.mode {
                Mode::Done => return Ok(0),
                Mode::Length(0) => {
                    self.mode = Mode::Done;
                }
                Mode::Length(left) => {
                    let left = *left;
                    let n = self.pull(out, left as usize)?;
                    if n == 0 {
                        return Err(io::ErrorKind::UnexpectedEof.into());
                    }
                    self.mode = Mode::Length(left - n as u64);
                    return Ok(n);
                }
                Mode::UntilClose => {
                    let n = self.pull(out, usize::MAX)?;
                    if n == 0 {
                        self.mode = Mode::Done;
                    }
                    return Ok(n);
                }
                Mode::Chunked(None) => {
                    let Some(line) = self.conn.line()? else { return Err(io::ErrorKind::UnexpectedEof.into()) };
                    let size = line.split(';').next().unwrap_or("").trim();
                    if size.is_empty() {
                        continue;
                    }
                    let size = u64::from_str_radix(size, 16).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "a chunk size does not read"))?;
                    if size == 0 {
                        // Trailers, up to the blank line.
                        while let Some(line) = self.conn.line()? {
                            if line.is_empty() {
                                break;
                            }
                        }
                        self.mode = Mode::Done;
                    } else {
                        self.mode = Mode::Chunked(Some(size));
                    }
                }
                Mode::Chunked(Some(left)) => {
                    let left = *left;
                    let n = self.pull(out, left as usize)?;
                    if n == 0 {
                        return Err(io::ErrorKind::UnexpectedEof.into());
                    }
                    self.mode = if left == n as u64 {
                        // The chunk's data is followed by a line end.
                        self.conn.line()?;
                        Mode::Chunked(None)
                    } else {
                        Mode::Chunked(Some(left - n as u64))
                    };
                    return Ok(n);
                }
            }
        }
    }
}

impl Body {
    /// Copies up to `limit` bytes from the buffer into `out`, reading more first when it is empty.
    fn pull(&mut self, out: &mut [u8], limit: usize) -> io::Result<usize> {
        if self.conn.available().is_empty() && !self.conn.fill()? {
            return Ok(0);
        }
        let n = self.conn.available().len().min(out.len()).min(limit);
        out[..n].copy_from_slice(&self.conn.available()[..n]);
        self.conn.pos += n;
        Ok(n)
    }

    /// Reads at most `limit` bytes as text, for an error body.
    pub fn text(&mut self, limit: usize) -> String {
        let mut bytes = Vec::new();
        let _ = self.by_ref().take(limit as u64).read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// Ends the connection now.
    pub fn close(&self) {
        self.conn.wire.close();
    }
}

impl Drop for Body {
    fn drop(&mut self) {
        self.conn.wire.close();
    }
}

pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}
