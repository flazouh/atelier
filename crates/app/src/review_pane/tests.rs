use std::path::Path;

use gpui_kit::TestAppContext;
use lathe_agents::session::{Attachment, Command};

use super::*;
use crate::fake_agent::{ended, git_project, start_in};

const BEFORE: &str = "1\n2\n3\n4\n5\n6\n7\n8\n";
const AFTER: &str = "1\nTWO\n3\n4\n5\n6\nSEVEN\n8\n";

fn read(dir: &Path, path: &str) -> String {
    std::fs::read_to_string(dir.join(path)).unwrap()
}

/// A session whose one turn changed a.txt in two places, and a review pane open on it.
fn reviewing(cx: &mut TestAppContext) -> (Entity<ReviewPane>, Entity<AgentSession>, std::sync::Arc<crate::fake_agent::Fake>, std::path::PathBuf, &mut gpui_kit::VisualTestContext) {
    let dir = git_project(&[("a.txt", BEFORE)]);
    let (session, fake, cx) = start_in(cx, dir.clone(), vec![vec![ended()]], false);
    let root = dir.clone();
    fake.work.lock().unwrap().push(Box::new(move || std::fs::write(root.join("a.txt"), AFTER).unwrap()));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("edit".into(), cx)));
    cx.run_until_parked();
    let project: Arc<dyn Project> = Arc::new(lathe_project::LocalProject::open(&dir).unwrap());
    let pane = cx.update(|window, cx| cx.new(|cx| ReviewPane::new(session.clone(), project, Scope::Turn(0), Some("a.txt"), window, cx)));
    cx.run_until_parked();
    (pane, session, fake, dir, cx)
}

fn hunk_ids(pane: &Entity<ReviewPane>, cx: &mut gpui_kit::VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| pane.read(cx).files[0].hunks().iter().map(|h| h.id.to_string()).collect())
}

fn editor_text(pane: &Entity<ReviewPane>, cx: &mut gpui_kit::VisualTestContext) -> String {
    cx.update(|_, cx| pane.read(cx).editor.read(cx).value().to_string())
}

#[gpui_kit::test]
fn a_hunk_accepted_and_one_rejected_reach_the_disk(cx: &mut TestAppContext) {
    let (pane, _, _, dir, cx) = reviewing(cx);
    let ids = hunk_ids(&pane, cx);
    assert_eq!(ids.len(), 2);
    cx.update(|window, cx| pane.update(cx, |p, cx| {
        p.decide_hunk(&ids[0], Decision::Accept, window, cx).unwrap();
        p.decide_hunk(&ids[1], Decision::Reject, window, cx).unwrap();
    }));
    cx.run_until_parked();
    assert_eq!(read(&dir, "a.txt"), "1\nTWO\n3\n4\n5\n6\n7\n8\n", "the accepted row stays, the rejected one goes back");
    assert_eq!(editor_text(&pane, cx), read(&dir, "a.txt"), "no hunk is left, so the buffer is the file");
    assert_eq!(cx.update(|_, cx| pane.read(cx).progress(cx).reviewed), 1, "a file with nothing left is reviewed");
}

#[gpui_kit::test]
fn an_edit_inside_a_hunk_is_what_accepting_it_writes(cx: &mut TestAppContext) {
    let (pane, _, _, dir, cx) = reviewing(cx);
    // The merged buffer: 1, 2, TWO, 3 ... The reader types at the end of "TWO".
    let text = editor_text(&pane, cx);
    let at = text.find("TWO").unwrap() + 3;
    cx.update(|window, cx| {
        let editor = pane.read(cx).editor.clone();
        editor.update(cx, |e, cx| {
            e.set_selected_range(at..at, cx);
            e.insert("!", window, cx);
        });
    });
    cx.run_until_parked();
    let ids = hunk_ids(&pane, cx);
    assert_eq!(ids.len(), 2, "the typing moved the hunk, it did not end it");
    cx.update(|window, cx| pane.update(cx, |p, cx| {
        p.decide_hunk(&ids[0], Decision::Accept, window, cx);
    }));
    cx.run_until_parked();
    assert!(read(&dir, "a.txt").starts_with("1\nTWO!\n3\n"), "{}", read(&dir, "a.txt"));
}

#[gpui_kit::test]
fn the_agent_writing_again_adds_hunks_and_keeps_the_caret(cx: &mut TestAppContext) {
    let (pane, _, _, dir, cx) = reviewing(cx);
    cx.update(|_, cx| {
        let editor = pane.read(cx).editor.clone();
        editor.update(cx, |e, cx| e.set_selected_range(1..1, cx));
    });
    std::fs::write(dir.join("a.txt"), "1\nTWO\n3\n4\n5\n6\nSEVEN\n8\nNINE\n").unwrap();
    cx.update(|window, cx| pane.update(cx, |p, cx| p.check_disk(vec!["a.txt".into()], window, cx)));
    cx.run_until_parked();
    assert_eq!(hunk_ids(&pane, cx).len(), 3, "the new row is a hunk of its own");
    assert!(editor_text(&pane, cx).ends_with("NINE\n"));
    assert_eq!(cx.update(|_, cx| pane.read(cx).editor.read(cx).cursor()), 1, "the caret stayed on its text");
}

#[gpui_kit::test]
fn a_rejected_file_goes_back_and_accept_all_ends_every_hunk(cx: &mut TestAppContext) {
    let (pane, _, _, dir, cx) = reviewing(cx);
    cx.update(|window, cx| pane.update(cx, |p, cx| p.decide_file(Decision::Reject, window, cx)));
    cx.run_until_parked();
    assert_eq!(read(&dir, "a.txt"), BEFORE);
    std::fs::write(dir.join("a.txt"), AFTER).unwrap();
    cx.update(|window, cx| pane.update(cx, |p, cx| p.check_disk(vec!["a.txt".into()], window, cx)));
    cx.run_until_parked();
    assert_eq!(hunk_ids(&pane, cx).len(), 2, "the agent wrote it again");
    cx.update(|window, cx| pane.update(cx, |p, cx| p.decide_every_file(Decision::Accept, window, cx)));
    cx.run_until_parked();
    assert!(hunk_ids(&pane, cx).is_empty());
    assert_eq!(read(&dir, "a.txt"), AFTER);
}

#[gpui_kit::test]
fn a_comment_goes_with_the_next_message_and_is_answered_by_its_turn(cx: &mut TestAppContext) {
    let (pane, session, fake, _, cx) = reviewing(cx);
    // Row 2 of the merged text is "TWO", line 2 of the file now.
    cx.update(|_, cx| pane.update(cx, |p, cx| p.comment(2, "Why upper case?", cx)));
    assert_eq!(cx.update(|_, cx| session.read(cx).comments.all().len()), 1);
    fake.turns.lock().unwrap().push(vec![ended()]);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("see my comment".into(), cx)));
    cx.run_until_parked();
    let sent = fake.received.lock().unwrap().iter().rev().find_map(|c| match c {
        Command::Send { attachments, .. } => Some(attachments.clone()),
        _ => None,
    });
    let sent = sent.unwrap();
    assert!(matches!(&sent[..], [Attachment::LineComment { path, first_line: 2, body, .. }] if path == "a.txt" && body == "Why upper case?"), "{sent:?}");
    let s = cx.update(|_, cx| (session.read(cx).comments.all().len(), session.read(cx).sent_comments.clone()));
    assert_eq!(s.0, 0, "sent, so no longer waiting");
    assert!(s.1.iter().all(|(_, answered)| *answered), "the agent's turn after it ended");
}

#[gpui_kit::test]
fn the_whole_session_holds_every_turn_and_marks_keep_per_scope(cx: &mut TestAppContext) {
    let (pane, session, fake, dir, cx) = reviewing(cx);
    let root = dir.clone();
    fake.work.lock().unwrap().push(Box::new(move || std::fs::write(root.join("b.txt"), "b\n").unwrap()));
    fake.turns.lock().unwrap().push(vec![ended()]);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("more".into(), cx)));
    cx.run_until_parked();
    cx.update(|window, cx| pane.update(cx, |p, cx| p.switch_scope(window, cx)));
    cx.run_until_parked();
    let paths = cx.update(|_, cx| pane.read(cx).files.iter().map(|f| f.review.path.clone()).collect::<Vec<_>>());
    assert_eq!(paths, ["a.txt", "b.txt"]);
    cx.update(|_, cx| pane.update(cx, |p, cx| p.toggle_mark(cx)));
    assert_eq!(cx.update(|_, cx| pane.read(cx).progress(cx).reviewed), 1);
    cx.update(|window, cx| pane.update(cx, |p, cx| p.switch_scope(window, cx)));
    assert_eq!(cx.update(|_, cx| pane.read(cx).progress(cx).reviewed), 0, "the mark was on the whole session, not on the turn");
}
