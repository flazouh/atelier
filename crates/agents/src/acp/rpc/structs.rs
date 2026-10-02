use std::fmt;

use serde::Deserialize;
use serde_json::Value;

use super::helpers::present;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(in super::super) struct RpcError {
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
pub(super) struct Raw {
    pub(super) jsonrpc: Option<String>,
    pub(super) id: Option<Value>,
    pub(super) method: Option<String>,
    #[serde(default)]
    pub(super) params: Value,
    /// `Some(Value::Null)` for `"result": null`, which is an answer; `None` when there is no `result`.
    #[serde(default, deserialize_with = "present")]
    pub(super) result: Option<Value>,
    /// `Some(Value::Null)` for `"error": null`, which is no error object; `None` when there is no `error`.
    #[serde(default, deserialize_with = "present")]
    pub(super) error: Option<Value>,
}
