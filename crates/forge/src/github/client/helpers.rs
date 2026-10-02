use serde_json::Value;

use super::super::transport::Reply;
use crate::ForgeError;
use super::structs::GraphError;

pub(super) fn graph_errors(parsed: &Value) -> Vec<GraphError> {
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
pub(super) fn is_limited(reply: &Reply) -> bool {
    reply.status == 429
        || (reply.status == 403
            && (reply.header("retry-after").is_some()
                || reply.header("x-ratelimit-remaining") == Some("0")
                || reply.body.to_ascii_lowercase().contains("rate limit")))
        || reply.header("x-ratelimit-remaining") == Some("0") && reply.body.contains("RATE_LIMITED")
}

/// How many seconds to wait: the forge's `retry-after`, else the time to its `x-ratelimit-reset`.
pub(super) fn wait_of(reply: &Reply, now: u64) -> Option<u64> {
    if let Some(seconds) = reply.header("retry-after").and_then(|v| v.parse::<u64>().ok()) {
        return Some(seconds);
    }
    let reset = reply.header("x-ratelimit-reset")?.parse::<u64>().ok()?;
    Some(reset.saturating_sub(now) + 1)
}

/// The error a status stands for, or `None` for a success.
pub(super) fn status_error(reply: &Reply, what: &str) -> Option<ForgeError> {
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
