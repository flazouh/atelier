//! The wire format: each message is `Content-Length: <n>\r\n\r\n<n bytes of JSON>`.
//!
//! It is split out from the client so the framing can be tested over an in-memory pipe, with no server
//! and no process.

use std::io::{self, BufRead, Write};

/// Writes one message, headers and all.
pub fn write_message(out: &mut impl Write, body: &[u8]) -> io::Result<()> {
    write!(out, "Content-Length: {}\r\n\r\n", body.len())?;
    out.write_all(body)?;
    out.flush()
}

/// Reads one message. `Ok(None)` means the peer closed the stream cleanly.
pub fn read_message(input: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        // A header we do not know is not an error; the spec lets a server send more of them.
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let Some(length) = length else {
        return Err(new_missing_length());
    };
    let mut body = vec![0; length];
    input.read_exact(&mut body)?;
    Ok(Some(body))
}

fn new_missing_length() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "an LSP message needs a Content-Length header")
}

#[cfg(test)]
mod tests;
