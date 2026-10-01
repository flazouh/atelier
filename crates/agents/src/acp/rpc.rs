//! JSON-RPC 2.0, one message a line, as ACP frames it. Both sides send requests: atelier asks the agent to
//! start a session or run a prompt, and the agent asks atelier for a permission. An id is a number or a
//! string; atelier numbers its own and gives the agent's back as it came.
use std::fmt;

use serde::Deserialize;
use serde_json::{Value, json};

/// The code an agent returns for a method it does not have, and atelier for one it does not serve.
pub(super) const METHOD_NOT_FOUND: i64 = -32601;

/// One line from the agent.
#[derive(Debug, PartialEq)]
pub(super) enum Incoming {
    /// The agent asks atelier something and waits for the answer.
    Request { id: Value, method: String, params: Value },
    /// The answer to one of atelier's requests.
    Response { id: Value, outcome: Result<Value, RpcError> },
    Notification { method: String, params: Value },
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(super) struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

impl fmt::Display for RpcError {
    /// The agent's own words: `data.message` when it gives one, as Cursor does with what to run, else
    /// `message`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let detail = self.data.as_ref().and_then(|d| d.get("message")).and_then(Value::as_str);
        f.write_str(detail.unwrap_or(&self.message))
    }
}

#[derive(Deserialize)]
struct Raw {
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: Option<String>,
    #[serde(default)]
    params: Value,
    /// `Some(Value::Null)` for `"result": null`, which is an answer; `None` when there is no `result`.
    #[serde(default, deserialize_with = "present")]
    result: Option<Value>,
    /// `Some(Value::Null)` for `"error": null`, which is no error object; `None` when there is no `error`.
    #[serde(default, deserialize_with = "present")]
    error: Option<Value>,
}

fn present<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

/// Reads one line. A line that is not a JSON-RPC message is an error that says why.
pub(super) fn parse(line: &str) -> Result<Incoming, String> {
    let raw: Raw = serde_json::from_str(line).map_err(|e| e.to_string())?;
    if raw.jsonrpc.as_deref() != Some("2.0") {
        return Err("a message that is not JSON-RPC 2.0".into());
    }
    match (raw.id, raw.method) {
        (Some(id), Some(method)) => Ok(Incoming::Request { id, method, params: raw.params }),
        (None, Some(method)) => Ok(Incoming::Notification { method, params: raw.params }),
        (Some(id), None) => match (raw.result, raw.error) {
            (Some(result), None) => Ok(Incoming::Response { id, outcome: Ok(result) }),
            (None, Some(error)) => match serde_json::from_value::<RpcError>(error) {
                Ok(error) => Ok(Incoming::Response { id, outcome: Err(error) }),
                Err(_) => Err("a response whose error is not an error object".into()),
            },
            _ => Err("a response that is not one result or one error".into()),
        },
        (None, None) => Err("a message with neither an id nor a method".into()),
    }
}

pub(super) fn request(id: u64, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

pub(super) fn notification(method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "method": method, "params": params }).to_string()
}

pub(super) fn result(id: &Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

pub(super) fn error(id: &Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}
