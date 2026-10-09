//! The MCP messages the gateway answers.
use serde_json::json;

use super::{VERSION, fixture};

#[test]
fn initialize_names_the_server_and_its_tools() {
    let f = fixture();
    let answer = f.rpc(
        "initialize",
        json!({ "protocolVersion": VERSION, "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } }),
    );
    let result = &answer["result"];
    assert_eq!(result["protocolVersion"], VERSION);
    assert_eq!(result["serverInfo"]["name"], "atelier");
    assert!(result["capabilities"]["tools"].is_object());
    assert_eq!(answer["id"], 1);
}

#[test]
fn an_older_version_the_gateway_knows_is_kept() {
    let f = fixture();
    let answer = f.rpc("initialize", json!({ "protocolVersion": "2025-06-18" }));
    assert_eq!(answer["result"]["protocolVersion"], "2025-06-18");
}

#[test]
fn a_version_it_does_not_know_gets_the_newest() {
    let f = fixture();
    let answer = f.rpc("initialize", json!({ "protocolVersion": "2099-01-01" }));
    assert_eq!(answer["result"]["protocolVersion"], VERSION);
}

#[test]
fn the_initialized_notice_is_accepted_with_no_body() {
    let f = fixture();
    let body = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    let (status, answer) = f.post(Some(&f.access.token), &[], &body);
    assert_eq!(status, 202);
    assert!(answer.is_null());
}

#[test]
fn ping_answers_with_an_empty_object() {
    let f = fixture();
    assert_eq!(f.rpc("ping", json!({}))["result"], json!({}));
}

#[test]
fn a_protocol_version_header_it_does_not_know_is_a_bad_request() {
    let f = fixture();
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
    let (status, _) = f.post(
        Some(&f.access.token),
        &[("MCP-Protocol-Version", "1999-01-01")],
        &body,
    );
    assert_eq!(status, 400);
}

#[test]
fn a_body_that_is_not_json_is_a_parse_error() {
    let f = fixture();
    let response = f
        .http
        .post(&f.access.url)
        .header("Authorization", format!("Bearer {}", f.access.token))
        .send("{not json")
        .unwrap();
    assert_eq!(response.status().as_u16(), 400);
}

#[test]
fn an_unknown_method_is_a_json_rpc_error() {
    let f = fixture();
    let answer = f.rpc("resources/list", json!({}));
    assert_eq!(answer["error"]["code"], -32601);
}

#[test]
fn an_unknown_tool_is_a_json_rpc_error() {
    let f = fixture();
    let answer = f.rpc(
        "tools/call",
        json!({ "name": "mail_send", "arguments": {} }),
    );
    assert_eq!(answer["error"]["code"], -32602);
}

#[test]
fn tools_list_has_the_six_tasks_tools_with_schemas_and_hints() {
    let f = fixture();
    let tools = f.rpc("tools/list", json!({}))["result"]["tools"].clone();
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "tasks_list",
            "tasks_get",
            "tasks_create",
            "tasks_update",
            "tasks_comment",
            "tasks_search"
        ]
    );
    for tool in tools.as_array().unwrap() {
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        assert!(tool["description"].as_str().is_some_and(|d| !d.is_empty()));
        assert!(tool["inputSchema"]["properties"]["account"].is_object());
    }
    let hint = |name: &str, key: &str| {
        tools
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .map(|t| t["annotations"][key].clone())
            .unwrap()
    };
    for read in ["tasks_list", "tasks_get", "tasks_search"] {
        assert_eq!(hint(read, "readOnlyHint"), json!(true), "{read}");
        assert_eq!(hint(read, "destructiveHint"), json!(false), "{read}");
    }
    for write in ["tasks_create", "tasks_update", "tasks_comment"] {
        assert_eq!(hint(write, "readOnlyHint"), json!(false), "{write}");
        assert_eq!(hint(write, "destructiveHint"), json!(false), "{write}");
    }
    let required = |name: &str| {
        tools
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .map(|t| t["inputSchema"]["required"].clone())
            .unwrap()
    };
    assert_eq!(required("tasks_create"), json!(["title"]));
    assert_eq!(required("tasks_get"), json!(["ref"]));
    assert_eq!(required("tasks_comment"), json!(["ref", "body"]));
    assert_eq!(required("tasks_search"), json!(["query"]));
}
