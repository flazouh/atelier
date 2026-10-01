use serde_json::json;

use crate::acp::rpc::{Incoming, RpcError, error, notification, parse, request, result};

#[test]
fn a_line_reads_as_a_request_a_response_or_a_notification() {
    let asked = parse(r#"{"jsonrpc":"2.0","id":"p1","method":"session/request_permission","params":{"a":1}}"#).unwrap();
    assert_eq!(asked, Incoming::Request { id: json!("p1"), method: "session/request_permission".into(), params: json!({"a":1}) });

    let answered = parse(r#"{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}"#).unwrap();
    assert_eq!(answered, Incoming::Response { id: json!(3), outcome: Ok(json!({"stopReason":"end_turn"})) });

    let told = parse(r#"{"jsonrpc":"2.0","method":"session/update","params":{}}"#).unwrap();
    assert_eq!(told, Incoming::Notification { method: "session/update".into(), params: json!({}) });

    // A result of `null` is still an answer.
    let empty = parse(r#"{"jsonrpc":"2.0","id":4,"result":null}"#).unwrap();
    assert_eq!(empty, Incoming::Response { id: json!(4), outcome: Ok(json!(null)) });
}

#[test]
fn an_error_says_the_agents_own_words_when_it_gives_them() {
    let Incoming::Response { outcome: Err(failed), .. } =
        parse(r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"Authentication required","data":{"message":"Run 'agent login' first."}}}"#).unwrap()
    else {
        panic!("an error response")
    };
    assert_eq!(failed.code, -32000);
    assert_eq!(failed.to_string(), "Run 'agent login' first.");
    let bare = RpcError { code: -32603, message: "Internal error".into(), data: None };
    assert_eq!(bare.to_string(), "Internal error");
}

#[test]
fn a_line_that_is_not_json_rpc_says_why() {
    assert!(parse("not json").is_err());
    assert!(parse(r#"{"jsonrpc":"2.0"}"#).is_err());
}

#[test]
fn ateliers_lines_are_json_rpc_two() {
    let line: serde_json::Value = serde_json::from_str(&request(7, "session/prompt", json!({"x":1}))).unwrap();
    assert_eq!(line, json!({"jsonrpc":"2.0","id":7,"method":"session/prompt","params":{"x":1}}));
    let line: serde_json::Value = serde_json::from_str(&notification("session/cancel", json!({}))).unwrap();
    assert_eq!(line, json!({"jsonrpc":"2.0","method":"session/cancel","params":{}}));
    let line: serde_json::Value = serde_json::from_str(&result(&json!("p1"), json!({"ok":true}))).unwrap();
    assert_eq!(line, json!({"jsonrpc":"2.0","id":"p1","result":{"ok":true}}));
    let line: serde_json::Value = serde_json::from_str(&error(&json!(2), -32601, "no")).unwrap();
    assert_eq!(line, json!({"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"no"}}));
}

/// A frame that is not JSON-RPC 2.0, or a response that is neither a result nor an error, is no answer: it
/// cannot end a waiting handshake or turn as if the agent had said `null`.
#[test]
fn a_malformed_frame_is_an_error_and_not_an_empty_answer() {
    assert!(parse(r#"{"jsonrpc":"2.0","id":3}"#).is_err(), "neither a result nor an error");
    assert!(parse(r#"{"jsonrpc":"2.0","id":3,"result":{},"error":{"code":1,"message":"x"}}"#).is_err(), "both");
    assert!(parse(r#"{"id":3,"result":{}}"#).is_err(), "no version");
    assert!(parse(r#"{"jsonrpc":"1.0","id":3,"result":{}}"#).is_err(), "another version");
    assert_eq!(parse(r#"{"jsonrpc":"2.0","id":4,"result":null}"#).unwrap(), Incoming::Response { id: json!(4), outcome: Ok(json!(null)) });
}
