use super::{
    Reply,
    types::{Failure, NEEDS_SIGN_IN},
};

/// Why `gh` produced no reply, from its exit code and what it wrote to stderr.
pub(crate) fn classify(code: Option<i32>, stderr: &str) -> Failure {
    const OFFLINE: [&str; 7] = [
        "connection refused",
        "no such host",
        "check your internet connection",
        "i/o timeout",
        "network is unreachable",
        "tls handshake",
        "error connecting to",
    ];
    let lower = stderr.to_ascii_lowercase();
    if code == Some(NEEDS_SIGN_IN)
        || lower.contains("gh auth login")
        || lower.contains("bad credentials")
    {
        Failure::NotSignedIn
    } else if OFFLINE.iter().any(|marker| lower.contains(marker)) {
        Failure::Offline
    } else {
        let said = stderr
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("no message");
        Failure::Other(format!("gh ended with {code:?}: {said}"))
    }
}

/// `gh api --include`'s output: `HTTP/2.0 200 OK`, the headers, a blank line, then the body.
pub(crate) fn parse_reply(text: &str) -> Option<Reply> {
    let text = text.strip_prefix("HTTP/")?;
    let (head, body) = text
        .split_once("\r\n\r\n")
        .or_else(|| text.split_once("\n\n"))
        .unwrap_or((text, ""));
    let mut lines = head.lines();
    let status = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    Some(Reply {
        status,
        headers,
        body: body.to_string(),
    })
}
