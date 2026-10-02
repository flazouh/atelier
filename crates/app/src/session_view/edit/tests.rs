use atelier_agents::session::{Call, ToolCall, ToolId, ToolKind, ToolStatus};
use atelier_ui::tool_preview::ToolPreview;
use serde_json::json;

use super::*;

const ROOT: &str = "/w";

fn call(name: &str, input: serde_json::Value, status: ToolStatus) -> Call {
    let kind = if name == "Write" { ToolKind::Write } else if name == "Bash" { ToolKind::Shell } else { ToolKind::Edit };
    Call { call: ToolCall { id: ToolId::new("t"), name: name.into(), kind, input, file: None, parent: None, status }, output: None }
}

fn edit(status: ToolStatus) -> Call {
    call("Edit", json!({"file_path": "/w/a.rs", "old_string": "a", "new_string": "b"}), status)
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

#[test]
fn a_write_that_has_its_file_shows_the_text_so_far() {
    let write = call("Write", json!({"file_path": "/w/new.txt", "content": "hi"}), ToolStatus::Running);
    assert_eq!(edit_view(&write, ROOT, ToolDensity::Lines, None).unwrap().preview, ToolPreview::written("new.txt", "hi"));
}

#[test]
fn a_call_that_is_no_edit_or_has_no_file_or_failed_or_was_refused_has_no_diff() {
    assert_eq!(edit_view(&call("Bash", json!({"command": "ls"}), ToolStatus::Done), ROOT, ToolDensity::Lines, None), None);
    assert_eq!(edit_view(&call("Edit", json!({}), ToolStatus::Running), ROOT, ToolDensity::Lines, None), None, "no file yet");
    assert_eq!(edit_view(&edit(ToolStatus::Failed), ROOT, ToolDensity::Lines, None), None, "the row says why");
    assert_eq!(edit_view(&edit(ToolStatus::Done), ROOT, ToolDensity::Lines, Some("Denied")), None);
    assert_eq!(edit_view(&edit(ToolStatus::Done), ROOT, ToolDensity::Lines, Some("Not answered")), None);
    assert!(edit_view(&edit(ToolStatus::Done), ROOT, ToolDensity::Lines, Some("Approved")).is_some());
}
