use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::server::{self, FakeServer, Step};
use crate::own::{
    anthropic::{Anthropic, StreamState, request_body},
    message::{Block, Cancel, Delta, Message, Model, ModelError, ModelRequest, Role, Secret, StopReason, Thinking, ToolDef},
};

fn run(events: server::Sse) -> (Result<crate::own::Reply, ModelError>, Vec<Delta>) {
    let mut state = StreamState::default();
    let mut deltas = Vec::new();
    for (name, data) in &events {
        if let Err(e) = state.on_event(Some(name), data, &mut |d| deltas.push(d)) {
            return (Err(e), deltas);
        }
    }
    (state.finish(), deltas)
}

#[test]
fn text_thinking_and_a_tool_call_build_a_reply_in_order_with_usage() {
    let events = server::reply(
        vec![
            server::thinking_block(0, &["hm"], "sig"),
            server::text_block(1, &["Hel", "lo"]),
            server::tool_block(2, "t1", "read", &json!({"path": "a"})),
        ],
        "tool_use",
    );
    let (reply, deltas) = run(events);
    let reply = reply.unwrap();
    assert_eq!(
        reply.blocks,
        vec![
            Block::Thinking { text: "hm".into(), signature: Some("sig".into()) },
            Block::Text { text: "Hello".into() },
            Block::ToolUse { id: "t1".into(), name: "read".into(), input: json!({"path": "a"}) },
        ]
    );
    assert_eq!(reply.stop, StopReason::ToolUse);
    assert_eq!((reply.usage.input, reply.usage.output, reply.usage.cache_read, reply.usage.cache_write), (100, 42, 5, 7));
    assert!(reply.malformed.is_empty());
    assert_eq!(
        deltas,
        vec![
            Delta::Thinking("hm".into()),
            Delta::BlockEnd,
            Delta::Text("Hel".into()),
            Delta::Text("lo".into()),
            Delta::BlockEnd,
            Delta::ToolStart { id: "t1".into(), name: "read".into() },
            Delta::ToolDone { id: "t1".into(), input: json!({"path": "a"}) },
        ]
    );
}

#[test]
fn pings_unknown_events_and_blocks_of_kinds_we_do_not_use_are_ignored() {
    let mut events = vec![server::start(1), ("ping".into(), json!({"type": "ping"}).to_string()), ("future_event".into(), json!({"type": "future_event"}).to_string())];
    events.push(("content_block_start".into(), json!({"type": "content_block_start", "index": 0, "content_block": {"type": "server_tool_use", "id": "s", "name": "web_search", "input": {}}}).to_string()));
    events.push(("content_block_delta".into(), json!({"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": "{}"}}).to_string()));
    events.push(("content_block_stop".into(), json!({"type": "content_block_stop", "index": 0}).to_string()));
    events.extend(server::text_block(1, &["ok"]));
    events.extend(server::end("end_turn", 3));
    let (reply, _) = run(events);
    assert_eq!(reply.unwrap().blocks, vec![Block::Text { text: "ok".into() }]);
}

#[test]
fn a_tool_call_with_no_input_gets_an_empty_object_and_bad_json_is_marked() {
    let none = vec![
        server::start(1),
        ("content_block_start".into(), json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "a", "name": "list", "input": {}}}).to_string()),
        ("content_block_stop".into(), json!({"type": "content_block_stop", "index": 0}).to_string()),
    ];
    let mut events = none;
    events.extend(server::end("tool_use", 1));
    let reply = run(events).0.unwrap();
    assert_eq!(reply.blocks, vec![Block::ToolUse { id: "a".into(), name: "list".into(), input: json!({}) }]);

    let mut broken = vec![
        server::start(1),
        ("content_block_start".into(), json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "b", "name": "read", "input": {}}}).to_string()),
        ("content_block_delta".into(), json!({"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": "{\"path\":"}}).to_string()),
        ("content_block_stop".into(), json!({"type": "content_block_stop", "index": 0}).to_string()),
    ];
    broken.extend(server::end("tool_use", 1));
    let reply = run(broken).0.unwrap();
    assert_eq!(reply.malformed, vec!["b"]);
    assert_eq!(reply.blocks, vec![Block::ToolUse { id: "b".into(), name: "read".into(), input: Value::Null }]);
}

#[test]
fn redacted_thinking_is_kept_to_be_sent_back() {
    let mut events = vec![
        server::start(1),
        ("content_block_start".into(), json!({"type": "content_block_start", "index": 0, "content_block": {"type": "redacted_thinking", "data": "ENCRYPTED"}}).to_string()),
        ("content_block_stop".into(), json!({"type": "content_block_stop", "index": 0}).to_string()),
    ];
    events.extend(server::end("end_turn", 1));
    assert_eq!(run(events).0.unwrap().blocks, vec![Block::Redacted { data: "ENCRYPTED".into() }]);
}

#[test]
fn stop_reasons_map_and_an_unknown_one_is_kept() {
    for (word, stop) in [
        ("end_turn", StopReason::EndTurn),
        ("stop_sequence", StopReason::EndTurn),
        ("tool_use", StopReason::ToolUse),
        ("max_tokens", StopReason::MaxTokens),
        ("refusal", StopReason::Refusal),
        ("pause_turn", StopReason::Other("pause_turn".into())),
    ] {
        assert_eq!(run(server::reply(vec![], word)).0.unwrap().stop, stop, "{word}");
    }
}

#[test]
fn an_error_event_in_the_stream_becomes_the_error_it_names() {
    for (kind, retryable) in [("overloaded_error", true), ("api_error", true), ("rate_limit_error", true), ("invalid_request_error", false), ("authentication_error", false)] {
        let events = vec![server::start(1), ("error".into(), json!({"type": "error", "error": {"type": kind, "message": "bad"}}).to_string())];
        let error = run(events).0.unwrap_err();
        assert_eq!(error.is_retryable(), retryable, "{kind}: {error:?}");
        assert!(error.to_string().contains("bad"));
    }
}

#[test]
fn a_stream_that_breaks_is_an_error() {
    assert!(matches!(run(vec![server::start(1)]).0, Err(ModelError::Malformed(w)) if w.contains("ended before")));
    assert!(matches!(run(vec![("message_stop".into(), "{\"type\":\"message_stop\"}".into())]).0, Err(ModelError::Malformed(_))), "a stop with no start");
    assert!(matches!(run(vec![("x".into(), "not json".into())]).0, Err(ModelError::Malformed(w)) if w.contains("not JSON")));
    let orphan = vec![server::start(1), ("content_block_delta".into(), json!({"type": "content_block_delta", "index": 3, "delta": {"type": "text_delta", "text": "x"}}).to_string())];
    assert!(matches!(run(orphan).0, Err(ModelError::Malformed(w)) if w.contains("not open")));
}

fn tools() -> Vec<ToolDef> {
    vec![
        ToolDef { name: "a".into(), description: "first".into(), schema: json!({"type": "object"}) },
        ToolDef { name: "b".into(), description: "second".into(), schema: json!({"type": "object"}) },
    ]
}

fn body(messages: &[Message], model: &str, thinking: Thinking) -> Value {
    request_body(&ModelRequest { model, system: "SYSTEM", tools: &tools(), messages, max_tokens: 8000, thinking })
}

#[test]
fn blocks_are_sent_the_way_the_api_wants_them() {
    let messages = vec![
        Message::user("question"),
        Message::assistant(vec![
            Block::Thinking { text: "t".into(), signature: Some("s".into()) },
            Block::Thinking { text: "unsigned".into(), signature: None },
            Block::Text { text: "   ".into() },
            Block::Redacted { data: "R".into() },
            Block::ToolUse { id: "1".into(), name: "a".into(), input: Value::Null },
            Block::ToolUse { id: "2".into(), name: "b".into(), input: json!({"k": 1}) },
        ]),
        Message { role: Role::User, blocks: vec![Block::ToolResult { id: "1".into(), content: "ok".into(), is_error: false }, Block::ToolResult { id: "2".into(), content: "bad".into(), is_error: true }] },
    ];
    let b = body(&messages, "claude-opus-5-5", Thinking::Auto);
    let assistant = &b["messages"][1]["content"];
    assert_eq!(assistant.as_array().unwrap().len(), 4, "the unsigned thinking and the blank text are left out");
    assert_eq!(assistant[0], json!({"type": "thinking", "thinking": "t", "signature": "s"}));
    assert_eq!(assistant[1], json!({"type": "redacted_thinking", "data": "R"}));
    assert_eq!(assistant[2]["input"], json!({}), "a null input is sent as an empty object");
    let results = &b["messages"][2]["content"];
    assert!(results[0].get("is_error").is_none());
    assert_eq!(results[1]["is_error"], true);
}

#[test]
fn three_cache_breakpoints_stand_at_the_last_tool_the_system_prompt_and_the_end() {
    let b = body(&[Message::user("hi")], "claude-opus-5-5", Thinking::Auto);
    let tools = b["tools"].as_array().unwrap();
    assert!(tools[0].get("cache_control").is_none());
    assert!(tools.iter().all(|t| t["eager_input_streaming"] == true), "tool inputs stream as they are made");
    assert_eq!(tools[1]["cache_control"]["type"], "ephemeral");
    assert_eq!(b["system"][0]["cache_control"]["type"], "ephemeral");
    assert_eq!(b["messages"][0]["content"][0]["cache_control"]["type"], "ephemeral");
    let count = b.to_string().matches("cache_control").count();
    assert_eq!(count, 3, "the API allows four; three are used");
}

#[test]
fn thinking_is_adaptive_and_summarized_or_absent_and_haiku_gets_a_budget() {
    let messages = [Message::user("hi")];
    assert_eq!(body(&messages, "claude-opus-5-5", Thinking::Auto)["thinking"], json!({"type": "adaptive", "display": "summarized"}));
    assert!(body(&messages, "claude-opus-5-5", Thinking::Off).get("thinking").is_none());
    let haiku = body(&messages, "claude-haiku-4-5", Thinking::Auto);
    assert_eq!(haiku["thinking"]["type"], "enabled");
    let budget = haiku["thinking"]["budget_tokens"].as_u64().unwrap();
    assert!((1024..8000).contains(&budget), "{budget}");
}

#[test]
fn a_conversation_that_ends_on_an_empty_message_sends_no_empty_message() {
    let messages = vec![Message::user("hi"), Message::assistant(vec![Block::Text { text: "".into() }])];
    let b = body(&messages, "claude-opus-5-5", Thinking::Off);
    assert_eq!(b["messages"].as_array().unwrap().len(), 1);
}

fn ask(server: &FakeServer, cancel: &Cancel) -> Result<crate::own::Reply, ModelError> {
    let model = Anthropic::new(Secret::new("k")).with_base(server.url.clone());
    let messages = [Message::user("hi")];
    model.stream(&ModelRequest { model: "m", system: "s", tools: &[], messages: &messages, max_tokens: 100, thinking: Thinking::Off }, &mut |_| {}, cancel)
}

#[test]
fn a_request_cancelled_while_the_reply_streams_returns_at_once_and_hangs_up() {
    let events = vec![server::start(1), server::text_block(0, &["partial"])[0].clone(), server::text_block(0, &["partial"])[1].clone()];
    let server = FakeServer::start(vec![Step::Hang(events)]);
    let cancel = Cancel::default();
    let flag = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(250));
        flag.set();
    });
    let started = Instant::now();
    assert_eq!(ask(&server, &cancel).unwrap_err(), ModelError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    let limit = Instant::now() + Duration::from_secs(5);
    while !server.hung_up.load(std::sync::atomic::Ordering::SeqCst) && Instant::now() < limit {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(server.hung_up.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn a_request_cancelled_before_it_starts_never_reaches_the_server() {
    let server = FakeServer::start(vec![Step::Sse(server::says("x"))]);
    let cancel = Cancel::default();
    cancel.set();
    assert_eq!(ask(&server, &cancel).unwrap_err(), ModelError::Cancelled);
    std::thread::sleep(Duration::from_millis(50));
    assert!(server.recorded().is_empty());
}

#[test]
fn the_status_maps_to_the_error_and_a_retry_after_is_read() {
    let cases = [(401, "authentication_error", false), (403, "permission_error", false), (400, "invalid_request_error", false), (429, "rate_limit_error", true), (500, "api_error", true), (529, "overloaded_error", true)];
    for (code, kind, retryable) in cases {
        let server = FakeServer::start(vec![Step::Status { code, headers: vec![("retry-after", "2.5".into())], body: server::error_body(kind, "words") }]);
        let error = ask(&server, &Cancel::default()).unwrap_err();
        assert_eq!(error.is_retryable(), retryable, "{code}: {error:?}");
        if code == 429 {
            assert!(matches!(error, ModelError::RateLimited { retry_after: Some(d), .. } if d == Duration::from_millis(2500)));
        }
    }
    let plain = FakeServer::start(vec![Step::Status { code: 502, headers: vec![], body: "<html>Bad gateway</html>".into() }]);
    assert!(matches!(ask(&plain, &Cancel::default()).unwrap_err(), ModelError::Server { status: 502, message } if message.contains("Bad gateway")), "an error page still gives a message");
}

#[test]
fn the_model_list_comes_from_the_models_endpoint() {
    let body = json!({"data": [{"id": "claude-opus-5-5", "display_name": "Claude Opus 5.5"}, {"id": "claude-x"}], "has_more": false}).to_string();
    let server = FakeServer::start(vec![Step::Status { code: 200, headers: vec![], body }]);
    let model = Anthropic::new(Secret::new("k")).with_base(server.url.clone());
    let list = model.models(&Cancel::default()).unwrap();
    assert_eq!(list, vec![("claude-opus-5-5".to_string(), "Claude Opus 5.5".to_string()), ("claude-x".to_string(), "claude-x".to_string())]);
    let request = &server.recorded()[0];
    assert_eq!((request.method.as_str(), request.path.as_str()), ("GET", "/v1/models?limit=100"));
}

#[test]
fn the_key_never_shows_in_a_debug_print() {
    let secret = Secret::new("sk-ant-real-value");
    assert!(!format!("{secret:?}").contains("real-value"));
    assert!(!format!("{:?}", Some(&secret)).contains("real-value"));
}
