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
    assert_eq!(cx.update(|_, cx| pane.read(cx).reviewed(cx).len()), 1, "a file with nothing left is reviewed");
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
    assert_eq!(cx.update(|_, cx| session.read(cx).reviews.comments.all().len()), 1);
    fake.turns.lock().unwrap().push(vec![ended()]);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("see my comment".into(), cx)));
    cx.run_until_parked();
    let sent = fake.received.lock().unwrap().iter().rev().find_map(|c| match c {
        Command::Send { attachments, .. } => Some(attachments.clone()),
        _ => None,
    });
    let sent = sent.unwrap();
    assert!(matches!(&sent[..], [Attachment::LineComment { path, first_line: 2, body, .. }] if path == "a.txt" && body == "Why upper case?"), "{sent:?}");
    let s = cx.update(|_, cx| (session.read(cx).reviews.comments.all().len(), session.read(cx).reviews.sent.clone()));
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
    assert_eq!(cx.update(|_, cx| pane.read(cx).reviewed(cx).len()), 1);
    cx.update(|window, cx| pane.update(cx, |p, cx| p.switch_scope(window, cx)));
    assert_eq!(cx.update(|_, cx| pane.read(cx).reviewed(cx).len()), 0, "the mark was on the whole session, not on the turn");
}

/// Shows the pane as the window's root, so it is laid out and painted.
struct Shown(Entity<ReviewPane>);

impl gpui_kit::Render for Shown {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.0.clone())
    }
}

/// The gutter's + opens a composer that takes the keys at once: it is painted in a row block, so it can
/// only take focus once it has been painted.
#[gpui_kit::test]
fn a_new_comment_takes_the_keys_at_once(cx: &mut TestAppContext) {
    let (pane, _, _, _, cx) = reviewing(cx);
    let shown = pane.clone();
    cx.update(|window, cx| _ = window.replace_root(cx, |_, _| Shown(shown)));
    cx.run_until_parked();
    cx.update(|window, cx| pane.update(cx, |p, cx| p.open_composer(2, window, cx)));
    cx.run_until_parked();
    let focused = cx.update(|window, cx| pane.read(cx).composer.as_ref().is_some_and(|(_, c, _)| c.focus_handle(cx).is_focused(window)));
    assert!(focused, "the composer has the keys");
}

/// What is typed in a new comment's composer is what it sends, and its key sends it.
#[gpui_kit::test]
fn a_comment_typed_and_sent_with_its_key_is_kept_whole(cx: &mut TestAppContext) {
    let (pane, session, _, _, cx) = reviewing(cx);
    let shown = pane.clone();
    cx.update(|window, cx| _ = window.replace_root(cx, |_, _| Shown(shown)));
    cx.run_until_parked();
    cx.update(|window, cx| pane.update(cx, |p, cx| p.open_composer(2, window, cx)));
    cx.run_until_parked();
    cx.simulate_input("Why upper case?");
    cx.run_until_parked();
    cx.simulate_keystrokes("secondary-enter");
    cx.run_until_parked();
    let bodies = cx.update(|_, cx| session.read(cx).reviews.comments.all().iter().map(|c| c.body.clone()).collect::<Vec<_>>());
    assert_eq!(bodies, ["Why upper case?"]);
}

/// The composer sits in an editor row block, and a row block is drawn only when the editor is: what is
/// typed in it shows only if each change of the composer draws the editor again.
#[gpui_kit::test]
fn typing_in_a_comment_draws_the_editor_again(cx: &mut TestAppContext) {
    let (pane, _, _, _, cx) = reviewing(cx);
    let shown = pane.clone();
    cx.update(|window, cx| _ = window.replace_root(cx, |_, _| Shown(shown)));
    cx.run_until_parked();
    cx.update(|window, cx| pane.update(cx, |p, cx| p.open_composer(2, window, cx)));
    cx.run_until_parked();
    let drawn = std::rc::Rc::new(std::cell::Cell::new(0));
    let editor = cx.update(|_, cx| pane.read(cx).editor.clone());
    let count = drawn.clone();
    let _watch = cx.update(|_, cx| cx.observe(&editor, move |_, _| count.set(count.get() + 1)));
    cx.simulate_input("a");
    cx.run_until_parked();
    assert!(drawn.get() > 0, "the editor was asked to draw again");
}

/// A decision that leaves the disk as it was (an accept) is kept too: the review opens again as it was
/// left.
#[gpui_kit::test]
fn a_review_opened_again_keeps_an_accepted_hunk(cx: &mut TestAppContext) {
    let (pane, session, _, dir, cx) = reviewing(cx);
    let ids = hunk_ids(&pane, cx);
    cx.update(|window, cx| pane.update(cx, |p, cx| {
        p.decide_hunk(&ids[0], Decision::Accept, window, cx);
    }));
    cx.run_until_parked();
    assert_eq!(read(&dir, "a.txt"), AFTER, "an accept writes nothing");
    let project: Arc<dyn Project> = Arc::new(lathe_project::LocalProject::open(&dir).unwrap());
    let again = cx.update(|window, cx| cx.new(|cx| ReviewPane::new(session.clone(), project, Scope::Turn(0), Some("a.txt"), window, cx)));
    cx.run_until_parked();
    assert_eq!(hunk_ids(&again, cx), [ids[1].clone()], "only the hunk not decided is left");
}

/// The pane makes a new editor for each file and each scope; when the reader's keys were in the old
/// one, they go to the new one, so the next key still reaches the review.
#[gpui_kit::test]
fn the_keys_follow_the_editor_to_the_next_scope(cx: &mut TestAppContext) {
    let (pane, _, _, _, cx) = reviewing(cx);
    let shown = pane.clone();
    cx.update(|window, cx| _ = window.replace_root(cx, |_, _| Shown(shown)));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let editor = pane.read(cx).editor.clone();
        editor.update(cx, |e, cx| e.focus(window, cx));
    });
    cx.run_until_parked();
    cx.update(|window, cx| pane.update(cx, |p, cx| p.switch_scope(window, cx)));
    cx.run_until_parked();
    let focused = cx.update(|window, cx| pane.read(cx).editor.read(cx).focus_handle(cx).is_focused(window));
    assert!(focused, "the new editor has the keys");
}

/// Typing is written a moment after the last key; a pane closed in that moment still writes it.
#[gpui_kit::test]
fn typing_just_before_the_review_closes_reaches_the_disk(cx: &mut TestAppContext) {
    let (pane, _, _, dir, cx) = reviewing(cx);
    let at = editor_text(&pane, cx).find("TWO").unwrap() + 3;
    cx.update(|window, cx| {
        let editor = pane.read(cx).editor.clone();
        editor.update(cx, |e, cx| {
            e.set_selected_range(at..at, cx);
            e.insert("!", window, cx);
        });
    });
    drop(pane);
    // A dropped entity is released at the next flush of effects.
    cx.update(|_, _| ());
    cx.run_until_parked();
    assert!(read(&dir, "a.txt").contains("TWO!"), "{}", read(&dir, "a.txt"));
}

/// Rejecting a file the agent made removes it, instead of leaving it empty.
#[gpui_kit::test]
fn a_rejected_new_file_is_removed(cx: &mut TestAppContext) {
    let dir = git_project(&[("a.txt", BEFORE)]);
    let (session, fake, cx) = start_in(cx, dir.clone(), vec![vec![ended()]], false);
    let root = dir.clone();
    fake.work.lock().unwrap().push(Box::new(move || std::fs::write(root.join("new.txt"), "made\n").unwrap()));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("make a file".into(), cx)));
    cx.run_until_parked();
    let project: Arc<dyn Project> = Arc::new(lathe_project::LocalProject::open(&dir).unwrap());
    let pane = cx.update(|window, cx| cx.new(|cx| ReviewPane::new(session.clone(), project, Scope::Turn(0), Some("new.txt"), window, cx)));
    cx.run_until_parked();
    cx.update(|window, cx| pane.update(cx, |p, cx| p.decide_file(Decision::Reject, window, cx)));
    cx.run_until_parked();
    assert!(!dir.join("new.txt").exists(), "the file the agent made is gone");
}

/// The whole session is diffed off the UI thread: the switch keeps the turn drawn until its files land.
#[gpui_kit::test]
fn the_whole_session_is_read_off_the_ui_thread(cx: &mut TestAppContext) {
    let (pane, _, _, _, cx) = reviewing(cx);
    cx.update(|window, cx| pane.update(cx, |p, cx| p.switch_scope(window, cx)));
    assert_eq!(cx.update(|_, cx| pane.read(cx).scope), Scope::Turn(0), "the turn stays until the session's files land");
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| pane.read(cx).scope), Scope::Whole);
    assert_eq!(hunk_ids(&pane, cx).len(), 2);
}

/// The review opens its file once: the language server's session attaches to the editor the pane
/// made, rather than a second editor made for it.
#[gpui_kit::test]
fn a_review_with_a_language_server_opens_its_file_once(cx: &mut TestAppContext) {
    let (pane, _, _, dir, cx) = reviewing(cx);
    let project: Arc<dyn Project> = Arc::new(lathe_project::LocalProject::open(&dir).unwrap());
    let workers = Arc::new(lathe_lsp::Workers::new(project, lathe_lsp::Store::from_env(), lathe_editor::READY, lathe_editor::ASK));
    let given = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = given.clone();
    let language: SessionFor = std::rc::Rc::new(move |path, editor, rows, cx| {
        seen.borrow_mut().push(editor.entity_id());
        cx.new(|cx| lathe_editor::EditorSession::for_review(workers.clone(), editor, dir.join(path), rows, None, cx))
    });
    let before = cx.update(|_, cx| pane.read(cx).editor.entity_id());
    cx.update(|window, cx| pane.update(cx, |p, cx| p.attach_language(language, window, cx)));
    cx.run_until_parked();
    let after = cx.update(|_, cx| pane.read(cx).editor.entity_id());
    assert_eq!(after, before, "the same editor");
    assert_eq!(*given.borrow(), [before], "one session, on that editor");
}

#[gpui_kit::test]
fn the_comment_key_opens_the_composer_on_the_row_of_the_caret(cx: &mut TestAppContext) {
    use gpui_kit::component::input::Position;
    let (pane, _, _, _, cx) = reviewing(cx);
    let handler = cx.update(|_, cx| pane.update(cx, |p, cx| p.handlers(cx))).for_command(beui::keys::Command::Comment).cloned().expect("the pane answers the comment key");
    assert!(cx.update(|_, cx| pane.read(cx).composer.is_none()));
    cx.update(|window, cx| {
        let editor = pane.read(cx).editor.clone();
        editor.update(cx, |e, cx| e.set_cursor_position(Position { line: 2, character: 0 }, window, cx));
        handler(window, cx);
    });
    cx.run_until_parked();
    let row = cx.update(|_, cx| pane.read(cx).composer.as_ref().map(|(_, composer, _)| composer.read(cx).row()));
    assert_eq!(row, Some(2), "the composer is on the caret's row");
}
/// A file with nothing left to decide says how it was decided, and Undo brings back its last decision,
/// on disk too.
#[gpui_kit::test]
fn a_decided_file_says_so_and_undo_brings_it_back(cx: &mut TestAppContext) {
    let (pane, _, _, dir, cx) = reviewing(cx);
    assert_eq!(cx.update(|_, cx| pane.read(cx).decided_words(0)), None, "hunks wait");
    let ids = hunk_ids(&pane, cx);
    cx.update(|window, cx| pane.update(cx, |p, cx| {
        for id in &ids {
            p.decide_hunk(id, Decision::Accept, window, cx).unwrap();
        }
    }));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| pane.read(cx).decided_words(0)).as_deref(), Some("Accepted"));
    cx.update(|window, cx| pane.update(cx, |p, cx| p.undo_decision(window, cx)));
    cx.run_until_parked();
    assert_eq!(hunk_ids(&pane, cx).len(), 1, "the last accept is undone");
    assert_eq!(cx.update(|_, cx| pane.read(cx).decided_words(0)), None);
    let left = hunk_ids(&pane, cx);
    cx.update(|window, cx| pane.update(cx, |p, cx| p.decide_hunk(&left[0], Decision::Reject, window, cx).unwrap()));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| pane.read(cx).decided_words(0)).as_deref(), Some("Decided"), "one accepted, one rejected");
    assert_eq!(read(&dir, "a.txt"), "1\nTWO\n3\n4\n5\n6\n7\n8\n");
    cx.update(|window, cx| pane.update(cx, |p, cx| p.undo_decision(window, cx)));
    cx.run_until_parked();
    assert_eq!(read(&dir, "a.txt"), AFTER, "the undone reject is back on disk");
    assert!(editor_text(&pane, cx).contains("SEVEN"), "the editor shows it too");
}
/// Rejecting every hunk reads "Rejected".
#[gpui_kit::test]
fn a_file_rejected_whole_says_rejected(cx: &mut TestAppContext) {
    let (pane, _, _, _dir, cx) = reviewing(cx);
    let ids = hunk_ids(&pane, cx);
    cx.update(|window, cx| pane.update(cx, |p, cx| {
        for id in &ids {
            p.decide_hunk(id, Decision::Reject, window, cx).unwrap();
        }
    }));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| pane.read(cx).decided_words(0)).as_deref(), Some("Rejected"));
}
