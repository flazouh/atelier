use std::{
    io::{Write},
    path::{Path, PathBuf},
    sync::{mpsc::{Sender}},
};

use lsp_types::Uri;
use serde_json::{Value, json};

use crate::framing::{read_message, write_message};
use super::types::{LspError, METHOD_NOT_FOUND, Pending, Routed, ServerMessage, Writer};

/// Writes one message whole, so the client's requests and the read loop's answers never interleave.
pub(super) fn send_on(writer: &Writer<impl Write>, body: &Value) -> Result<(), LspError> {
    let bytes = serde_json::to_vec(body).map_err(|e| LspError::Protocol(e.to_string()))?;
    let mut out = writer.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    write_message(&mut *out, &bytes).map_err(LspError::Transport)?;
    out.flush().map_err(LspError::Transport)
}

/// Reads every message until the server closes, routing replies to the waiters and answering the
/// server's own requests on the spot. Some servers (tsgo) answer nothing until they hear back, while
/// the worker may be blocked waiting on one of those answers, so the answer cannot wait for the worker.
pub(super) fn read_loop(
    mut input: impl std::io::BufRead,
    pending: Pending,
    out: Sender<ServerMessage>,
    writer: Writer<impl Write>,
    root: PathBuf,
) {
    while let Ok(Some(body)) = read_message(&mut input) {
        let Ok(message) = serde_json::from_slice::<Value>(&body) else { continue };
        match classify(&message) {
            Routed::Request { id, method, params } => {
                let body = match answer_for(&method, &params, &root) {
                    Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                    Err((code, message)) => {
                        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
                    }
                };
                // A failed write means the server is gone, which the end of this loop reports.
                let _ = send_on(&writer, &body);
                if out.send(ServerMessage::Log(format!("answered the server's {method}"))).is_err() {
                    break;
                }
            }
            Routed::Reply(id, result) => {
                if let Some(waiter) = pending.lock().expect("the pending map is not poisoned").remove(&id) {
                    let _ = waiter.send(result);
                }
            }
            Routed::Server(message) => {
                if out.send(message).is_err() {
                    break;
                }
            }
            Routed::Ignore => {}
        }
    }
    let _ = out.send(ServerMessage::Exited);
}

/// Sorts one message. Pulled out of the read loop so it can be tested without a server.
pub(super) fn classify(message: &Value) -> Routed {
    // A server request carries an id and a method; only a reply has no method. The server picks the
    // id, and it may be a string (tsgo's are "ts1", "ts2", ...), so it is sent back as it came.
    if let (Some(id), Some(method)) = (message.get("id"), message.get("method").and_then(Value::as_str)) {
        return Routed::Request {
            id: id.clone(),
            method: method.to_string(),
            params: message.get("params").cloned().unwrap_or(Value::Null),
        };
    }
    // Replies answer atelier's own requests, which it numbers.
    if let Some(id) = message.get("id").and_then(Value::as_i64) {
        if let Some(error) = message.get("error") {
            let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
            let text = error.get("message").and_then(Value::as_str).unwrap_or("no reason given");
            return Routed::Reply(id, Err(LspError::Server { code, message: text.to_string() }));
        }
        let result = message.get("result").cloned().unwrap_or(Value::Null);
        return Routed::Reply(id, Ok(result));
    }
    match message.get("method").and_then(Value::as_str) {
        Some("textDocument/publishDiagnostics") => {
            match serde_json::from_value(message.get("params").cloned().unwrap_or(Value::Null)) {
                Ok(params) => Routed::Server(ServerMessage::Diagnostics(params)),
                Err(_) => Routed::Ignore,
            }
        }
        Some("experimental/serverStatus") => {
            match message.get("params").and_then(|p| p.get("quiescent")).and_then(Value::as_bool) {
                Some(quiescent) => Routed::Server(ServerMessage::Status { quiescent }),
                None => Routed::Ignore,
            }
        }
        Some("window/logMessage") | Some("window/showMessage") => {
            let text = message
                .get("params")
                .and_then(|p| p.get("message"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            Routed::Server(ServerMessage::Log(text))
        }
        _ => Routed::Ignore,
    }
}

/// A file path as the `file://` URI every server expects.
pub fn path_to_uri(path: &Path) -> Result<Uri, LspError> {
    let path = path.canonicalize().map_err(LspError::Transport)?;
    let text = path.to_str().ok_or_else(|| LspError::Protocol("a path that is not UTF-8".into()))?;
    let mut encoded = String::from("file://");
    for byte in text.bytes() {
        match byte {
            // Unreserved, RFC 3986 section 2.3.
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~'
            // Sub-delims, plus the two extra path characters, RFC 3986 section 3.3. A server built
            // on Rust's `url` crate leaves every one of these raw. We compare URIs byte for byte,
            // so escaping one here means the server's diagnostics never match the file we asked
            // about: a path holding `+` waited out the full timeout while the answer sat unread.
            | b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'=' | b':'
            | b'@' | b'/' => encoded.push(byte as char),
            // Everything else: space, `#`, `?`, `%`, the control bytes, and every non-ASCII byte.
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded.parse().map_err(|_| LspError::Protocol(format!("a path that is not a URI: {text}")))
}

/// The file a `file://` URI names, with its percent-escapes decoded. `None` for any other scheme,
/// such as a server's virtual documents. Two servers may escape the same path differently (`@` or
/// `%40`), so paths, never URI strings, are what the worker compares.
pub fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    if uri.scheme().map(|s| s.as_str()) != Some("file") {
        return None;
    }
    let bytes = uri.path().as_str().as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok().map(PathBuf::from)
}

/// What atelier answers to a request a server sends. A server that asks for settings gets none, so it
/// uses its defaults; one that registers a capability or a progress token is told yes; one that asks
/// for the workspace folders gets the one root. Anything else is not implemented, which a server must
/// accept rather than hang on an answer that never comes.
pub fn answer_for(method: &str, params: &Value, root: &Path) -> Result<Value, (i64, String)> {
    match method {
        "workspace/configuration" => {
            let items = params.get("items").and_then(Value::as_array).map_or(0, Vec::len);
            Ok(Value::Array(vec![Value::Null; items]))
        }
        "client/registerCapability" | "client/unregisterCapability" | "window/workDoneProgress/create" => {
            Ok(Value::Null)
        }
        "workspace/workspaceFolders" => {
            let uri = path_to_uri(root).map_err(|e| (-32603, e.to_string()))?;
            let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            Ok(json!([{ "uri": uri.as_str(), "name": name }]))
        }
        other => Err((METHOD_NOT_FOUND, format!("atelier does not implement {other}"))),
    }
}
