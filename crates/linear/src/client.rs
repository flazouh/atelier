use std::time::{Duration, SystemTime, UNIX_EPOCH};

use atelier_capabilities::{CapError, CapResult};
use serde_json::{Value, json};

/// A blocking GraphQL client for one Linear endpoint. It holds the API key and shows it to nobody: the key is
/// not in `Debug`, not in an error, and not in a log.
pub struct Client {
    agent: ureq::Agent,
    endpoint: String,
    authorization: String,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

/// How long a rate limited call waits when Linear does not say.
const DEFAULT_RETRY_MS: u64 = 60_000;
/// How long a connection may take to open before the next address is tried.
const CONNECT_SECONDS: u64 = 3;

impl Client {
    pub fn new(endpoint: &str, api_key: &str) -> Self {
        // Linear takes a personal key bare, and an OAuth token as a bearer.
        let authorization = if api_key.starts_with("lin_oauth_") {
            format!("Bearer {api_key}")
        } else {
            api_key.to_string()
        };
        // A 4xx or 5xx is not an error for ureq here: the status and the body tell us which CapError it is.
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            // A host with no IPv6 waits for the first address before it tries the next: a short connect timeout makes that
            // fallback quick.
            .timeout_connect(Some(Duration::from_secs(CONNECT_SECONDS)))
            .build()
            .into();
        Self {
            agent,
            endpoint: endpoint.to_string(),
            authorization,
        }
    }

    /// Runs one GraphQL document and gives back its `data`.
    pub fn run(&self, query: &str, variables: Value) -> CapResult<Value> {
        let body = json!({ "query": query, "variables": variables }).to_string();
        let mut response = self
            .agent
            .post(&self.endpoint)
            .header("Authorization", &self.authorization)
            .header("Content-Type", "application/json")
            .send(body)
            .map_err(offline_or_provider)?;
        let status = response.status().as_u16();
        let retry_after = retry_after_ms(&response);
        let text = response.body_mut().read_to_string().unwrap_or_default();
        match status {
            401 => return Err(CapError::NotSignedIn),
            429 => {
                return Err(CapError::RateLimited {
                    retry_after_ms: retry_after,
                });
            }
            _ => {}
        }
        let parsed: Value = serde_json::from_str(&text).map_err(|_| CapError::Provider {
            code: format!("http_{status}"),
            message: "Linear answered with something that is not JSON".into(),
        })?;
        if let Some(errors) = parsed.get("errors").and_then(Value::as_array)
            && let Some(first) = errors.first()
        {
            return Err(graphql_error(first, retry_after));
        }
        if status >= 400 {
            return Err(CapError::Provider {
                code: format!("http_{status}"),
                message: "Linear refused the call".into(),
            });
        }
        parsed.get("data").cloned().ok_or(CapError::Provider {
            code: "no_data".into(),
            message: "Linear answered without data".into(),
        })
    }
}

fn offline_or_provider(error: ureq::Error) -> CapError {
    match error {
        ureq::Error::Io(_)
        | ureq::Error::Timeout(_)
        | ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed => CapError::Offline,
        // The text of a ureq error names the URL, never a header, so it holds no key.
        other => CapError::Provider {
            code: "http".into(),
            message: other.to_string(),
        },
    }
}

/// Linear says how long to wait in `Retry-After` (seconds) or in `X-RateLimit-Requests-Reset` (epoch milliseconds).
fn retry_after_ms(response: &ureq::http::Response<ureq::Body>) -> u64 {
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
    };
    if let Some(seconds) = header("retry-after") {
        return seconds.saturating_mul(1000);
    }
    if let Some(reset) = header("x-ratelimit-requests-reset") {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        return reset.saturating_sub(now).max(1000);
    }
    DEFAULT_RETRY_MS
}

/// A GraphQL error is `Provider` with Linear's own code, except for the few that mean something to the screen.
fn graphql_error(error: &Value, retry_after: u64) -> CapError {
    let code = error
        .pointer("/extensions/code")
        .and_then(Value::as_str)
        .unwrap_or("graphql")
        .to_string();
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Linear returned an error")
        .to_string();
    match code.as_str() {
        "AUTHENTICATION_ERROR" => CapError::NotSignedIn,
        "RATELIMITED" => CapError::RateLimited {
            retry_after_ms: retry_after,
        },
        // A missing issue comes back as an input error that says so; the code alone does not tell.
        "ENTITY_NOT_FOUND" => CapError::NotFound { what: message },
        "INPUT_ERROR" if message.starts_with("Entity not found") => {
            CapError::NotFound { what: message }
        }
        _ => CapError::Provider { code, message },
    }
}
