use std::{
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};

use super::super::transport::{Reply, Request, Transport, TransportError};
use crate::{ForgeError, ForgeResult};
use super::types::{LONGEST_WAIT, MOST_PAGES, TRIES};
use super::helpers::{graph_errors, is_limited, status_error, wait_of};

/// A GraphQL answer: the data, with the errors that came beside it.
pub(in super::super) struct Graph {
    pub data: Value,
    pub errors: Vec<GraphError>,
}

pub(in super::super) struct GraphError {
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

pub(in super::super) struct Client {
    pub(super) transport: Box<dyn Transport>,
    pub(super) pause: Box<dyn Fn(Duration) + Send + Sync>,
    pub(super) now: Box<dyn Fn() -> u64 + Send + Sync>,
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

    pub(super) fn send(&self, request: &Request) -> ForgeResult<Reply> {
        self.transport.send(request).map_err(|error| match error {
            TransportError::ToolMissing => ForgeError::ToolMissing { tool: "gh".into() },
            TransportError::NotSignedIn => ForgeError::NotSignedIn,
            TransportError::Offline => ForgeError::Offline,
            TransportError::Failed(why) => ForgeError::Unexpected(why),
        })
    }

    pub(super) fn wait_or_give_up(&self, reply: &Reply) -> ForgeResult<()> {
        match wait_of(reply, (self.now)()) {
            Some(seconds) if seconds <= LONGEST_WAIT => {
                (self.pause)(Duration::from_secs(seconds));
                Ok(())
            }
            long => Err(ForgeError::RateLimited { retry_after: long }),
        }
    }
}
