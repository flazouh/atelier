use std::io::{self, Read, Write};

use super::{
    structs::{Request, Response},
    types::{BODY_MAX, HEAD_MAX, ReadError},
};

/// Reads one request from `stream`. The stream has a read timeout, so a client that stalls ends as [`ReadError::Gone`].
pub(crate) fn read_request(stream: &mut impl Read) -> Result<Request, ReadError> {
    let mut buffer = Vec::with_capacity(1024);
    let head_end = loop {
        if let Some(at) = find(&buffer, b"\r\n\r\n") {
            break at;
        }
        if buffer.len() > HEAD_MAX {
            return Err(ReadError::TooLarge);
        }
        fill(stream, &mut buffer)?;
    };
    let head = std::str::from_utf8(&buffer[..head_end]).map_err(|_| ReadError::Malformed)?;
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next().unwrap_or_default().split(' ');
    let (Some(method), Some(path), Some(version), None) = (
        request_line.next(),
        request_line.next(),
        request_line.next(),
        request_line.next(),
    ) else {
        return Err(ReadError::Malformed);
    };
    if !version.starts_with("HTTP/1.") {
        return Err(ReadError::Malformed);
    }
    let mut request = Request {
        method: method.to_string(),
        path: path.to_string(),
        ..Request::default()
    };
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(ReadError::Malformed)?;
        request
            .headers
            .push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }
    if request.header("transfer-encoding").is_some() {
        return Err(ReadError::Chunked);
    }
    let length = match request.header("content-length") {
        None => 0,
        Some(text) => text.parse::<usize>().map_err(|_| ReadError::Malformed)?,
    };
    if length > BODY_MAX {
        return Err(ReadError::TooLarge);
    }
    let mut body = buffer.split_off(head_end + 4);
    while body.len() < length {
        fill(stream, &mut body)?;
    }
    // Bytes past the length belong to a second request, which the gateway does not serve.
    body.truncate(length);
    request.body = body;
    Ok(request)
}

fn fill(stream: &mut impl Read, buffer: &mut Vec<u8>) -> Result<(), ReadError> {
    let mut chunk = [0u8; 4096];
    match stream.read(&mut chunk) {
        Ok(0) => Err(ReadError::Gone),
        Ok(n) => {
            buffer.extend_from_slice(&chunk[..n]);
            Ok(())
        }
        Err(_) => Err(ReadError::Gone),
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

pub(crate) fn write_response(stream: &mut impl Write, response: &Response) -> io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status,
        reason(response.status)
    );
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
        response.body.len()
    ));
    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Content Too Large",
        _ => "Error",
    }
}
