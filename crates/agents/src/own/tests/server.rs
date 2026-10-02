//! A local HTTP server that plays scripted answers, so the model clients are tested with no network and no
//! key. One answer per request, in order. It records each request.
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

pub type Sse = Vec<(String, String)>;

pub enum Step {
    /// 200 and these events, one HTTP chunk each.
    Sse(Sse),
    /// 200 and these bytes as the body, cut into chunks of this size.
    Raw { bytes: Vec<u8>, chunk: usize },
    Status { code: u16, headers: Vec<(&'static str, String)>, body: String },
    /// 200 and these events, then nothing until the client hangs up (or ten seconds).
    Hang(Sse),
}

#[derive(Clone, Debug)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

pub struct FakeServer {
    pub url: String,
    pub requests: Arc<Mutex<Vec<Recorded>>>,
    /// Set when the client closed a `Hang` connection.
    pub hung_up: Arc<AtomicBool>,
}

fn read_request(stream: &mut TcpStream) -> Option<Recorded> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.lines();
    let mut first = lines.next()?.split_whitespace();
    let (method, path) = (first.next()?.to_string(), first.next()?.to_string());
    let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    let length = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.parse::<usize>().ok()).unwrap_or(0);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    Some(Recorded { method, path, headers, body: serde_json::from_slice(&body).unwrap_or(Value::Null) })
}

fn chunked(stream: &mut TcpStream, bytes: &[u8]) -> std::io::Result<()> {
    write!(stream, "{:x}\r\n", bytes.len())?;
    stream.write_all(bytes)?;
    stream.write_all(b"\r\n")?;
    stream.flush()
}

fn sse_bytes(events: &Sse) -> Vec<Vec<u8>> {
    events.iter().map(|(name, data)| format!("event: {name}\ndata: {data}\n\n").into_bytes()).collect()
}

const SSE_HEAD: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";

impl FakeServer {
    pub fn start(steps: Vec<Step>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let hung_up = Arc::new(AtomicBool::new(false));
        let (log, hung) = (requests.clone(), hung_up.clone());
        let steps = Arc::new(Mutex::new(std::collections::VecDeque::from(steps)));
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let (log, hung, steps) = (log.clone(), hung.clone(), steps.clone());
                thread::spawn(move || {
                    let Some(request) = read_request(&mut stream) else { return };
                    log.lock().unwrap().push(request);
                    let step = steps.lock().unwrap().pop_front();
                    let _ = play(&mut stream, step, &hung);
                });
            }
        });
        Self { url, requests, hung_up }
    }

    pub fn recorded(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }
}

fn play(stream: &mut TcpStream, step: Option<Step>, hung: &AtomicBool) -> std::io::Result<()> {
    match step {
        None => stream.write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 20\r\nConnection: close\r\n\r\nno more scripted steps"[..].as_ref()),
        Some(Step::Status { code, headers, body }) => {
            let mut head = format!("HTTP/1.1 {code} Status\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
            for (k, v) in headers {
                head.push_str(&format!("{k}: {v}\r\n"));
            }
            head.push_str("\r\n");
            stream.write_all(head.as_bytes())?;
            stream.write_all(body.as_bytes())
        }
        Some(Step::Sse(events)) => {
            stream.write_all(SSE_HEAD.as_bytes())?;
            for piece in sse_bytes(&events) {
                chunked(stream, &piece)?;
            }
            stream.write_all(b"0\r\n\r\n")
        }
        Some(Step::Raw { bytes, chunk }) => {
            stream.write_all(SSE_HEAD.as_bytes())?;
            for piece in bytes.chunks(chunk.max(1)) {
                chunked(stream, piece)?;
            }
            stream.write_all(b"0\r\n\r\n")
        }
        Some(Step::Hang(events)) => {
            stream.write_all(SSE_HEAD.as_bytes())?;
            for piece in sse_bytes(&events) {
                chunked(stream, &piece)?;
            }
            // Wait for the client to close its side.
            stream.set_read_timeout(Some(Duration::from_millis(50)))?;
            let start = Instant::now();
            let mut sink = [0u8; 16];
            while start.elapsed() < Duration::from_secs(10) {
                match stream.read(&mut sink) {
                    Ok(0) => {
                        hung.store(true, Ordering::SeqCst);
                        return Ok(());
                    }
                    Ok(_) => {}
                    Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                        // A write to a closed socket fails too: try one.
                        if stream.write_all(b"").is_err() {
                            hung.store(true, Ordering::SeqCst);
                            return Ok(());
                        }
                    }
                    Err(_) => {
                        hung.store(true, Ordering::SeqCst);
                        return Ok(());
                    }
                }
            }
            Ok(())
        }
    }
}

// ---- The events of Anthropic's stream, as the API writes them. ----

fn ev(name: &str, data: Value) -> (String, String) {
    (name.to_string(), data.to_string())
}

pub fn start(input_tokens: u64) -> (String, String) {
    ev("message_start", json!({"type": "message_start", "message": {"id": "msg_1", "type": "message", "role": "assistant", "content": [], "model": "claude-test", "usage": {"input_tokens": input_tokens, "cache_read_input_tokens": 5, "cache_creation_input_tokens": 7, "output_tokens": 1}}}))
}

pub fn text_block(index: usize, pieces: &[&str]) -> Sse {
    let mut out = vec![ev("content_block_start", json!({"type": "content_block_start", "index": index, "content_block": {"type": "text", "text": ""}}))];
    for piece in pieces {
        out.push(ev("content_block_delta", json!({"type": "content_block_delta", "index": index, "delta": {"type": "text_delta", "text": piece}})));
    }
    out.push(ev("content_block_stop", json!({"type": "content_block_stop", "index": index})));
    out
}

pub fn thinking_block(index: usize, pieces: &[&str], signature: &str) -> Sse {
    let mut out = vec![ev("content_block_start", json!({"type": "content_block_start", "index": index, "content_block": {"type": "thinking", "thinking": ""}}))];
    for piece in pieces {
        out.push(ev("content_block_delta", json!({"type": "content_block_delta", "index": index, "delta": {"type": "thinking_delta", "thinking": piece}})));
    }
    out.push(ev("content_block_delta", json!({"type": "content_block_delta", "index": index, "delta": {"type": "signature_delta", "signature": signature}})));
    out.push(ev("content_block_stop", json!({"type": "content_block_stop", "index": index})));
    out
}

pub fn tool_block(index: usize, id: &str, name: &str, input: &Value) -> Sse {
    let json = input.to_string();
    let (a, b) = json.split_at(json.len() / 2);
    let mut cut = json.len() / 2;
    while !json.is_char_boundary(cut) {
        cut += 1;
    }
    let _ = (a, b);
    let (a, b) = json.split_at(cut);
    vec![
        ev("content_block_start", json!({"type": "content_block_start", "index": index, "content_block": {"type": "tool_use", "id": id, "name": name, "input": {}}})),
        ev("content_block_delta", json!({"type": "content_block_delta", "index": index, "delta": {"type": "input_json_delta", "partial_json": a}})),
        ev("content_block_delta", json!({"type": "content_block_delta", "index": index, "delta": {"type": "input_json_delta", "partial_json": b}})),
        ev("content_block_stop", json!({"type": "content_block_stop", "index": index})),
    ]
}

/// A tool call whose input streams in the pieces given, in that order.
pub fn tool_pieces(index: usize, id: &str, name: &str, pieces: &[&str]) -> Sse {
    let mut out = vec![ev("content_block_start", json!({"type": "content_block_start", "index": index, "content_block": {"type": "tool_use", "id": id, "name": name, "input": {}}}))];
    out.extend(pieces.iter().map(|piece| ev("content_block_delta", json!({"type": "content_block_delta", "index": index, "delta": {"type": "input_json_delta", "partial_json": piece}}))));
    out.push(ev("content_block_stop", json!({"type": "content_block_stop", "index": index})));
    out
}

pub fn end(stop_reason: &str, output_tokens: u64) -> Sse {
    vec![
        ev("message_delta", json!({"type": "message_delta", "delta": {"stop_reason": stop_reason, "stop_sequence": null}, "usage": {"output_tokens": output_tokens}})),
        ev("message_stop", json!({"type": "message_stop"})),
    ]
}

/// A reply of these blocks, ending as `stop_reason` says.
pub fn reply(blocks: Vec<Sse>, stop_reason: &str) -> Sse {
    let mut out = vec![start(100), ev("ping", json!({"type": "ping"}))];
    for block in blocks {
        out.extend(block);
    }
    out.extend(end(stop_reason, 42));
    out
}

pub fn says(text: &str) -> Sse {
    reply(vec![text_block(0, &[text])], "end_turn")
}

pub fn uses(calls: &[(&str, &str, Value)]) -> Sse {
    reply(calls.iter().enumerate().map(|(i, (id, name, input))| tool_block(i, id, name, input)).collect(), "tool_use")
}

pub fn error_body(kind: &str, message: &str) -> String {
    json!({"type": "error", "error": {"type": kind, "message": message}}).to_string()
}
