//! A small HTTP/1.1 client for streaming replies: one request per connection (`Connection: close`),
//! plain or TLS, with the response body read in pieces as it arrives.
//!
//! It is here, not a library, for one reason: a reply can stall for a long time (a model thinks), and the
//! caller must be able to stop it at once. Every wait in this file wakes ten times a second to look at a
//! [`Cancel`] flag, and all parsing works on our own buffer, so a wake never leaves a half-read line.
//! No header of a request appears in an error: a key in `x-api-key` never reaches a log.
use std::{
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use rustls::{ClientConfig, ClientConnection, StreamOwned, pki_types::ServerName};

use super::message::Cancel;

/// How often a wait looks at the cancel flag.
const WAKE: Duration = Duration::from_millis(100);
const MAX_HEAD: usize = 64 * 1024;

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

#[derive(Debug, PartialEq, Eq)]
pub enum HttpError {
    Cancelled,
    /// The request could not be made: a bad URL, no route, a refused connection, a TLS failure.
    Connect(String),
    /// The connection broke or went silent.
    Io(String),
    /// The server's answer is not HTTP.
    Protocol(String),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("stopped"),
            Self::Connect(why) | Self::Io(why) | Self::Protocol(why) => f.write_str(why),
        }
    }
}

trait Wire: Read + Write + Send {
    fn wake_after(&self, timeout: Duration);
    fn close(&self);
}

impl Wire for TcpStream {
    fn wake_after(&self, timeout: Duration) {
        let _ = self.set_read_timeout(Some(timeout));
    }

    fn close(&self) {
        let _ = self.shutdown(std::net::Shutdown::Both);
    }
}

impl Wire for StreamOwned<ClientConnection, TcpStream> {
    fn wake_after(&self, timeout: Duration) {
        let _ = self.sock.set_read_timeout(Some(timeout));
    }

    fn close(&self) {
        let _ = self.sock.shutdown(std::net::Shutdown::Both);
    }
}

struct Url {
    tls: bool,
    host: String,
    port: u16,
    /// Path and query.
    target: String,
}

fn parse_url(url: &str) -> Result<Url, HttpError> {
    let bad = || HttpError::Connect(format!("not a usable URL: {}", url.split('?').next().unwrap_or("")));
    let (tls, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        (false, rest)
    } else {
        return Err(bad());
    };
    let (authority, target) = match rest.find(['/', '?']) {
        Some(at) if rest.as_bytes()[at] == b'/' => (&rest[..at], rest[at..].to_string()),
        Some(at) => (&rest[..at], format!("/{}", &rest[at..])),
        None => (rest, "/".to_string()),
    };
    if authority.contains('@') || authority.is_empty() {
        return Err(bad());
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.ends_with(']') || host.starts_with('[') => (host, port.parse::<u16>().map_err(|_| bad())?),
        _ => (authority, if tls { 443 } else { 80 }),
    };
    Ok(Url { tls, host: host.trim_matches(['[', ']']).to_string(), port, target })
}

fn tls_config() -> Result<Arc<ClientConfig>, HttpError> {
    static CONFIG: OnceLock<Result<Arc<ClientConfig>, String>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut roots = rustls::RootCertStore::empty();
            let loaded = rustls_native_certs::load_native_certs();
            let (added, _) = roots.add_parsable_certificates(loaded.certs);
            if added == 0 {
                return Err("no trusted root certificates on this machine".to_string());
            }
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let config = ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .map_err(|e| e.to_string())?
                .with_root_certificates(roots)
                .with_no_client_auth();
            Ok(Arc::new(config))
        })
        .clone()
        .map_err(HttpError::Connect)
}

fn connect(url: &Url, options: &HttpOptions) -> Result<Box<dyn Wire>, HttpError> {
    let fail = |why: String| HttpError::Connect(why);
    let addrs = (url.host.as_str(), url.port).to_socket_addrs().map_err(|e| fail(format!("cannot find {}: {e}", url.host)))?;
    let mut last = None;
    let mut stream = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, options.connect_timeout) {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(e) => last = Some(e),
        }
    }
    let stream = stream.ok_or_else(|| fail(format!("cannot connect to {}: {}", url.host, last.map_or("no address".into(), |e| e.to_string()))))?;
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(options.idle_timeout));
    if !url.tls {
        return Ok(Box::new(stream));
    }
    let name = ServerName::try_from(url.host.clone()).map_err(|_| fail(format!("{} is not a host name", url.host)))?;
    let connection = ClientConnection::new(tls_config()?, name).map_err(|e| fail(format!("TLS failed: {e}")))?;
    Ok(Box::new(StreamOwned::new(connection, stream)))
}

/// The reading side: our own buffer over the wire, so a wake in the middle of a line loses nothing.
struct Conn {
    wire: Box<dyn Wire>,
    buf: Vec<u8>,
    pos: usize,
    cancel: Cancel,
    idle: Duration,
    last_data: Instant,
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
    fn line(&mut self) -> io::Result<Option<String>> {
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

enum Mode {
    Length(u64),
    /// Bytes left in the current chunk, or `None` between chunks.
    Chunked(Option<u64>),
    UntilClose,
    Done,
}

/// The body of a response, as a reader.
pub struct Body {
    conn: Conn,
    mode: Mode,
}

impl Body {
    fn io_error(e: io::Error) -> HttpError {
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

/// Sends the request and reads the response head. The body is read from `response.body`.
pub fn send(request: &HttpRequest, cancel: &Cancel, options: &HttpOptions) -> Result<HttpResponse, HttpError> {
    if cancel.is_set() {
        return Err(HttpError::Cancelled);
    }
    let url = parse_url(&request.url)?;
    let mut wire = connect(&url, options)?;
    let mut head = format!("{} {} HTTP/1.1\r\nHost: {}", request.method, url.target, url.host);
    if url.port != if url.tls { 443 } else { 80 } {
        head.push_str(&format!(":{}", url.port));
    }
    head.push_str("\r\nConnection: close\r\nAccept: */*\r\n");
    for (name, value) in &request.headers {
        if name.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
            return Err(HttpError::Connect("a header holds a line break".into()));
        }
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\n\r\n", request.body.len()));
    wire.write_all(head.as_bytes()).and_then(|()| wire.write_all(&request.body)).and_then(|()| wire.flush()).map_err(|e| HttpError::Io(format!("the request was not sent: {e}")))?;
    wire.wake_after(WAKE);
    let mut conn = Conn { wire, buf: Vec::new(), pos: 0, cancel: cancel.clone(), idle: options.idle_timeout, last_data: Instant::now() };
    let status_line = conn.line().map_err(Body::io_error)?.ok_or_else(|| HttpError::Io("the server closed the connection".into()))?;
    let mut parts = status_line.split_whitespace();
    let (Some(version), Some(status)) = (parts.next(), parts.next()) else { return Err(HttpError::Protocol("the reply is not HTTP".into())) };
    if !version.starts_with("HTTP/1.") {
        return Err(HttpError::Protocol("the reply is not HTTP/1".into()));
    }
    let status: u16 = status.parse().map_err(|_| HttpError::Protocol("the status does not read".into()))?;
    let mut headers = Vec::new();
    loop {
        let line = conn.line().map_err(Body::io_error)?.ok_or_else(|| HttpError::Io("the reply ended in its headers".into()))?;
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    let find = |name: &str| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.to_ascii_lowercase());
    let mode = if find("transfer-encoding").is_some_and(|v| v.contains("chunked")) {
        Mode::Chunked(None)
    } else if let Some(length) = find("content-length").and_then(|v| v.parse::<u64>().ok()) {
        Mode::Length(length)
    } else if status == 204 || status == 304 || (100..200).contains(&status) {
        Mode::Length(0)
    } else {
        Mode::UntilClose
    };
    Ok(HttpResponse { status, headers, body: Body { conn, mode } })
}

pub fn map_read_error(e: &io::Error) -> HttpError {
    Body::io_error(io::Error::new(e.kind(), e.to_string()))
}
