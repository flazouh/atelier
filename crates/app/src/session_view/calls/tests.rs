use serde_json::Value;
use atelier_agents::session::{Answer, Call, ChoiceKind, Item, PermissionRequest, RequestId, ToolCall, ToolId, ToolKind, ToolStatus};
use super::*;
fn tool(id: &str) -> Item {
    let call = ToolCall { id: ToolId::new(id), name: "Edit".into(), kind: ToolKind::Edit, input: Value::Null, file: None, parent: None, status: ToolStatus::Running };
    Item::Tool(Call { call, output: None })
}
fn asked(id: &str, answer: Answer) -> Item {
    let call = match tool(id) {
        Item::Tool(c) => c.call,
        _ => unreachable!(),
    };
    Item::Permission { request: PermissionRequest { id: RequestId::new(format!("r-{id}")), call, reason: None, choices: Vec::new() }, answer }
}
/// While the approval waits, it stands for the call and the row hides; once answered, the approval
/// hides and the row carries the answer as a mark. Other rows are left alone.
#[test]
fn a_call_shows_once_whatever_its_approval_says() {
    let items = vec![tool("a"), asked("a", Answer::Asking), tool("b"), asked("b", Answer::Answered(ChoiceKind::Allow)), tool("c")];
    let shows: Vec<bool> = (0..items.len()).map(|ix| shows(&items, ix)).collect();
    assert_eq!(shows, [false, true, true, false, true]);
    assert_eq!(mark(&items, &ToolId::new("b")), Some("Approved"));
    assert_eq!(mark(&items, &ToolId::new("c")), None);
    assert_eq!(mark(&items, &ToolId::new("a")), None, "still asking");
}
/// Each answer's mark.
#[test]
fn each_answer_has_its_mark() {
    let with = |answer| vec![tool("a"), asked("a", answer)];
    assert_eq!(mark(&with(Answer::Answered(ChoiceKind::AllowAlways)), &ToolId::new("a")), Some("Always allowed"));
    assert_eq!(mark(&with(Answer::Answered(ChoiceKind::Deny)), &ToolId::new("a")), Some("Denied"));
    assert_eq!(mark(&with(Answer::Withdrawn), &ToolId::new("a")), Some("Not answered"));
}
/// A resumed session has no approvals in its history: the row's mark comes from the answers the
/// session's record kept, and a live approval still wins.
#[test]
fn a_resumed_call_keeps_its_mark_from_the_record() {
    let mut kept = std::collections::HashMap::new();
    kept.insert("a".to_string(), crate::review_state::Approval::Approved);
    kept.insert("b".to_string(), crate::review_state::Approval::Denied);
    let items = vec![tool("a"), tool("b"), asked("b", Answer::Answered(ChoiceKind::AllowAlways))];
    assert_eq!(mark_kept(&items, &ToolId::new("a"), &kept), Some("Approved"));
    assert_eq!(mark_kept(&items, &ToolId::new("b"), &kept), Some("Always allowed"), "the live answer wins");
    assert_eq!(mark_kept(&items, &ToolId::new("c"), &kept), None);
}
