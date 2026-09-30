use gpui_kit::TestAppContext;
use lathe_agents::session::{Choice, ChoiceId, PermissionRequest, RequestId, ToolCall, ToolId, ToolKind, ToolStatus};

use super::*;
use crate::fake_agent::{ended, git_project, start, start_in};

fn ask() -> PermissionRequest {
    PermissionRequest {
        id: RequestId::new("r1"),
        call: ToolCall {
            id: ToolId::new("t1"),
            name: "Write".into(),
            kind: ToolKind::Write,
            input: serde_json::json!({ "file_path": "a.txt" }),
            file: Some("a.txt".into()),
            parent: None,
            status: ToolStatus::Pending,
        },
        reason: None,
        choices: vec![
            Choice { id: ChoiceId::new("yes"), label: "Allow".into(), kind: ChoiceKind::Allow },
            Choice { id: ChoiceId::new("always"), label: "Always".into(), kind: ChoiceKind::AllowAlways },
            Choice { id: ChoiceId::new("no"), label: "Deny".into(), kind: ChoiceKind::Deny },
        ],
    }
}

/// A message works, a question needs the reader, an answer sends the choice it offers, and the turn
/// ends finished while the reader looks elsewhere.
#[gpui_kit::test]
fn a_turn_asks_is_answered_and_ends_finished(cx: &mut TestAppContext) {
    let (session, fake, cx) = start(cx, vec![vec![Event::Permission(ask())]], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("write a.txt".into(), cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| session.read(cx).status.clone()), SessionStatus::NeedsYou(beui::session_status::Need::Approval));
    assert_eq!(cx.update(|_, cx| session.read(cx).title.to_string()), "write a.txt", "the first message names it");
    cx.update(|_, cx| session.update(cx, |s, cx| s.answer(&RequestId::new("r1"), ChoiceKind::AllowAlways, cx)));
    assert!(fake.received.lock().unwrap().iter().any(|c| matches!(c, Command::Answer { choice, .. } if choice.0 == "always")));
    assert_eq!(cx.update(|_, cx| session.read(cx).status.clone()), SessionStatus::Working);
    fake.turns.lock().unwrap().push(vec![ended()]);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("go on".into(), cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| session.read(cx).status.clone()), SessionStatus::Finished, "unseen: amber");
    cx.update(|_, cx| session.update(cx, |s, cx| s.set_seen(true, cx)));
    assert_eq!(cx.update(|_, cx| session.read(cx).status.clone()), SessionStatus::Idle, "opening clears it");
}

/// A session whose agent could not start tries again when the reader writes, and the message goes.
#[gpui_kit::test]
fn a_failed_start_is_retried_by_the_next_message(cx: &mut TestAppContext) {
    let (session, fake, cx) = start(cx, vec![vec![ended()]], true);
    assert!(cx.update(|_, cx| session.read(cx).problem.clone()).is_some_and(|p| p.contains("not installed")));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hello".into(), cx)));
    cx.run_until_parked();
    assert!(cx.update(|_, cx| session.read(cx).running()), "it started this time");
    assert!(fake.received.lock().unwrap().iter().any(|c| matches!(c, Command::Send { text, .. } if text == "hello")), "and the message went");
}

/// Streaming text joins into one row, and only the rows that changed are measured again.
#[gpui_kit::test]
fn a_stream_folds_into_one_row(cx: &mut TestAppContext) {
    let block = lathe_agents::session::BlockId(1);
    let turn = (0..50).map(|i| Event::Text { block, delta: format!("word{i} ") }).chain([ended()]).collect();
    let (session, _, cx) = start(cx, vec![turn], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("talk".into(), cx)));
    cx.run_until_parked();
    let (items, rows) = cx.update(|_, cx| {
        let s = session.read(cx);
        (s.conversation.items().len(), s.list.item_count())
    });
    assert_eq!(items, 2, "the message and one text");
    assert_eq!(rows, items, "the list holds a row for each");
}

/// What a long session costs: 2,000 messages (the reader's and the agent's, and a tool call every
/// tenth) folded and drawn in the panel, then scrolled a frame at a time. The harness shapes no text
/// (GPUI's `NoopTextSystem`) and paints no pixels, so this is the fold's and the list's own cost; the
/// real frame is the app's, under `LATHE_FRAMES=1` (docs/performance.md, "Agent sessions in the app").
///     cargo test --release -p lathe-app -- --ignored --nocapture a_long_session_scrolls
#[gpui_kit::test]
#[ignore]
fn a_long_session_scrolls_under_a_frame(cx: &mut TestAppContext) {
    use std::time::{Duration, Instant};
    let mut turn = Vec::new();
    for i in 0..1000u64 {
        turn.push(Event::UserMessage { text: format!("Question {i}: what does this part of the relay do when the client goes away?") });
        turn.push(Event::Text { block: lathe_agents::session::BlockId(i), delta: format!("Answer {i}. {}", "It detaches the byte stream, so a second write does nothing. ".repeat(3)) });
        if i % 10 == 0 {
            let call = ToolCall { id: ToolId::new(format!("t{i}")), name: "Read".into(), kind: ToolKind::Read, input: serde_json::json!({}), file: Some("src/relay.rs".into()), parent: None, status: ToolStatus::Done };
            turn.push(Event::ToolStarted(call));
        }
    }
    turn.push(ended());
    let (session, _, cx) = start(cx, vec![turn], false);
    let opened = Instant::now();
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("go".into(), cx)));
    cx.run_until_parked();
    let folded = opened.elapsed();
    struct Panel(Entity<AgentSession>);
    impl gpui_kit::Render for Panel {
        fn render(&mut self, window: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
            use gpui_kit::{ParentElement, Styled};
            gpui_kit::div().w(gpui_kit::px(480.)).h(gpui_kit::px(820.)).child(crate::session_view::session_view(&self.0, window, cx))
        }
    }
    let shown = session.clone();
    let (_panel, cx) = cx.add_window_view(move |_, _| Panel(shown));
    let first = Instant::now();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let first = first.elapsed();
    let items = cx.update(|_, cx| session.read(cx).conversation.items().len());
    let mut frames: Vec<Duration> = Vec::new();
    for step in 0..200 {
        let list = cx.update(|_, cx| session.read(cx).list.clone());
        list.scroll_by(gpui_kit::px(if step < 100 { -240. } else { 240. }));
        let at = Instant::now();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        frames.push(at.elapsed());
    }
    frames.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.;
    println!(
        "{items} items: folded in {:.1} ms, first draw {:.2} ms; scroll frames median {:.2} ms, p95 {:.2} ms, worst {:.2} ms, over 8 ms: {}",
        ms(folded), ms(first), ms(frames[100]), ms(frames[190]), ms(frames[199]), frames.iter().filter(|f| **f > Duration::from_millis(8)).count()
    );
}

/// A turn that edits one file and makes another, one of them through a shell command no tool names,
/// ends with its changed files, and their card follows the turn's last row.
#[gpui_kit::test]
fn a_turn_ends_with_its_changed_files_after_its_rows(cx: &mut TestAppContext) {
    let dir = git_project(&[("a.txt", "one\n"), ("b.txt", "keep\n")]);
    let (session, fake, cx) = start_in(cx, dir.clone(), vec![vec![Event::Text { block: lathe_agents::session::BlockId(1), delta: "done".into() }, ended()]], false);
    let root = dir.clone();
    fake.work.lock().unwrap().push(Box::new(move || {
        std::fs::write(root.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(root.join("c.txt"), "new\n").unwrap();
    }));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("edit".into(), cx)));
    cx.run_until_parked();
    let (turns, shown, counts) = cx.update(|_, cx| {
        let s = session.read(cx);
        let files = s.reviews.turns.turns().first().map(|t| t.files().iter().map(|f| (f.path.clone(), f.counts())).collect::<Vec<_>>());
        (s.reviews.turns.turns().len(), s.shown.clone(), files)
    });
    assert_eq!(turns, 1);
    assert_eq!(counts.unwrap(), [("a.txt".to_string(), (1, 0)), ("c.txt".to_string(), (1, 0))], "b.txt did not change");
    assert_eq!(shown.last(), Some(&list_diff::Row::Changes { turn: 0 }), "the card comes after the turn's rows");
}

/// The review is kept in the data folder with the session: a session resumed after a restart opens its
/// turns, its cards and the reader's marks as they were.
#[gpui_kit::test]
fn a_resumed_session_opens_its_review_as_it_was_left(cx: &mut TestAppContext) {
    let dir = git_project(&[("a.txt", "one\n")]);
    let (session, fake, cx) = start_in(cx, dir.clone(), vec![vec![ended()]], false);
    let root = dir.clone();
    fake.work.lock().unwrap().push(Box::new(move || std::fs::write(root.join("a.txt"), "two\n").unwrap()));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("edit".into(), cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        session.update(cx, |s, cx| {
            let a = s.reviews.turns.turns()[0].file("a.txt").cloned().unwrap();
            s.reviews.set_reviewed(0, &a, true);
            s.save_review(cx);
        })
    });
    cx.executor().advance_clock(SAVE_AFTER * 2);
    cx.run_until_parked();
    let (project, agent, id, marks) = cx.update(|_, cx| {
        let s = session.read(cx);
        (s.project.clone(), s.agent.clone(), s.id.clone().unwrap(), s.reviews.turn_marks.clone())
    });
    let resumed = cx.update(|window, cx| cx.new(|cx| AgentSession::start("k2".into(), agent, project, Some((id, "edit".into())), window, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        let r = &resumed.read(cx).reviews;
        assert_eq!(r.turns.turns().len(), 1, "the turn came back");
        let a = r.turns.turns()[0].file("a.txt").unwrap();
        assert!(r.is_reviewed(0, a), "and the mark on it");
        assert_eq!(r.turn_marks, marks, "and where its card sits");
    });
}
/// After its first turn, a session takes a short title the agent drafts; a name the reader gave stays.
#[gpui_kit::test]
fn the_first_turn_drafts_a_short_title(cx: &mut TestAppContext) {
    let (session, _fake, cx) = start(cx, vec![vec![ended()]], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("please could you fix the flaky test in the lease module".into(), cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| session.read(cx).shown_title().to_string()), "Keep TWO", "the fake agent drafts this");
}
#[gpui_kit::test]
fn a_name_the_reader_gave_stays(cx: &mut TestAppContext) {
    let (session, _fake, cx) = start(cx, vec![vec![ended()]], false);
    cx.update(|_, cx| session.update(cx, |s, _| s.name = Some("Mine".into())));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("fix it".into(), cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| session.read(cx).shown_title().to_string()), "Mine");
}

/// A turn cut off (the app closed while the agent asked something) still leaves when the agent last
/// worked: the record is kept on activity, not only at a turn's end. Resumed, the session takes that
/// time, not the agent's list, which the resume touches and so says "now".
#[gpui_kit::test]
fn a_session_cut_off_mid_turn_keeps_its_last_activity(cx: &mut TestAppContext) {
    let dir = git_project(&[("a.txt", "one\n")]);
    let working = lathe_agents::session::Event::Text { block: lathe_agents::session::BlockId(0), delta: "working".into() };
    let (session, _fake, cx) = start_in(cx, dir.clone(), vec![vec![working]], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("edit".into(), cx)));
    cx.run_until_parked();
    cx.executor().advance_clock(SAVE_AFTER * 2);
    cx.run_until_parked();
    let (project, agent, id, worked) = cx.update(|_, cx| {
        let s = session.read(cx);
        (s.project.clone(), s.agent.clone(), s.id.clone().unwrap(), s.active_at)
    });
    assert!(worked > 0, "the agent's text is activity");
    let resumed = cx.update(|window, cx| cx.new(|cx| AgentSession::start("k2".into(), agent, project, Some((id, "edit".into())), window, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        let r = resumed.read(cx);
        assert!(r.activity_known, "the resumed session knows when it last worked");
        assert_eq!(r.active_at, worked, "and it is the time of the cut-off turn, not now");
    });
}
