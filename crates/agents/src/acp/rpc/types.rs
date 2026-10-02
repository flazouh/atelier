use serde_json::Value;

use super::structs::RpcError;

/// The code an agent returns for a method it does not have, and atelier for one it does not serve.
pub(in super::super) const METHOD_NOT_FOUND: i64 = -32601;

/// One line from the agent.
#[derive(Debug, PartialEq)]
pub(in super::super) enum Incoming {
    /// The agent asks atelier something and waits for the answer.
    Request { id: Value, method: String, params: Value },
    /// The answer to one of atelier's requests.
    Response { id: Value, outcome: Result<Value, RpcError> },
    Notification { method: String, params: Value },
}
