use serde_json::{Value, json};

use crate::{claude_code::control, session::PermissionMode};

fn parse(line: &str) -> Value {
    assert!(!line.contains('\n'), "one line");
    serde_json::from_str(line).expect("valid JSON")
}

#[test]
fn a_user_message_is_a_user_line_with_the_text() {
    let line = parse(&control::user_message("fix \"it\"\nnow"));
    assert_eq!(line, json!({"type": "user", "message": {"role": "user", "content": "fix \"it\"\nnow"}}));
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
