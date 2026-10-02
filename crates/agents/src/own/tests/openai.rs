use serde_json::{Value, json};

use super::server::{FakeServer, Step};
use crate::own::{
    message::{Block, Cancel, Delta, Message, Model, ModelError, ModelRequest, Role, Secret, StopReason, Thinking, ToolDef},
    openai::{ChatState, OpenAiCompatible, request_body},
};

fn data(value: Value) -> String {
    format!("data: {value}\n\n")
}

fn chunk(delta: Value, finish: Option<&str>) -> String {
    data(json!({"choices": [{"index": 0, "delta": delta, "finish_reason": finish}]}))
}

fn stream(parts: &[String]) -> Vec<u8> {
    let mut all = parts.concat();
    all.push_str("data: [DONE]\n\n");
    all.into_bytes()
}

fn ask(server: &FakeServer, client: OpenAiCompatible) -> (Result<crate::own::Reply, ModelError>, Vec<Delta>) {
    let _ = server;
    let tools = [ToolDef { name: "read".into(), description: "d".into(), schema: json!({"type": "object"}) }];
    let messages = [Message::user("hi")];
    let mut deltas = Vec::new();
    let result = client.stream(&ModelRequest { model: "gpt-x", system: "SYS", tools: &tools, messages: &messages, max_tokens: 100, thinking: Thinking::Off }, &mut |d| deltas.push(d), &Cancel::default());
    (result, deltas)
}

#[test]
fn text_and_usage_stream_and_the_key_goes_in_a_bearer_header() {
    let parts = [
        chunk(json!({"role": "assistant", "content": "Hel"}), None),
        chunk(json!({"content": "lo"}), None),
        chunk(json!({}), Some("stop")),
        data(json!({"choices": [], "usage": {"prompt_tokens": 30, "completion_tokens": 4, "prompt_tokens_details": {"cached_tokens": 10}}})),
    ];
    let server = FakeServer::start(vec![Step::Raw { bytes: stream(&parts), chunk: 13 }]);
    let client = OpenAiCompatible::new(Some(Secret::new("sk-openai")), format!("{}/v1", server.url));
    let (reply, deltas) = ask(&server, client);
    let reply = reply.unwrap();
    assert_eq!(reply.blocks, vec![Block::Text { text: "Hello".into() }]);
    assert_eq!(reply.stop, StopReason::EndTurn);
    assert_eq!((reply.usage.input, reply.usage.output, reply.usage.cache_read), (20, 4, 10), "cached tokens are not counted twice");
    assert_eq!(deltas, vec![Delta::Text("Hel".into()), Delta::Text("lo".into()), Delta::BlockEnd]);
    let request = &server.recorded()[0];
    assert_eq!(request.path, "/v1/chat/completions");
    assert_eq!(request.header("authorization"), Some("Bearer sk-openai"));
    assert_eq!(request.body["stream"], true);
    assert_eq!(request.body["stream_options"]["include_usage"], true);
    assert_eq!(request.body["messages"][0], json!({"role": "system", "content": "SYS"}));
}

#[test]
fn reasoning_arrives_as_thinking_before_the_text_and_is_not_signed() {
    let parts = [chunk(json!({"reasoning_content": "hm "}), None), chunk(json!({"reasoning": "so"}), None), chunk(json!({"content": "Answer"}), None), chunk(json!({}), Some("stop"))];
    let server = FakeServer::start(vec![Step::Raw { bytes: stream(&parts), chunk: 1000 }]);
    let (reply, deltas) = ask(&server, OpenAiCompatible::new(None, server.url.clone()));
    let reply = reply.unwrap();
    assert_eq!(reply.blocks, vec![Block::Thinking { text: "hm so".into(), signature: None }, Block::Text { text: "Answer".into() }]);
    assert_eq!(deltas, vec![Delta::Thinking("hm ".into()), Delta::Thinking("so".into()), Delta::BlockEnd, Delta::Text("Answer".into()), Delta::BlockEnd]);
    assert!(server.recorded()[0].header("authorization").is_none(), "a local server needs no key");
}

#[test]
fn tool_calls_arrive_in_fragments_by_index() {
    let parts = [
        chunk(json!({"tool_calls": [{"index": 0, "id": "call_a", "type": "function", "function": {"name": "read", "arguments": ""}}]}), None),
        chunk(json!({"tool_calls": [{"index": 0, "function": {"arguments": "{\"path\":"}}]}), None),
        chunk(json!({"tool_calls": [{"index": 1, "id": "call_b", "type": "function", "function": {"name": "list", "arguments": "{}"}}]}), None),
        chunk(json!({"tool_calls": [{"index": 0, "function": {"arguments": " \"x.rs\"}"}}]}), None),
        chunk(json!({}), Some("tool_calls")),
    ];
    let server = FakeServer::start(vec![Step::Raw { bytes: stream(&parts), chunk: 50 }]);
    let (reply, deltas) = ask(&server, OpenAiCompatible::new(None, server.url.clone()));
    let reply = reply.unwrap();
    assert_eq!(
        reply.blocks,
        vec![
            Block::ToolUse { id: "call_a".into(), name: "read".into(), input: json!({"path": "x.rs"}) },
            Block::ToolUse { id: "call_b".into(), name: "list".into(), input: json!({}) },
        ]
    );
    assert_eq!(reply.stop, StopReason::ToolUse);
    assert!(matches!(&deltas[0], Delta::ToolStart { id, name } if id == "call_a" && name == "read"));
    let pieces = |of: &str| deltas.iter().filter_map(|d| if let Delta::ToolInput { id, piece } = d { (id == of).then_some(piece.as_str()) } else { None }).collect::<String>();
    assert_eq!(pieces("call_a"), "{\"path\": \"x.rs\"}", "each piece of the input is passed on as it comes");
    assert_eq!(pieces("call_b"), "{}");
}

#[test]
fn a_tool_call_whose_arguments_are_not_json_is_marked_and_a_stop_with_calls_means_tool_use() {
    let parts = [chunk(json!({"tool_calls": [{"index": 0, "id": "c", "function": {"name": "read", "arguments": "{\"pa"}}]}), None), chunk(json!({}), Some("stop"))];
    let server = FakeServer::start(vec![Step::Raw { bytes: stream(&parts), chunk: 1000 }]);
    let (reply, _) = ask(&server, OpenAiCompatible::new(None, server.url.clone()));
    let reply = reply.unwrap();
    assert_eq!(reply.malformed, vec!["c"]);
    assert_eq!(reply.stop, StopReason::ToolUse);
}

#[test]
fn finish_reasons_map() {
    for (word, stop) in [("length", StopReason::MaxTokens), ("content_filter", StopReason::Refusal), ("stop", StopReason::EndTurn)] {
        let mut state = ChatState::default();
        state.on_data(&json!({"choices": [{"delta": {"content": "x"}, "finish_reason": word}]}).to_string(), &mut |_| {}).unwrap();
        state.on_data("[DONE]", &mut |_| {}).unwrap();
        assert_eq!(state.finish(&mut |_| {}).unwrap().stop, stop, "{word}");
    }
}

#[test]
fn an_error_in_the_stream_a_missing_done_and_bad_json_are_errors() {
    let mut state = ChatState::default();
    let error = state.on_data(&json!({"error": {"message": "quota", "code": 503}}).to_string(), &mut |_| {}).unwrap_err();
    assert!(matches!(&error, ModelError::Server { status: 503, message } if message == "quota"));
    let mut state = ChatState::default();
    state.on_data(&json!({"choices": [{"delta": {"content": "x"}}]}).to_string(), &mut |_| {}).unwrap();
    assert!(matches!(state.finish(&mut |_| {}), Err(ModelError::Malformed(w)) if w.contains("ended before")));
    assert!(matches!(ChatState::default().on_data("{oops", &mut |_| {}), Err(ModelError::Malformed(_))));
}

#[test]
fn statuses_map_to_errors() {
    for (code, retryable) in [(401, false), (400, false), (429, true), (503, true)] {
        let server = FakeServer::start(vec![Step::Status { code, headers: vec![], body: json!({"error": {"message": "words"}}).to_string() }]);
        let (result, _) = ask(&server, OpenAiCompatible::new(None, server.url.clone()));
        let error = result.unwrap_err();
        assert_eq!(error.is_retryable(), retryable, "{code}");
        assert!(error.to_string().contains("words"));
    }
}

#[test]
fn openrouter_sends_two_headers_that_name_the_app() {
    let parts = [chunk(json!({"content": "x"}), Some("stop"))];
    let server = FakeServer::start(vec![Step::Raw { bytes: stream(&parts), chunk: 1000 }]);
    let client = OpenAiCompatible::new(Some(Secret::new("or-key")), format!("{}/api/v1", server.url)).with_header("HTTP-Referer", "https://example.test").with_header("X-Title", "atelier");
    ask(&server, client).0.unwrap();
    let request = &server.recorded()[0];
    assert_eq!(request.path, "/api/v1/chat/completions");
    assert_eq!((request.header("http-referer"), request.header("x-title")), (Some("https://example.test"), Some("atelier")));
    let router = OpenAiCompatible::openrouter(Secret::new("k"));
    let _ = router;
}

#[test]
fn messages_map_to_chat_form_with_tool_results_as_tool_messages() {
    let messages = vec![
        Message::user("do it"),
        Message::assistant(vec![
            Block::Thinking { text: "hidden".into(), signature: None },
            Block::Text { text: "Sure.".into() },
            Block::ToolUse { id: "c1".into(), name: "read".into(), input: json!({"path": "a"}) },
        ]),
        Message { role: Role::User, blocks: vec![Block::ToolResult { id: "c1".into(), content: "contents".into(), is_error: false }] },
        Message::assistant(vec![Block::ToolUse { id: "c2".into(), name: "list".into(), input: json!({}) }]),
    ];
    let tools = [ToolDef { name: "read".into(), description: "d".into(), schema: json!({"type": "object", "properties": {}}) }];
    let b = request_body(&ModelRequest { model: "m", system: "S", tools: &tools, messages: &messages, max_tokens: 50, thinking: Thinking::Auto });
    let m = b["messages"].as_array().unwrap();
    assert_eq!(m[1], json!({"role": "user", "content": "do it"}));
    assert_eq!(m[2]["content"], "Sure.");
    assert_eq!(m[2]["tool_calls"][0], json!({"id": "c1", "type": "function", "function": {"name": "read", "arguments": "{\"path\":\"a\"}"}}));
    assert!(!b.to_string().contains("hidden"), "reasoning is not sent back");
    assert_eq!(m[3], json!({"role": "tool", "tool_call_id": "c1", "content": "contents"}));
    assert_eq!(m[4]["content"], Value::Null, "a message of calls only has null content");
    assert_eq!(b["tools"][0]["function"]["name"], "read");
    assert_eq!(b["max_tokens"], 50);
}
