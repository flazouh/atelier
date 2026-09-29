//! Transports for tests and benchmarks: one replays answers, one records the real ones. Both key an
//! answer by the operation, so a test says "the `Pull` query answers with this" and never depends on
//! the order of requests.
use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use super::transport::{Reply, Request, Transport, TransportError};

/// The name an answer is kept under: the GraphQL operation (`Pull`, `Merge`), or the REST call with
/// its path made safe for a file name (`GET-repos-o-r-actions-jobs-1`).
pub fn key_of(request: &Request) -> String {
    if request.path == "graphql" {
        let query = request
            .body
            .as_deref()
            .and_then(|body| serde_json::from_str::<serde_json::Value>(body).ok())
            .and_then(|v| v["query"].as_str().map(str::to_string))
            .unwrap_or_default();
        let name = query.split_whitespace().nth(1).unwrap_or("").split(['(', '{']).next().unwrap_or("");
        return name.to_string();
    }
    let path: String = request.path.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    format!("{}-{path}", request.method)
}

#[derive(Default)]
struct State {
    replies: HashMap<String, VecDeque<Result<Reply, TransportError>>>,
    sent: Vec<Request>,
}

/// Replays queued answers. Each key answers its queue in order and starts over at the end, so a test
/// can read the same pages again.
#[derive(Clone, Default)]
pub struct Fixtures(Arc<Mutex<State>>);

impl Fixtures {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues a `200` answer with `body` for `key`.
    pub fn ok(self, key: &str, body: impl Into<String>) -> Self {
        self.answer(key, Ok(Reply { status: 200, headers: Vec::new(), body: body.into() }))
    }

    /// Queues an answer with a status and headers.
    pub fn status(self, key: &str, status: u16, headers: &[(&str, &str)], body: impl Into<String>) -> Self {
        let headers = headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        self.answer(key, Ok(Reply { status, headers, body: body.into() }))
    }

    /// Queues a request that produces no answer at all.
    pub fn fails(self, key: &str, error: TransportError) -> Self {
        self.answer(key, Err(error))
    }

    fn answer(self, key: &str, reply: Result<Reply, TransportError>) -> Self {
        self.0.lock().unwrap().replies.entry(key.to_string()).or_default().push_back(reply);
        self
    }

    /// Every `Key.json` and `Key.2.json`, `Key.3.json`... of a folder, as that key's pages in order.
    pub fn from_dir(dir: &Path) -> Self {
        // `Key.json` is page 1 and `Key.N.json` page N, so sort by the key and then the page number.
        let mut files: Vec<(String, usize, PathBuf)> = fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .map(|e| e.unwrap().path())
            .filter(|f| f.extension().is_some_and(|e| e == "json"))
            .map(|file| {
                let stem = file.file_stem().unwrap().to_string_lossy().to_string();
                match stem.rsplit_once('.').and_then(|(key, page)| Some((key, page.parse().ok()?))) {
                    Some((key, page)) => (key.to_string(), page, file),
                    None => (stem, 1, file),
                }
            })
            .collect();
        files.sort();
        let mut fixtures = Self::new();
        for (key, _, file) in files {
            fixtures = fixtures.ok(&key, fs::read_to_string(file).unwrap());
        }
        fixtures
    }

    /// The size of every queued answer, in bytes.
    pub fn bytes(&self) -> usize {
        let state = self.0.lock().unwrap();
        state.replies.values().flatten().filter_map(|r| r.as_ref().ok()).map(|r| r.body.len()).sum()
    }

    /// The requests sent so far.
    pub fn sent(&self) -> Vec<Request> {
        self.0.lock().unwrap().sent.clone()
    }

    /// The requests sent for one key.
    pub fn sent_for(&self, key: &str) -> Vec<Request> {
        self.sent().into_iter().filter(|r| key_of(r) == key).collect()
    }
}

impl Transport for Fixtures {
    fn send(&self, request: &Request) -> Result<Reply, TransportError> {
        let mut state = self.0.lock().unwrap();
        state.sent.push(request.clone());
        let key = key_of(request);
        let queue = state.replies.get_mut(&key).unwrap_or_else(|| panic!("no fixture for `{key}`"));
        let reply = queue.pop_front().unwrap();
        queue.push_back(reply.clone());
        reply
    }
}

/// Passes requests to a real transport and writes each answer's body into a folder, as the fixtures
/// [`Fixtures::from_dir`] reads. Recording is read-only by construction: it refuses a request that is
/// not a query or a GET.
pub struct Recording<T> {
    inner: T,
    dir: PathBuf,
    seen: Mutex<HashMap<String, usize>>,
}

impl<T> Recording<T> {
    pub fn new(inner: T, dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        fs::create_dir_all(&dir).expect("the fixture folder can be made");
        Self { inner, dir, seen: Mutex::default() }
    }
}

impl<T: Transport> Transport for Recording<T> {
    fn send(&self, request: &Request) -> Result<Reply, TransportError> {
        let is_query = request.body.as_deref().is_some_and(|b| b.contains("\"query\":\"query"));
        assert!(request.method == "GET" || is_query, "recording never sends a change: {request:?}");
        let reply = self.inner.send(request)?;
        let key = key_of(request);
        let n = {
            let mut seen = self.seen.lock().unwrap();
            let n = seen.entry(key.clone()).or_default();
            *n += 1;
            *n
        };
        let name = if n == 1 { format!("{key}.json") } else { format!("{key}.{n}.json") };
        fs::write(self.dir.join(name), &reply.body).expect("the fixture is written");
        Ok(reply)
    }
}
