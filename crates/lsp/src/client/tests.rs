use super::*;
use std::path::{Path, PathBuf};

#[test]
fn a_reply_goes_to_the_request_that_asked() {
    let message = json!({"jsonrpc": "2.0", "id": 7, "result": {"ok": true}});
    match classify(&message) {
        Routed::Reply(7, Ok(value)) => assert_eq!(value, json!({"ok": true})),
        other => panic!("a reply must route to its id, got {}", name(&other)),
    }
}

#[test]
fn a_reply_with_no_result_field_is_a_null_answer_not_an_error() {
    // rust-analyzer answers `definition` with no result when it knows of no definition.
    let message = json!({"jsonrpc": "2.0", "id": 3});
    match classify(&message) {
        Routed::Reply(3, Ok(value)) => assert_eq!(value, Value::Null),
        other => panic!("an empty answer is still an answer, got {}", name(&other)),
    }
}

#[test]
fn a_server_error_reaches_the_waiter_as_an_error() {
    let message = json!({"jsonrpc": "2.0", "id": 4, "error": {"code": -32601, "message": "unknown"}});
    match classify(&message) {
        Routed::Reply(4, Err(LspError::Server { code, message })) => {
            assert_eq!(code, -32601);
            assert_eq!(message, "unknown");
        }
        other => panic!("an error reply must carry its reason, got {}", name(&other)),
    }
}

#[test]
fn a_server_request_with_a_string_id_is_still_a_request() {
    // tsgo numbers its own requests "ts1", "ts2", ... and waits for the answer before it serves.
    let message = json!({"jsonrpc": "2.0", "id": "ts1", "method": "workspace/configuration", "params": {"items": [{}]}});
    match classify(&message) {
        Routed::Request { id, method, .. } => {
            assert_eq!(id, json!("ts1"), "the answer must carry the id as it came");
            assert_eq!(method, "workspace/configuration");
        }
        other => panic!("a request with a string id must be answered, got {}", name(&other)),
    }
}

#[test]
fn a_request_from_the_server_is_not_mistaken_for_a_reply() {
    // It has an id and a method. Routing it as a reply would wake the wrong waiter.
    let message = json!({"jsonrpc": "2.0", "id": 1, "method": "workspace/configuration", "params": {"items": [{}]}});
    match classify(&message) {
        Routed::Request { id, method, params } => {
            assert_eq!((id, method.as_str()), (json!(1), "workspace/configuration"));
            assert_eq!(params, json!({"items": [{}]}));
        }
        other => panic!("a server request must reach the worker to be answered, got {}", name(&other)),
    }
}

#[test]
fn a_request_for_settings_gets_one_empty_answer_per_item() {
    let answer = answer_for("workspace/configuration", &json!({"items": [{}, {}]}), Path::new("/"));
    assert_eq!(answer, Ok(json!([null, null])));
}

#[test]
fn a_registration_is_accepted_and_an_unknown_request_is_refused() {
    assert_eq!(answer_for("client/registerCapability", &json!({}), Path::new("/")), Ok(Value::Null));
    assert_eq!(answer_for("window/workDoneProgress/create", &json!({}), Path::new("/")), Ok(Value::Null));
    assert!(matches!(answer_for("window/showDocument", &json!({}), Path::new("/")), Err((METHOD_NOT_FOUND, _))));
}

#[test]
fn a_request_for_the_workspace_folders_gets_the_root() {
    let root = std::env::temp_dir();
    let answer = answer_for("workspace/workspaceFolders", &json!(null), &root).expect("the root exists");
    assert_eq!(answer[0]["uri"], json!(path_to_uri(&root).unwrap().as_str()));
}

#[test]
fn a_uri_escaped_either_way_names_the_same_path() {
    let plain: Uri = "file:///work/@scope/x%20y.ts".parse().unwrap();
    let escaped: Uri = "file:///work/%40scope/x%20y.ts".parse().unwrap();
    assert_eq!(uri_to_path(&plain), Some(PathBuf::from("/work/@scope/x y.ts")));
    assert_eq!(uri_to_path(&plain), uri_to_path(&escaped), "rust-analyzer and vscode-uri servers escape @ differently");
    assert_eq!(uri_to_path(&"jdt://contents/x".parse().unwrap()), None, "not a file");
}

#[test]
fn diagnostics_come_through_as_their_own_message() {
    let message = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {"uri": "file:///tmp/a.rs", "diagnostics": []},
    });
    match classify(&message) {
        Routed::Server(ServerMessage::Diagnostics(params)) => {
            assert_eq!(params.uri.as_str(), "file:///tmp/a.rs");
            assert!(params.diagnostics.is_empty());
        }
        other => panic!("diagnostics must reach the caller, got {}", name(&other)),
    }
}

#[test]
fn a_log_line_comes_through_with_its_text() {
    let message = json!({
        "jsonrpc": "2.0",
        "method": "window/logMessage",
        "params": {"type": 3, "message": "indexing"},
    });
    match classify(&message) {
        Routed::Server(ServerMessage::Log(text)) => assert_eq!(text, "indexing"),
        other => panic!("a log must reach the caller, got {}", name(&other)),
    }
}

#[test]
fn a_notification_we_do_not_read_is_dropped() {
    let message = json!({"jsonrpc": "2.0", "method": "$/progress", "params": {}});
    assert!(matches!(classify(&message), Routed::Ignore));
}

#[test]
fn a_path_becomes_a_file_uri_with_its_spaces_encoded() {
    let dir = std::env::temp_dir().join("atelier lsp uri test");
    std::fs::create_dir_all(&dir).expect("the temp dir is writable");
    let uri = path_to_uri(&dir).expect("a real path has a uri");
    assert!(uri.as_str().starts_with("file:///"), "got {uri:?}");
    assert!(uri.as_str().contains("%20"), "a space must be encoded, got {uri:?}");
    std::fs::remove_dir_all(&dir).ok();
}

/// The bug this catches: we escaped every byte outside a tiny set, so a path holding `+` went out as
/// `%2B`. A server built on the `url` crate publishes it raw, `fresh_enough` compares the two URIs
/// byte for byte, and the caller then waited out its whole timeout while the answer sat unread.
#[test]
fn a_path_keeps_the_characters_a_server_leaves_raw() {
    let dir = std::env::temp_dir().join("atelier+lsp,raw=chars!(one)");
    std::fs::create_dir_all(&dir).expect("the temp dir is writable");
    let uri = path_to_uri(&dir).expect("a real path has a uri");
    let text = uri.as_str();
    for raw in ["+", ",", "=", "!", "(", ")"] {
        assert!(text.contains(raw), "{raw} must stay raw, got {text}");
    }
    assert!(!text.contains('%'), "nothing in this path needs encoding, got {text}");
    std::fs::remove_dir_all(&dir).ok();
}

/// A percent sign is the escape character itself, so it must always be encoded. Leaving it raw
/// would turn a real `%2B` in a filename into a `+` at the server.
#[test]
fn a_percent_sign_in_a_path_is_always_encoded() {
    let dir = std::env::temp_dir().join("atelier%2Blsp");
    std::fs::create_dir_all(&dir).expect("the temp dir is writable");
    let uri = path_to_uri(&dir).expect("a real path has a uri");
    assert!(uri.as_str().contains("%252B"), "the percent must be encoded, got {uri:?}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_path_that_does_not_exist_has_no_uri() {
    assert!(path_to_uri(Path::new("/no/such/path/at/all")).is_err());
}

fn name(routed: &Routed) -> &'static str {
    match routed {
        Routed::Reply(..) => "a reply",
        Routed::Request { .. } => "a server request",
        Routed::Server(..) => "a server message",
        Routed::Ignore => "an ignored message",
    }
}


#[test]
fn a_server_status_says_whether_it_is_quiet() {
    let routed = classify(&json!({
        "jsonrpc": "2.0",
        "method": "experimental/serverStatus",
        "params": { "health": "ok", "quiescent": false, "message": "Loading" }
    }));
    assert!(matches!(routed, Routed::Server(ServerMessage::Status { quiescent: false })));
    let without = classify(&json!({ "jsonrpc": "2.0", "method": "experimental/serverStatus", "params": {} }));
    assert_eq!(name(&without), "an ignored message", "a status with no quiescent field tells us nothing");
}

#[test]
fn a_diagnostic_pull_leaves_out_the_fields_it_does_not_set() {
    // tsgo refuses `"identifier": null`: the field is optional, so an unset one must be absent.
    let uri: lsp_types::Uri = "file:///tmp/a.ts".parse().unwrap();
    let params = serde_json::to_value(PullDiagnosticsParams { text_document: TextDocumentIdentifier { uri } }).unwrap();
    assert_eq!(params, json!({ "textDocument": { "uri": "file:///tmp/a.ts" } }));
}

#[test]
fn the_read_loop_answers_a_server_request_itself() {
    // tsgo asks for its configuration and answers nothing until it hears back. The worker may be
    // blocked waiting on tsgo, so the reader, which never blocks on the worker, must answer.
    let mut input = Vec::new();
    let request = json!({"jsonrpc": "2.0", "id": "ts1", "method": "workspace/configuration", "params": {"items": [{}, {}]}});
    crate::framing::write_message(&mut input, &serde_json::to_vec(&request).unwrap()).unwrap();
    let written = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::channel();
    read_loop(std::io::Cursor::new(input), Pending::default(), tx, Arc::clone(&written), PathBuf::from("/tmp"));

    let written = written.lock().unwrap().clone();
    let body = crate::framing::read_message(&mut std::io::Cursor::new(written)).unwrap().expect("one answer went out");
    let answer: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(answer, json!({"jsonrpc": "2.0", "id": "ts1", "result": [null, null]}));
    assert!(matches!(rx.try_recv(), Ok(ServerMessage::Log(line)) if line.contains("workspace/configuration")));
    assert!(matches!(rx.try_recv(), Ok(ServerMessage::Exited)));
}
