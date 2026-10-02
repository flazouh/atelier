use serde::Deserialize;
use serde_json::{Value, json};

use super::structs::{Raw, RpcError};
use super::types::Incoming;

pub(super) fn present<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

/// Reads one line. A line that is not a JSON-RPC message is an error that says why.
pub(in super::super) fn parse(line: &str) -> Result<Incoming, String> {
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

pub(in super::super) fn request(id: u64, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

pub(in super::super) fn notification(method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "method": method, "params": params }).to_string()
}

pub(in super::super) fn result(id: &Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

pub(in super::super) fn error(id: &Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}
