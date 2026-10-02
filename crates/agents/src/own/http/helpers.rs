use std::{io::{self, Write}, net::{TcpStream, ToSocketAddrs}, sync::{Arc, OnceLock}, time::Instant};

use rustls::{ClientConfig, ClientConnection, StreamOwned, pki_types::ServerName};

use super::super::message::Cancel;
use super::structs::{Body, Conn, HttpOptions, HttpRequest, HttpResponse, Url};
use super::types::{HttpError, Mode, WAKE};
use super::traits::Wire;

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
