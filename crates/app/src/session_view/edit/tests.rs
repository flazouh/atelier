use atelier_agents::session::{Call, FileEdit, ToolCall, ToolId, ToolKind, ToolStatus};
use atelier_ui::tool_preview::ToolPreview;

use super::*;

const ROOT: &str = "/w";

fn call(name: &str, kind: ToolKind, edit: Option<FileEdit>, status: ToolStatus) -> Call {
    let input = serde_json::Value::Null;
    Call { call: ToolCall { id: ToolId::new("t"), name: name.into(), kind, input, file: None, parent: None, status }, output: None, edit }
}

fn change(old: &str, new: &str) -> Option<FileEdit> {
    Some(FileEdit { path: "/w/a.rs".into(), old: old.into(), new: new.into() })
}

fn edit(status: ToolStatus) -> Call {
    call("Edit", ToolKind::Edit, change("a", "b"), status)
}

#[test]
fn an_edit_is_its_diff_open_while_it_streams_in_every_density() {
    for density in ToolDensity::ALL {
        let view = edit_view(&edit(ToolStatus::Running), ROOT, density, None).unwrap();
        assert_eq!(view.preview, ToolPreview::edit("a.rs", "a", "b"));
        assert!(view.streaming && view.open, "{density:?}");
    }
}

#[test]
fn a_finished_edit_is_shut_unless_the_density_is_full_detail() {
    let done = edit(ToolStatus::Done);
    for density in [ToolDensity::Grouped, ToolDensity::Lines] {
        let view = edit_view(&done, ROOT, density, None).unwrap();
        assert!(!view.streaming && !view.open && view.fold_when_done, "{density:?}");
    }
    let view = edit_view(&done, ROOT, ToolDensity::Detailed, None).unwrap();
    assert!(!view.streaming && view.open && !view.fold_when_done);
}

/// The view reads the shared edit and the kind, never the agent's names: Cursor's "Edit File", the own agent's
/// "write" and Claude's "Write" all show the same way.
#[test]
fn any_agents_edit_or_write_shows_by_its_kind_whatever_its_name() {
    for name in ["Edit", "Edit File", "edit"] {
        let view = edit_view(&call(name, ToolKind::Edit, change("a", "b"), ToolStatus::Done), ROOT, ToolDensity::Lines, None).unwrap();
        assert_eq!(view.preview, ToolPreview::edit("a.rs", "a", "b"), "{name}");
    }
    for name in ["Write", "write"] {
        let view = edit_view(&call(name, ToolKind::Write, change("", "hi"), ToolStatus::Running), ROOT, ToolDensity::Lines, None).unwrap();
        assert_eq!(view.preview, ToolPreview::written("a.rs", "hi"), "{name}");
    }
}

#[test]
fn a_call_with_no_edit_or_that_failed_or_was_refused_has_no_diff() {
    assert_eq!(edit_view(&call("Bash", ToolKind::Shell, None, ToolStatus::Done), ROOT, ToolDensity::Lines, None), None);
    assert_eq!(edit_view(&call("Edit", ToolKind::Edit, None, ToolStatus::Running), ROOT, ToolDensity::Lines, None), None, "no text told yet");
    assert_eq!(edit_view(&edit(ToolStatus::Failed), ROOT, ToolDensity::Lines, None), None, "the row says why");
    assert_eq!(edit_view(&edit(ToolStatus::Done), ROOT, ToolDensity::Lines, Some("Denied")), None);
    assert_eq!(edit_view(&edit(ToolStatus::Done), ROOT, ToolDensity::Lines, Some("Not answered")), None);
    assert!(edit_view(&edit(ToolStatus::Done), ROOT, ToolDensity::Lines, Some("Approved")).is_some());
}
