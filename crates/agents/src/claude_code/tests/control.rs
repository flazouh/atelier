use serde_json::{Value, json};

use crate::{claude_code::control, session::{Attachment, PermissionMode}};

fn parse(line: &str) -> Value {
    assert!(!line.contains('\n'), "one line");
    serde_json::from_str(line).expect("valid JSON")
}

#[test]
fn a_user_message_is_a_user_line_with_its_id_and_the_text() {
    let line = parse(&control::user_message("m1", "fix \"it\"\nnow", &[]));
    assert_eq!(line, json!({"type": "user", "uuid": "m1", "message": {"role": "user", "content": "fix \"it\"\nnow"}}));
}

#[test]
fn a_message_id_has_the_uuid_form_and_differs_by_number() {
    let id = control::message_id(0xabc, 7);
    assert_eq!(id, "00000abc-0000-4000-8000-000000000007");
    assert_ne!(control::message_id(0xabc, 8), id);
    let ids = control::MessageIds::new();
    assert_ne!(ids.next(), ids.next());
}

#[test]
fn control_requests_carry_their_id_and_subtype() {
    assert_eq!(parse(&control::interrupt("a"))["request"], json!({"subtype": "interrupt"}));
    assert_eq!(parse(&control::interrupt("a"))["request_id"], "a");
    assert_eq!(parse(&control::set_model("b", "opus"))["request"], json!({"subtype": "set_model", "model": "opus"}));
    let mode = parse(&control::set_permission_mode("c", PermissionMode::AcceptEdits));
    assert_eq!(mode["request"], json!({"subtype": "set_permission_mode", "mode": "acceptEdits"}));
}

#[test]
fn permission_modes_round_trip_through_claudes_names() {
    for mode in [PermissionMode::Ask, PermissionMode::AcceptEdits, PermissionMode::Plan, PermissionMode::Auto, PermissionMode::Bypass] {
        assert_eq!(control::mode_from_name(control::mode_name(mode)), Some(mode));
    }
    assert_eq!(control::mode_from_name("manual"), Some(PermissionMode::Ask));
    assert_eq!(control::mode_from_name("dontAsk"), None);
}

#[test]
fn a_message_carries_its_attachments_as_text_after_its_own() {
    let comment = Attachment::LineComment {
        path: "src/a.rs".into(),
        first_line: 10,
        last_line: 12,
        removed: false,
        quote: "let a = 1;\nlet b = 2;".into(),
        body: "Why not a struct?".into(),
    };
    let line = parse(&control::user_message("m1", "Please look", &[comment, Attachment::File { path: "src/b.rs".into() }]));
    assert_eq!(
        line["message"]["content"],
        "Please look\n\nReview comment on src/a.rs, lines 10-12:\n> let a = 1;\n> let b = 2;\nWhy not a struct?\n\nFile: src/b.rs"
    );
}

#[test]
fn the_request_for_the_context_carries_the_prefix_its_answer_is_told_by() {
    let line = parse(&control::context_usage(&format!("{}1", control::CONTEXT_REQUEST)));
    assert_eq!(line["request"], json!({"subtype": "get_context_usage"}));
    assert!(line["request_id"].as_str().unwrap().starts_with(control::CONTEXT_REQUEST));
}
