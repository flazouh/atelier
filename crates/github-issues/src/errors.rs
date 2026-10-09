//! GitHub's answers that mean "no", as the errors the screen knows.
use std::time::{SystemTime, UNIX_EPOCH};

use atelier_capabilities::CapError;

use crate::runner::{Failure, Reply};

/// Waits this long when GitHub says to slow down and does not say for how long.
const RATE_LIMIT_DEFAULT_MS: u64 = 60_000;

pub fn from_failure(failure: Failure) -> CapError {
    match failure {
        Failure::NotSignedIn => CapError::NotSignedIn,
        Failure::Offline => CapError::Offline,
        Failure::ToolMissing => CapError::Provider {
            code: "gh_missing".into(),
            message: "the `gh` command line tool is not installed on this host".into(),
        },
        Failure::Other(message) => CapError::Provider {
            code: "gh_failed".into(),
            message,
        },
    }
}

/// The error for a reply with a status of 400 or more. `what` names the thing asked for, for a 404.
pub fn from_reply(reply: &Reply, what: &str) -> CapError {
    from_reply_at(reply, what, now_secs())
}

pub(crate) fn from_reply_at(reply: &Reply, what: &str, now_secs: u64) -> CapError {
    let body: serde_json::Value = serde_json::from_str(&reply.body).unwrap_or_default();
    let message = body["message"].as_str().unwrap_or_default();
    match reply.status {
        401 => CapError::NotSignedIn,
        404 => CapError::not_found(what),
        _ if is_rate_limit(reply, message) => CapError::RateLimited {
            retry_after_ms: retry_after_ms(reply, now_secs),
        },
        422 => CapError::invalid(
            body["errors"][0]["field"]
                .as_str()
                .unwrap_or("request")
                .to_string(),
        ),
        status => CapError::Provider {
            code: status.to_string(),
            message: if message.is_empty() {
                format!("HTTP {status}")
            } else {
                message.to_string()
            },
        },
    }
}

/// A 429, or a 403 that gives a wait, says the limit is used up, or names a rate limit.
fn is_rate_limit(reply: &Reply, message: &str) -> bool {
    reply.status == 429
        || (reply.status == 403
            && (reply.header("retry-after").is_some()
                || reply.header("x-ratelimit-remaining") == Some("0")
                || message.to_ascii_lowercase().contains("rate limit")))
}

fn retry_after_ms(reply: &Reply, now_secs: u64) -> u64 {
    let number = |name: &str| reply.header(name).and_then(|v| v.parse::<u64>().ok());
    if let Some(seconds) = number("retry-after") {
        return seconds * 1000;
    }
    match number("x-ratelimit-reset") {
        Some(reset) => reset.saturating_sub(now_secs) * 1000,
        None => RATE_LIMIT_DEFAULT_MS,
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests;
