use atelier_capabilities::CapError;

use crate::types::{RATE_LIMIT_WAIT_MS, RunFailure};

/// Turns what a failed run printed into the error the screen knows. `gmailcli` writes one line, `error: <sentence>`; the
/// sentences it knows are `the browser is not signed in to Gmail`, `Gmail's page layout did not match` and `ego-browser is not
/// installed`. A line from `ssh` (no route, no name) means there is no connection.
pub(crate) fn map_failure(failure: &RunFailure) -> CapError {
    let text = failure.text();
    let low = text.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| low.contains(n));
    if has(&["not signed in", "accounts.google.com"]) {
        return CapError::NotSignedIn;
    }
    if has(&[
        "rate limit",
        "too many requests",
        "unusual traffic",
        "429",
        "quota",
    ]) {
        return CapError::RateLimited {
            retry_after_ms: RATE_LIMIT_WAIT_MS,
        };
    }
    if has(&[
        "could not resolve host",
        "name or service not known",
        "no route to host",
        "network is unreachable",
        "connection timed out",
        "connection refused",
        "err_internet_disconnected",
        "err_name_not_resolved",
    ]) {
        return CapError::Offline;
    }
    if has(&["page layout did not match"]) {
        return provider(
            "layout_changed",
            "Gmail changed its page; gmailcli needs an update",
        );
    }
    if matches!(failure, RunFailure::Spawn(_)) || has(&["not installed", "command not found"]) {
        return provider("not_installed", &sentence(&text));
    }
    provider("gmailcli", &sentence(&text))
}

fn provider(code: &str, message: &str) -> CapError {
    CapError::Provider {
        code: code.into(),
        message: message.into(),
    }
}

/// The first line that says something, without the `error:` word, cut to 300 characters.
fn sentence(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("gmailcli failed with no message");
    let line = line.strip_prefix("error:").unwrap_or(line).trim();
    line.chars().take(300).collect()
}
