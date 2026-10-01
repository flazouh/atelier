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
