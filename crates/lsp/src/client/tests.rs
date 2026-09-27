use super::*;

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
fn a_request_from_the_server_is_not_mistaken_for_a_reply() {
    // It has an id and a method. Routing it as a reply would wake the wrong waiter.
    let message = json!({"jsonrpc": "2.0", "id": 1, "method": "workspace/configuration", "params": {}});
    assert!(matches!(classify(&message), Routed::Ignore));
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
    let dir = std::env::temp_dir().join("lathe lsp uri test");
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
    let dir = std::env::temp_dir().join("lathe+lsp,raw=chars!(one)");
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
    let dir = std::env::temp_dir().join("lathe%2Blsp");
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
        Routed::Server(..) => "a server message",
        Routed::Ignore => "an ignored message",
    }
}

fn uri(path: &str) -> Uri {
    path.parse().expect("a literal uri parses")
}

#[test]
fn a_set_for_another_file_is_not_the_one_we_waited_for() {
    assert!(!fresh_enough(&uri("file:///b.rs"), Some(2), &uri("file:///a.rs"), Some(2)));
}

#[test]
fn a_set_the_server_published_before_our_edit_is_stale() {
    // The empty set from indexing carries version 1; we asked about version 2.
    assert!(!fresh_enough(&uri("file:///a.rs"), Some(1), &uri("file:///a.rs"), Some(2)));
}

#[test]
fn a_set_for_our_edit_or_a_later_one_is_fresh() {
    assert!(fresh_enough(&uri("file:///a.rs"), Some(2), &uri("file:///a.rs"), Some(2)));
    assert!(fresh_enough(&uri("file:///a.rs"), Some(3), &uri("file:///a.rs"), Some(2)));
}

#[test]
fn a_server_that_sends_no_version_is_taken_at_its_word() {
    assert!(fresh_enough(&uri("file:///a.rs"), None, &uri("file:///a.rs"), Some(2)));
}

#[test]
fn a_caller_that_asks_for_no_version_takes_the_next_set() {
    assert!(fresh_enough(&uri("file:///a.rs"), Some(1), &uri("file:///a.rs"), None));
}
