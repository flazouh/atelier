//! What atelier asks of an ACP agent, as a method and its params. `protocol` numbers the requests and
//! frames them with `rpc`.
use serde_json::{Value, json};

/// The protocol version atelier speaks.
pub(super) const PROTOCOL_VERSION: u64 = 1;

pub(super) struct Outgoing {
    pub method: &'static str,
    pub params: Value,
}

fn out(method: &'static str, params: Value) -> Outgoing {
    Outgoing { method, params }
}

/// atelier serves no file or terminal methods: the agent runs on the project's host, through
/// `Project::spawn`, and its own tools read, write and run there.
pub(super) fn initialize() -> Outgoing {
    out(
        "initialize",
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false }, "terminal": false },
            "clientInfo": { "name": "atelier", "version": env!("CARGO_PKG_VERSION") },
        }),
    )
}

pub(super) fn authenticate(method: &str) -> Outgoing {
    out("authenticate", json!({ "methodId": method }))
}

pub(super) fn new_session(cwd: &str) -> Outgoing {
    out("session/new", json!({ "cwd": cwd, "mcpServers": [] }))
}

pub(super) fn load_session(cwd: &str, session: &str) -> Outgoing {
    out("session/load", json!({ "sessionId": session, "cwd": cwd, "mcpServers": [] }))
}

pub(super) fn list_sessions(cwd: &str) -> Outgoing {
    out("session/list", json!({ "cwd": cwd }))
}

pub(super) fn prompt(session: &str, text: &str) -> Outgoing {
    out("session/prompt", json!({ "sessionId": session, "prompt": [{ "type": "text", "text": text }] }))
}

/// A notification: the prompt's own answer ends the turn, with stop reason `cancelled`.
pub(super) fn cancel(session: &str) -> Outgoing {
    out("session/cancel", json!({ "sessionId": session }))
}

pub(super) fn set_mode(session: &str, mode: &str) -> Outgoing {
    out("session/set_mode", json!({ "sessionId": session, "modeId": mode }))
}

/// Sets one of the session's config options, such as its model.
pub(super) fn set_config_option(session: &str, option: &str, value: &str) -> Outgoing {
    out("session/set_config_option", json!({ "sessionId": session, "configId": option, "value": value }))
}

/// Sets the model through ACP's unstable model selection, for an agent that offers no model option.
pub(super) fn set_model(session: &str, model: &str) -> Outgoing {
    out("session/set_model", json!({ "sessionId": session, "modelId": model }))
}

/// The result of `session/request_permission` for the option chosen.
pub(super) fn permission_selected(option: &str) -> Value {
    json!({ "outcome": { "outcome": "selected", "optionId": option } })
}

/// The result of `session/request_permission` when the turn was cancelled first.
pub(super) fn permission_cancelled() -> Value {
    json!({ "outcome": { "outcome": "cancelled" } })
}
