//! A small HTTP server on a local port that answers Linear's GraphQL calls from a function, and writes down each
//! call it gets. It is what lets the tests run the real HTTP client, and the write paths, with no network.
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use serde_json::Value;

/// What the server answers.
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Reply {
    pub fn json(body: &Value) -> Self {
        Self {
            status: 200,
            headers: vec![],
            body: body.to_string(),
        }
    }

    pub fn status(status: u16) -> Self {
        Self {
            status,
            headers: vec![],
            body: String::new(),
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// A call the server received.
#[derive(Clone, Debug)]
pub struct Call {
    pub authorization: String,
    /// The operation name: `List`, `Get`, `Update`...
    pub operation: String,
    pub variables: Value,
}

pub struct Server {
    pub url: String,
    pub calls: Arc<Mutex<Vec<Call>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

type Handler = dyn Fn(&Call) -> Reply + Send + Sync;

impl Server {
    pub fn start(handler: impl Fn(&Call) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/graphql", listener.local_addr().unwrap());
        let calls = Arc::new(Mutex::new(vec![]));
        let stop = Arc::new(AtomicBool::new(false));
        let handler: Arc<Handler> = Arc::new(handler);
        let thread = {
            let (calls, stop) = (calls.clone(), stop.clone());
            thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::Acquire) {
                        return;
                    }
                    if let Ok(stream) = stream {
                        serve(stream, &*handler, &calls);
                    }
                }
            })
        };
        Self {
            url,
            calls,
            stop,
            thread: Some(thread),
        }
    }

    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    /// The calls of one operation, in order.
    pub fn calls_of(&self, operation: &str) -> Vec<Call> {
        self.calls()
            .into_iter()
            .filter(|c| c.operation == operation)
            .collect()
    }

    /// An address where nothing listens: a call there fails to connect.
    pub fn closed_address() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}/graphql", listener.local_addr().unwrap())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // The accept loop waits for a connection; one more lets it see the flag.
        let _ = TcpStream::connect(
            self.url
                .trim_start_matches("http://")
                .trim_end_matches("/graphql"),
        );
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(mut stream: TcpStream, handler: &Handler, calls: &Mutex<Vec<Call>>) {
    let Some((head, body)) = read_request(&mut stream) else {
        return;
    };
    let authorization = head
        .lines()
        .find_map(|l| {
            l.strip_prefix("Authorization: ")
                .or_else(|| l.strip_prefix("authorization: "))
        })
        .unwrap_or_default()
        .to_string();
    let payload: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let query = payload["query"].as_str().unwrap_or_default();
    let operation = query
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .split(['(', '{'])
        .next()
        .unwrap_or_default()
        .to_string();
    let call = Call {
        authorization,
        operation,
        variables: payload["variables"].clone(),
    };
    calls.lock().unwrap().push(call.clone());
    let reply = handler(&call);
    let mut text = format!(
        "HTTP/1.1 {} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        reply.status,
        reply.body.len()
    );
    for (name, value) in &reply.headers {
        text.push_str(&format!("{name}: {value}\r\n"));
    }
    text.push_str("\r\n");
    text.push_str(&reply.body);
    let _ = stream.write_all(text.as_bytes());
}

fn read_request(stream: &mut TcpStream) -> Option<(String, String)> {
    let mut data = vec![];
    let mut buf = [0u8; 4096];
    let split = loop {
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            return None;
        }
        data.extend_from_slice(&buf[..n]);
        if let Some(at) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
    };
    let head = String::from_utf8_lossy(&data[..split]).to_string();
    let length: usize = head
        .lines()
        .find_map(|l| {
            let (name, value) = l.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())?
        })
        .unwrap_or(0);
    while data.len() < split + length {
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
    }
    Some((head, String::from_utf8_lossy(&data[split..]).to_string()))
}
