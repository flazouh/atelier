//! One request with the forge's manners: back off when it asks, retry a server that stumbles, follow
//! pages, and turn what goes wrong into a [`ForgeError`] the UI can show.
use std::{
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};

use super::transport::{Reply, Request, Transport, TransportError};
use crate::{ForgeError, ForgeResult};

/// How many times one request is tried in all.
const TRIES: u32 = 4;
/// The longest lathe waits inside one call. A longer wait is the caller's to schedule.
const LONGEST_WAIT: u64 = 60;
/// The most pages one list follows, so a forge that never ends cannot hold a call forever.
const MOST_PAGES: usize = 500;

/// A GraphQL answer: the data, with the errors that came beside it.
pub(super) struct Graph {
    pub data: Value,
    pub errors: Vec<GraphError>,
}

pub(super) struct GraphError {
    pub kind: Option<String>,
    pub message: String,
}

impl Graph {
    /// The data, when the whole query answered. An error beside the data fails the call.
    pub fn whole(self) -> ForgeResult<Value> {
        match self.errors.into_iter().next() {
            Some(error) => Err(error.into()),
            None => Ok(self.data),
        }
    }
}

impl From<GraphError> for ForgeError {
    fn from(error: GraphError) -> Self {
        match error.kind.as_deref() {
            Some("NOT_FOUND") => ForgeError::NotFound(error.message),
            Some("FORBIDDEN" | "INSUFFICIENT_SCOPES") => ForgeError::Denied(error.message),
            _ => ForgeError::Rejected(error.message),
        }
    }
}

pub(super) struct Client {
    transport: Box<dyn Transport>,
    pause: Box<dyn Fn(Duration) + Send + Sync>,
    now: Box<dyn Fn() -> u64 + Send + Sync>,
}

impl Client {
    pub fn new(transport: impl Transport + 'static) -> Self {
        Self {
            transport: Box::new(transport),
            pause: Box::new(thread::sleep),
            now: Box::new(|| SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())),
        }
    }

    /// Replaces how the client waits and tells the time, so a test does not sleep.
    #[cfg(test)]
    pub fn with_clock(
        mut self,
        pause: impl Fn(Duration) + Send + Sync + 'static,
        now: impl Fn() -> u64 + Send + Sync + 'static,
    ) -> Self {
        self.pause = Box::new(pause);
        self.now = Box::new(now);
        self
    }

    pub fn graphql(&self, query: &str, variables: Value) -> ForgeResult<Graph> {
        let body = json!({"query": query, "variables": variables}).to_string();
        let request = Request { method: "POST", path: "graphql".into(), body: Some(body) };
        // A server error on a query is tried again; on a mutation the change may have landed.
        let is_query = query.trim_start().starts_with("query");
        for attempt in 1..=TRIES {
            let reply = self.send(&request)?;
            if is_query && (500..600).contains(&reply.status) && attempt < TRIES {
                (self.pause)(Duration::from_secs(1 << (attempt - 1)));
                continue;
            }
            let parsed = serde_json::from_str::<Value>(&reply.body);
            let errors = parsed.as_ref().map(graph_errors).unwrap_or_default();
            let limited = is_limited(&reply) || errors.iter().any(|e| e.kind.as_deref() == Some("RATE_LIMITED"));
            if limited && attempt < TRIES {
                self.wait_or_give_up(&reply)?;
                continue;
            }
            if limited {
                return Err(ForgeError::RateLimited { retry_after: wait_of(&reply, (self.now)()) });
            }
            if let Some(error) = status_error(&reply, &request.path) {
                return Err(error);
            }
            let parsed = parsed.map_err(|e| ForgeError::Unexpected(format!("the answer is not JSON: {e}")))?;
            let data = parsed.get("data").cloned().unwrap_or(Value::Null);
            if data.is_null() {
                let first = errors.into_iter().next();
                return Err(first.map_or_else(|| ForgeError::Unexpected("no data in the answer".into()), Into::into));
            }
            return Ok(Graph { data, errors });
        }
        unreachable!("the last try returns")
    }

    /// A REST call. A reply with a status that is not a success is an error.
    pub fn rest(&self, method: &'static str, path: &str, body: Option<Value>) -> ForgeResult<Reply> {
        let request = Request { method, path: path.into(), body: body.map(|b| b.to_string()) };
        for attempt in 1..=TRIES {
            let reply = self.send(&request)?;
            // A server error on a change may have landed the change: only a read is tried again.
            let retryable = is_limited(&reply) || (method == "GET" && (500..600).contains(&reply.status));
            if retryable && attempt < TRIES {
                if is_limited(&reply) {
                    self.wait_or_give_up(&reply)?;
                } else {
                    (self.pause)(Duration::from_secs(1 << (attempt - 1)));
                }
                continue;
            }
            if is_limited(&reply) {
                return Err(ForgeError::RateLimited { retry_after: wait_of(&reply, (self.now)()) });
            }
            return match status_error(&reply, path) {
                Some(error) => Err(error),
                None => Ok(reply),
            };
        }
        unreachable!("the last try returns")
    }

    /// Every item of a paged GraphQL list. `page` takes one page's data and gives its items and the
    /// cursor of the next, when there is one; the query takes the cursor as `$after`.
    pub fn pages<T>(
        &self,
        query: &str,
        mut variables: Value,
        page: impl Fn(Value) -> ForgeResult<(Vec<T>, Option<String>)>,
    ) -> ForgeResult<Vec<T>> {
        let mut items = Vec::new();
        for _ in 0..MOST_PAGES {
            let data = self.graphql(query, variables.clone())?.whole()?;
            let (mut found, next) = page(data)?;
            items.append(&mut found);
            match next {
                Some(cursor) => variables["after"] = Value::String(cursor),
                None => return Ok(items),
            }
        }
        Err(ForgeError::Unexpected(format!("a list went on past {MOST_PAGES} pages")))
    }

    fn send(&self, request: &Request) -> ForgeResult<Reply> {
        self.transport.send(request).map_err(|error| match error {
            TransportError::ToolMissing => ForgeError::ToolMissing { tool: "gh".into() },
            TransportError::NotSignedIn => ForgeError::NotSignedIn,
            TransportError::Offline => ForgeError::Offline,
            TransportError::Failed(why) => ForgeError::Unexpected(why),
        })
    }

    fn wait_or_give_up(&self, reply: &Reply) -> ForgeResult<()> {
        match wait_of(reply, (self.now)()) {
            Some(seconds) if seconds <= LONGEST_WAIT => {
                (self.pause)(Duration::from_secs(seconds));
                Ok(())
            }
            long => Err(ForgeError::RateLimited { retry_after: long }),
        }
    }
}

fn graph_errors(parsed: &Value) -> Vec<GraphError> {
    let text = |value: &Value| value.as_str().map(str::to_string);
    parsed
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|error| GraphError {
            kind: error.get("type").and_then(text),
            message: error.get("message").and_then(text).unwrap_or_default(),
        })
        .collect()
}

/// Whether the forge says "slow down": a 429, or a 403 that names a rate limit or gives a wait.
fn is_limited(reply: &Reply) -> bool {
    reply.status == 429
        || (reply.status == 403
            && (reply.header("retry-after").is_some()
                || reply.header("x-ratelimit-remaining") == Some("0")
                || reply.body.to_ascii_lowercase().contains("rate limit")))
        || reply.header("x-ratelimit-remaining") == Some("0") && reply.body.contains("RATE_LIMITED")
}

/// How many seconds to wait: the forge's `retry-after`, else the time to its `x-ratelimit-reset`.
fn wait_of(reply: &Reply, now: u64) -> Option<u64> {
    if let Some(seconds) = reply.header("retry-after").and_then(|v| v.parse::<u64>().ok()) {
        return Some(seconds);
    }
    let reset = reply.header("x-ratelimit-reset")?.parse::<u64>().ok()?;
    Some(reset.saturating_sub(now) + 1)
}

/// The error a status stands for, or `None` for a success.
fn status_error(reply: &Reply, what: &str) -> Option<ForgeError> {
    if (200..300).contains(&reply.status) {
        return None;
    }
    let message = serde_json::from_str::<Value>(&reply.body)
        .ok()
        .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| format!("HTTP {}", reply.status));
    Some(match reply.status {
        401 => ForgeError::NotSignedIn,
        403 => ForgeError::Denied(message),
        404 => ForgeError::NotFound(what.to_string()),
        405 | 409 | 422 => ForgeError::Rejected(message),
        status => ForgeError::Unexpected(format!("HTTP {status}: {message}")),
    })
}

#[cfg(test)]
mod tests;
