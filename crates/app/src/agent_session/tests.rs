use gpui_kit::AppContext;
use gpui_kit::TestAppContext;
use atelier_agents::session::{Choice, ChoiceId, PermissionRequest, RequestId, ToolCall, ToolId, ToolKind, ToolStatus};

use super::*;
use crate::fake_agent::{ended, git_project, git_project_in, start, start_in, start_shown_in};

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
    assert_eq!(cx.update(|_, cx| session.read(cx).status.clone()), SessionStatus::NeedsYou(atelier_ui::session_status::Need::Approval));
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

/// A turn that ends asks the agent what fills its context, and what it tells reaches the conversation. An agent that
/// refuses is no problem for the reader to see.
#[gpui_kit::test]
fn a_turn_that_ends_asks_what_fills_the_context(cx: &mut TestAppContext) {
    let (session, fake, cx) = start(cx, vec![vec![Event::ContextParts(vec![atelier_agents::session::ContextPart { label: "Skills".into(), tokens: 9_950 }]), ended()]], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hello".into(), cx)));
    cx.run_until_parked();
    assert!(fake.received.lock().unwrap().iter().any(|c| matches!(c, Command::RefreshContext)), "it asked");
    assert_eq!(cx.update(|_, cx| session.read(cx).conversation.context_parts().len()), 1, "and what it told is kept");
    assert!(cx.update(|_, cx| session.read(cx).problem.clone()).is_none());
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

/// A message whose agent cannot start is not a turn that goes on: the session says why and stops waiting, with no Stop
/// button and no "Waiting for" line left for good.
#[gpui_kit::test]
fn a_message_the_agent_could_not_start_for_does_not_leave_the_turn_open(cx: &mut TestAppContext) {
    let (session, fake, cx) = start(cx, vec![], true);
    fake.fail_next_open();
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hello".into(), cx)));
    cx.run_until_parked();
    assert!(cx.update(|_, cx| session.read(cx).problem.clone()).is_some_and(|p| p.contains("not installed")));
    assert!(!cx.update(|_, cx| session.read(cx).conversation.working()), "no turn waits for an agent that never started");
}

/// An answer that cannot reach the agent, because its session has ended, does not leave the session working.
#[gpui_kit::test]
fn an_answer_the_ended_session_cannot_take_does_not_leave_it_working(cx: &mut TestAppContext) {
    let (session, _fake, cx) = start(cx, vec![vec![Event::Permission(ask())]], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("write a.txt".into(), cx)));
    cx.run_until_parked();
    cx.update(|_, cx| session.update(cx, |s, cx| s.stop(cx)));
    cx.update(|_, cx| session.update(cx, |s, cx| s.answer(&RequestId::new("r1"), ChoiceKind::Allow, cx)));
    assert_ne!(cx.update(|_, cx| session.read(cx).status.clone()), SessionStatus::Working, "nothing runs");
    assert!(!cx.update(|_, cx| session.read(cx).conversation.working()));
}

/// Streaming text joins into one row, and only the rows that changed are measured again.
#[gpui_kit::test]
fn a_stream_folds_into_one_row(cx: &mut TestAppContext) {
    let block = atelier_agents::session::BlockId(1);
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
/// real frame is the app's, under `ATELIER_FRAMES=1` (docs/performance.md, "Agent sessions in the app").
///     cargo test --release -p atelier-app -- --ignored --nocapture a_long_session_scrolls
#[gpui_kit::test]
#[ignore]
fn a_long_session_scrolls_under_a_frame(cx: &mut TestAppContext) {
    use std::time::{Duration, Instant};
    let mut turn = Vec::new();
    for i in 0..1000u64 {
        turn.push(Event::UserMessage { text: format!("Question {i}: what does this part of the relay do when the client goes away?") });
        turn.push(Event::Text { block: atelier_agents::session::BlockId(i), delta: format!("Answer {i}. {}", "It detaches the byte stream, so a second write does nothing. ".repeat(3)) });
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
    let (session, fake, cx) = start_in(cx, dir.clone(), vec![vec![Event::Text { block: atelier_agents::session::BlockId(1), delta: "done".into() }, ended()]], false);
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
    let working = atelier_agents::session::Event::Text { block: atelier_agents::session::BlockId(0), delta: "working".into() };
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

/// The composer's lists come from the project: atelier's running commands, its skills and command files,
/// and its tracked files. A atelier command that does nothing yet is not offered.
#[gpui_kit::test]
fn the_composer_offers_the_projects_commands_and_files(cx: &mut TestAppContext) {
    let dir = crate::test_dirs::path();
    std::fs::create_dir_all(dir.join(".claude/skills/ship")).unwrap();
    std::fs::write(dir.join(".claude/skills/ship/SKILL.md"), "---\nname: ship\ndescription: Ship the change\n---\nbody\n").unwrap();
    std::fs::create_dir_all(dir.join(".claude/commands")).unwrap();
    std::fs::write(dir.join(".claude/commands/tidy.md"), "Tidy the folder\n").unwrap();
    let dir = git_project_in(dir, &[("a.txt", "one\n"), ("src.rs", "fn main() {}\n")]);
    let (session, _, cx) = start_in(cx, dir, Vec::new(), false);
    cx.executor().advance_clock(std::time::Duration::from_millis(50));
    cx.run_until_parked();
    let (names, files) = cx.update(|_, cx| {
        let composer = session.read(cx).composer.read(cx);
        (composer.commands().iter().map(|c| c.name.to_string()).collect::<Vec<_>>(), composer.files().iter().map(|f| f.to_string()).collect::<Vec<_>>())
    });
    for name in ["files", "tasks", "review", "login", "ship", "tidy"] {
        assert!(names.iter().any(|n| n == name), "{name} is offered: {names:?}");
    }
    assert!(!names.iter().any(|n| n == "goal"), "a command atelier cannot run yet is not offered: {names:?}");
    assert!(files.iter().any(|f| f == "a.txt") && files.iter().any(|f| f == "src.rs"), "the tracked files are offered: {files:?}");
}

/// A `/` command from the list: atelier's own runs in atelier, any other goes to the agent as its text.
#[gpui_kit::test]
fn a_command_runs_in_atelier_or_goes_to_the_agent_as_text(cx: &mut TestAppContext) {
    let (session, fake, cx) = start(cx, vec![vec![ended()]], false);
    let heard = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let log = heard.clone();
    cx.update(|_, cx| {
        cx.subscribe(&session, move |_, e: &SessionEvent, _| log.borrow_mut().push(std::mem::discriminant(e))).detach();
        session.update(cx, |s, cx| {
            s.run_command("files", "", cx);
            s.run_command("tasks", "", cx);
            s.run_command("review", "", cx);
        });
    });
    let kinds = heard.borrow().clone();
    assert_eq!(kinds.len(), 3, "atelier's three commands each told the app");
    assert!(fake.received.lock().unwrap().is_empty(), "and none reached the agent");
    cx.update(|_, cx| session.update(cx, |s, cx| s.run_command("compact", "now", cx)));
    cx.run_until_parked();
    assert!(fake.received.lock().unwrap().iter().any(|c| matches!(c, Command::Send { text, .. } if text == "/compact now")), "the agent's own command went as text");
}

fn started_tool(id: &str) -> Event {
    Event::ToolStarted(ToolCall {
        id: ToolId::new(id),
        name: "Bash".into(),
        kind: ToolKind::Shell,
        input: serde_json::json!({ "command": "ls" }),
        file: None,
        parent: None,
        status: ToolStatus::Running,
    })
}

fn finished_tool(id: &str) -> Event {
    Event::ToolFinished { id: ToolId::new(id), output: atelier_agents::session::ToolOutput { text: "ok".into(), truncated: false, full_at: None, is_error: false } }
}

/// The agent's work in a turn is one group: live and open while the agent works with nothing after it,
/// folded to its words once the turn is over, and a press opens it again.
#[gpui_kit::test]
fn a_run_of_work_is_one_group_that_folds_when_the_turn_ends(cx: &mut TestAppContext) {
    let block = atelier_agents::session::BlockId(1);
    let work = vec![
        Event::Thinking { block, delta: "hm".into() },
        Event::ThinkingDone { block, took: std::time::Duration::from_secs(3) },
        started_tool("a"),
        finished_tool("a"),
        started_tool("b"),
        finished_tool("b"),
    ];
    let (session, fake, cx) = start(cx, vec![work.clone()], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("go".into(), cx)));
    cx.run_until_parked();
    let (shown, live, open) = cx.update(|_, cx| {
        let s = session.read(cx);
        (s.shown.clone(), s.group_is_live(4), s.group_is_open(1, 4))
    });
    assert_eq!(shown, [list_diff::Row::Item(0), list_diff::Row::Activity { from: 1, to: 4 }], "the user's message, then one group");
    assert!(live && open, "the agent still works: the group is live and open");
    fake.turns.lock().unwrap().push(vec![Event::Text { block: atelier_agents::session::BlockId(2), delta: "done".into() }, ended()]);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("more".into(), cx)));
    cx.run_until_parked();
    let (shown, folded) = cx.update(|_, cx| {
        let s = session.read(cx);
        (s.shown.clone(), !s.group_is_open(1, 4))
    });
    assert!(shown.contains(&list_diff::Row::Activity { from: 1, to: 4 }), "the group is still one row: {shown:?}");
    assert!(folded, "the turn is over: the group is folded");
    cx.update(|_, cx| session.update(cx, |s, cx| s.toggle_group(1, cx)));
    assert!(cx.update(|_, cx| session.read(cx).group_is_open(1, 4)), "a press opens it");
    cx.update(|_, cx| session.update(cx, |s, cx| s.toggle_group(1, cx)));
    assert!(!cx.update(|_, cx| session.read(cx).group_is_open(1, 4)), "and folds it again");
}

/// With the density at one line per call (or full detail), a run of work is not folded into a group: every item has its own
/// row. A change of the setting reaches a session that is open, both ways.
#[gpui_kit::test]
fn a_density_of_lines_gives_each_call_its_own_row_and_follows_the_setting(cx: &mut TestAppContext) {
    use crate::tool_density::ToolDensity;
    let block = atelier_agents::session::BlockId(1);
    let work = vec![
        Event::Thinking { block, delta: "hm".into() },
        Event::ThinkingDone { block, took: std::time::Duration::from_secs(3) },
        started_tool("a"),
        finished_tool("a"),
        started_tool("b"),
        finished_tool("b"),
        ended(),
    ];
    let (session, _fake, cx) = start(cx, vec![work], false);
    cx.update(|_, cx| cx.set_global(ToolDensity::Lines));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("go".into(), cx)));
    cx.run_until_parked();
    let rows = |cx: &mut gpui_kit::VisualTestContext| cx.update(|_, cx| session.read(cx).shown.clone());
    let items = (0..4).map(list_diff::Row::Item).collect::<Vec<_>>();
    assert_eq!(rows(cx), items, "one row each, the user's message first");
    cx.update(|_, cx| cx.set_global(ToolDensity::Detailed));
    cx.run_until_parked();
    assert_eq!(rows(cx), items, "full detail is also one row each");
    cx.update(|_, cx| cx.set_global(ToolDensity::Grouped));
    cx.run_until_parked();
    assert_eq!(rows(cx), [list_diff::Row::Item(0), list_diff::Row::Activity { from: 1, to: 4 }], "grouped again");
}

mod providers {
    use std::sync::Arc;

    use atelier_agents::session::{Account, ApiKey, EndReason, Event, Provider};
    use atelier_settings::secrets::{InMemory, OPENROUTER_KEY, Secrets};
    use gpui_kit::{AppContext, TestAppContext};

    use crate::{
        fake_agent::{start, start_on_providers},
        providers::{Choice, DefaultProvider, NO_KEY, ProviderServices},
    };

    const WORK: &str = "work";
    const TEAM: &str = "team";
    const KEY: &str = "sk-or-v1-test";

    fn accounts() -> Vec<Account> {
        vec![
            Account { name: atelier_agents::claude_code::accounts::DEFAULT_ACCOUNT.into(), signed_in: true, plan: Some("max".into()), email: None },
            Account { name: WORK.into(), signed_in: true, plan: Some("max".into()), email: None },
            Account { name: TEAM.into(), signed_in: false, plan: None, email: None },
        ]
    }

    /// Keeps `secrets` as the keychain and `default` as the default, before the session starts.
    fn set_up(cx: &mut TestAppContext, secrets: &Arc<InMemory>, default: Choice) {
        let services = ProviderServices { secrets: secrets.clone(), ..ProviderServices::isolated() };
        cx.update(|cx| {
            cx.set_global(services);
            cx.set_global(DefaultProvider(default));
        });
    }

    #[gpui_kit::test]
    fn a_new_session_starts_on_the_default_provider(cx: &mut TestAppContext) {
        set_up(cx, &Arc::new(InMemory::default()), Choice::Account(WORK.into()));

        let (session, fake, cx) = start_on_providers(cx, accounts());

        assert_eq!(cx.update(|_, cx| session.read(cx).provider.clone()), Some(Choice::Account(WORK.into())));
        assert_eq!(fake.opened.lock().unwrap()[0].provider, Some(Provider::Account(WORK.into())));
    }

    /// A session that resumes runs on the provider it ran on, and a resume that names none keeps the agent's own sign-in.
    #[gpui_kit::test]
    fn a_resumed_session_runs_on_the_provider_it_is_given(cx: &mut TestAppContext) {
        set_up(cx, &Arc::new(InMemory::default()), Choice::Account(WORK.into()));
        let (session, fake, cx) = start_on_providers(cx, accounts());
        let (agent, project) = cx.update(|_, cx| (session.read(cx).agent.clone(), session.read(cx).project.clone()));
        let resume = || Some((atelier_agents::session::SessionId::new("old"), "title".into()));

        let (a, p) = (agent.clone(), project.clone());
        let on_work = cx.update(|window, cx| {
            cx.new(|cx| super::super::AgentSession::start_on("k2".into(), a, p, resume(), Some(Choice::Account(WORK.into())), window, cx))
        });
        cx.run_until_parked();
        assert_eq!(cx.update(|_, cx| on_work.read(cx).provider.clone()), Some(Choice::Account(WORK.into())));
        assert_eq!(fake.opened.lock().unwrap().last().unwrap().provider, Some(Provider::Account(WORK.into())));

        let none = cx.update(|window, cx| cx.new(|cx| super::super::AgentSession::start("k3".into(), agent, project, resume(), window, cx)));
        cx.run_until_parked();
        assert_eq!(cx.update(|_, cx| none.read(cx).provider.clone()), None, "a plain resume names no provider");
    }

    #[gpui_kit::test]
    fn an_agent_with_no_providers_starts_as_it_is(cx: &mut TestAppContext) {
        set_up(cx, &Arc::new(InMemory::default()), Choice::Account(WORK.into()));

        let (session, fake, cx) = start(cx, Vec::new(), false);

        assert_eq!(cx.update(|_, cx| session.read(cx).provider.clone()), None);
        assert_eq!(fake.opened.lock().unwrap()[0].provider, None);
    }

    #[gpui_kit::test]
    fn the_choices_are_the_signed_in_accounts_and_openrouter_with_a_key(cx: &mut TestAppContext) {
        let secrets = Arc::new(InMemory::default());
        secrets.write(OPENROUTER_KEY, KEY).unwrap();
        set_up(cx, &secrets, Choice::usual());

        let (session, _, cx) = start_on_providers(cx, accounts());

        assert_eq!(
            cx.update(|_, cx| session.read(cx).provider_choices()),
            vec![Choice::usual(), Choice::Account(WORK.into()), Choice::OpenRouter],
            "the team account is not signed in"
        );
    }

    #[gpui_kit::test]
    fn the_agent_a_switch_stops_does_not_end_the_one_that_replaces_it(cx: &mut TestAppContext) {
        let secrets = Arc::new(InMemory::default());
        secrets.write(OPENROUTER_KEY, KEY).unwrap();
        set_up(cx, &secrets, Choice::usual());
        let (session, fake, cx) = start_on_providers(cx, accounts());
        cx.update(|_, cx| session.update(cx, |s, cx| s.set_provider(Choice::OpenRouter, cx)));
        cx.run_until_parked();

        let replaced = fake.sinks.lock().unwrap()[0].clone();
        replaced(Event::Ended(EndReason::Exited { code: None, stderr: String::new() }));
        cx.run_until_parked();

        assert!(cx.update(|_, cx| session.read(cx).running()), "the old agent's end belongs to the old agent");
    }

    #[gpui_kit::test]
    fn switching_provider_starts_the_agent_again_on_it(cx: &mut TestAppContext) {
        let secrets = Arc::new(InMemory::default());
        secrets.write(OPENROUTER_KEY, KEY).unwrap();
        set_up(cx, &secrets, Choice::usual());
        let (session, fake, cx) = start_on_providers(cx, accounts());

        cx.update(|_, cx| session.update(cx, |s, cx| s.set_provider(Choice::OpenRouter, cx)));
        cx.run_until_parked();

        let opened = fake.opened.lock().unwrap();
        assert_eq!(opened.len(), 2);
        assert_eq!(opened[1].provider, Some(Provider::OpenRouter { key: ApiKey::new(KEY) }));
    }

    #[gpui_kit::test]
    fn openrouter_with_no_key_says_where_to_add_one(cx: &mut TestAppContext) {
        set_up(cx, &Arc::new(InMemory::default()), Choice::OpenRouter);

        let (session, fake, cx) = start_on_providers(cx, accounts());

        assert!(fake.opened.lock().unwrap().is_empty(), "the agent never started");
        assert!(cx.update(|_, cx| session.read(cx).problem.clone()).is_some_and(|p| p.contains(NO_KEY)));
    }
}

mod handoff {
    use atelier_agents::session::{BlockId, Command, Event, Item, SessionId};
    use gpui_kit::{Entity, TestAppContext, VisualTestContext};

    use std::sync::Arc;

    use atelier_settings::secrets::{InMemory, OPENROUTER_KEY, Secrets};

    use crate::{
        agent_session::{AgentSession, handoff::Source},
        fake_agent::{backend_with_history, start, start_forking},
        providers::{Choice, DefaultProvider, ProviderServices},
    };

    const AGENT: &str = "Claude Code";
    const TITLE: &str = "Add a subtract function";
    const EARLIER_ASK: &str = "Add a subtract function to src/lib.rs";
    const EARLIER_ANSWER: &str = "I added subtract and a test.";
    const SOURCE_ID: &str = "source-1";
    const NEXT_ASK: &str = "Now add multiply";
    const LATER_ASK: &str = "And divide";

    fn source() -> Source {
        let history = vec![
            Event::UserMessage { text: EARLIER_ASK.into() },
            Event::Text { block: BlockId(1), delta: EARLIER_ANSWER.into() },
        ];
        Source { backend: backend_with_history(history), agent: AGENT.into(), id: SessionId::new(SOURCE_ID), title: TITLE.into() }
    }

    fn continue_from_source(session: &Entity<AgentSession>, cx: &mut VisualTestContext) {
        cx.update(|_, cx| session.update(cx, |s, cx| s.continue_from(source(), None, cx)));
    }

    fn send(session: &Entity<AgentSession>, text: &str, cx: &mut VisualTestContext) {
        cx.update(|_, cx| session.update(cx, |s, cx| s.send(text.into(), cx)));
    }

    fn sent_texts(received: &[Command]) -> Vec<String> {
        received.iter().filter_map(|c| match c { Command::Send { text, .. } => Some(text.clone()), _ => None }).collect()
    }

    fn user_texts(session: &Entity<AgentSession>, cx: &mut VisualTestContext) -> Vec<String> {
        cx.update(|_, cx| {
            session.read(cx).conversation.items().iter().filter_map(|i| match i { Item::User { text } => Some(text.clone()), _ => None }).collect()
        })
    }

    #[gpui_kit::test]
    fn the_first_message_carries_the_brief_and_shows_as_typed(cx: &mut TestAppContext) {
        let (session, fake, cx) = start(cx, Vec::new(), false);
        continue_from_source(&session, cx);
        cx.run_until_parked();

        send(&session, NEXT_ASK, cx);
        cx.run_until_parked();

        let sent = sent_texts(&fake.received.lock().unwrap());
        for part in [AGENT, TITLE, EARLIER_ASK, EARLIER_ANSWER, NEXT_ASK] {
            assert!(sent[0].contains(part), "the first message holds {part:?}:\n{}", sent[0]);
        }
        assert_eq!(user_texts(&session, cx), vec![NEXT_ASK.to_string()]);
    }

    #[gpui_kit::test]
    fn a_message_sent_before_the_brief_is_ready_waits_for_it(cx: &mut TestAppContext) {
        let (session, fake, cx) = start(cx, Vec::new(), false);
        continue_from_source(&session, cx);

        send(&session, NEXT_ASK, cx);
        cx.run_until_parked();

        let sent = sent_texts(&fake.received.lock().unwrap());
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains(EARLIER_ANSWER) && sent[0].contains(NEXT_ASK), "{}", sent[0]);
    }

    #[gpui_kit::test]
    fn only_the_first_message_carries_the_brief(cx: &mut TestAppContext) {
        let (session, fake, cx) = start(cx, Vec::new(), false);
        continue_from_source(&session, cx);
        cx.run_until_parked();

        send(&session, NEXT_ASK, cx);
        cx.run_until_parked();
        send(&session, LATER_ASK, cx);
        cx.run_until_parked();

        assert_eq!(sent_texts(&fake.received.lock().unwrap())[1], LATER_ASK);
    }

    #[gpui_kit::test]
    fn the_whole_transcript_is_kept_in_the_data_folder_and_the_brief_names_it(cx: &mut TestAppContext) {
        let (session, fake, cx) = start(cx, Vec::new(), false);
        continue_from_source(&session, cx);
        cx.run_until_parked();
        send(&session, NEXT_ASK, cx);
        cx.run_until_parked();

        let kept = cx.update(|_, cx| session.read(cx).project.data_read(&format!("handoffs/{SOURCE_ID}.md"))).expect("the transcript is kept");
        assert!(String::from_utf8_lossy(&kept).contains(EARLIER_ANSWER));
        assert!(sent_texts(&fake.received.lock().unwrap())[0].contains(&format!("handoffs/{SOURCE_ID}.md")));
    }

    /// Keeps an OpenRouter key and the usual account as the default.
    fn set_up_providers(cx: &mut TestAppContext) {
        let secrets = Arc::new(InMemory::default());
        secrets.write(OPENROUTER_KEY, "sk-or-v1-test").unwrap();
        cx.update(|cx| {
            cx.set_global(ProviderServices { secrets, ..ProviderServices::isolated() });
            cx.set_global(DefaultProvider(Choice::usual()));
        });
    }

    #[gpui_kit::test]
    fn the_same_agent_on_an_account_resumes_a_fork_and_sends_no_brief(cx: &mut TestAppContext) {
        set_up_providers(cx);
        let (session, fake, cx) = start_forking(cx);
        continue_from_source(&session, cx);
        cx.run_until_parked();

        send(&session, NEXT_ASK, cx);
        cx.run_until_parked();

        let last = fake.opened.lock().unwrap().last().cloned().unwrap();
        assert_eq!((last.resume, last.fork), (Some(SessionId::new(SOURCE_ID)), true));
        assert_eq!(sent_texts(&fake.received.lock().unwrap()), vec![NEXT_ASK.to_string()]);
    }

    #[gpui_kit::test]
    fn a_handoff_that_names_a_provider_starts_the_agent_on_it(cx: &mut TestAppContext) {
        set_up_providers(cx);
        let (session, fake, cx) = start_forking(cx);
        cx.update(|_, cx| session.update(cx, |s, cx| s.continue_from(source(), Some(Choice::OpenRouter), cx)));
        cx.run_until_parked();

        assert_eq!(cx.update(|_, cx| session.read(cx).provider.clone()), Some(Choice::OpenRouter));
        let last = fake.opened.lock().unwrap().last().cloned().unwrap();
        assert!(matches!(last.provider, Some(atelier_agents::session::Provider::OpenRouter { .. })), "the agent started again on OpenRouter");
    }

    #[gpui_kit::test]
    fn the_same_agent_on_openrouter_starts_fresh_and_takes_the_brief(cx: &mut TestAppContext) {
        set_up_providers(cx);
        let (session, fake, cx) = start_forking(cx);
        continue_from_source(&session, cx);
        cx.run_until_parked();
        cx.update(|_, cx| session.update(cx, |s, cx| s.set_provider(Choice::OpenRouter, cx)));
        cx.run_until_parked();

        send(&session, NEXT_ASK, cx);
        cx.run_until_parked();

        let last = fake.opened.lock().unwrap().last().cloned().unwrap();
        assert_eq!((last.resume, last.fork), (None, false));
        assert!(sent_texts(&fake.received.lock().unwrap())[0].contains(EARLIER_ANSWER));
    }
}

fn sent_texts(fake: &crate::fake_agent::Fake) -> Vec<String> {
    fake.received.lock().unwrap().iter().filter_map(|c| match c {
        Command::Send { text, .. } => Some(text.clone()),
        _ => None,
    }).collect()
}

/// A message queued while a turn runs waits for it and goes when it completes, one per turn; a Stop
/// keeps the rest waiting.
#[gpui_kit::test]
fn a_queued_message_waits_for_the_turn_to_complete(cx: &mut TestAppContext) {
    let (session, fake, cx) = start(cx, vec![vec![], vec![], vec![]], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("first".into(), cx)));
    cx.run_until_parked();
    cx.update(|_, cx| session.update(cx, |s, cx| {
        s.queue("second".into(), cx);
        s.queue("third".into(), cx);
    }));
    assert_eq!(sent_texts(&fake), ["first"]);
    assert_eq!(cx.update(|_, cx| session.read(cx).queued.clone()), ["second", "third"]);

    let agent = fake.sinks.lock().unwrap()[0].clone();
    agent(ended());
    cx.run_until_parked();
    assert_eq!(sent_texts(&fake), ["first", "second"], "the oldest goes when the turn completes");
    assert_eq!(cx.update(|_, cx| session.read(cx).queued.clone()), ["third"]);

    agent(Event::TurnEnded(atelier_agents::session::TurnEnd { outcome: atelier_agents::session::TurnOutcome::Interrupted, summary: None }));
    cx.run_until_parked();
    assert_eq!(sent_texts(&fake), ["first", "second"], "a Stop keeps the queue");
    assert_eq!(cx.update(|_, cx| session.read(cx).queued.clone()), ["third"]);

    agent(Event::Text { block: atelier_agents::session::BlockId(1), delta: "the background task finished".into() });
    agent(ended());
    cx.run_until_parked();
    assert_eq!(sent_texts(&fake), ["first", "second"], "a turn the agent started itself leaves the queue alone");
    assert_eq!(cx.update(|_, cx| session.read(cx).queued.clone()), ["third"]);
}

/// With no turn running a queued message goes at once; a queued one can go now, or come out.
#[gpui_kit::test]
fn the_queue_sends_at_once_when_idle_and_rows_send_now_or_come_out(cx: &mut TestAppContext) {
    let (session, fake, cx) = start(cx, vec![vec![], vec![], vec![]], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.queue("now".into(), cx)));
    cx.run_until_parked();
    assert_eq!(sent_texts(&fake), ["now"], "nothing ran, so nothing to wait for");
    cx.update(|_, cx| session.update(cx, |s, cx| {
        s.queue("dropped".into(), cx);
        s.queue("steered".into(), cx);
        s.unqueue(0, cx);
        s.send_queued(0, cx);
    }));
    cx.run_until_parked();
    assert_eq!(sent_texts(&fake), ["now", "steered"]);
    assert!(cx.update(|_, cx| session.read(cx).queued.is_empty()));
}

/// A queued message that goes when the turn completes shows as the reader's message, like any other.
#[gpui_kit::test]
fn a_queued_message_shows_when_it_goes(cx: &mut TestAppContext) {
    let (session, fake, cx) = start(cx, vec![vec![], vec![]], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("first".into(), cx)));
    cx.run_until_parked();
    cx.update(|_, cx| session.update(cx, |s, cx| s.queue("second".into(), cx)));
    let agent = fake.sinks.lock().unwrap()[0].clone();
    agent(Event::Text { block: atelier_agents::session::BlockId(1), delta: "done".into() });
    agent(ended());
    cx.run_until_parked();
    let (items, rows) = cx.update(|_, cx| {
        let s = session.read(cx);
        (s.conversation.items().to_vec(), s.list.item_count())
    });
    assert!(matches!(items.last(), Some(atelier_agents::session::Item::User { text }) if text == "second"), "the queued message is the last item");
    assert_eq!(rows, items.len() + 1, "and it has a row, with the waiting line under it until the agent answers");
}

/// The composer runs from the moment a message goes: ⌘↵ typed before the agent's first word queues.
#[gpui_kit::test]
fn a_message_queued_before_the_agent_answers_waits(cx: &mut TestAppContext) {
    let (session, fake, cx) = start_shown_in(cx, crate::test_dirs::path(), vec![vec![], vec![]]);
    cx.update(|window, cx| {
        let composer = session.read(cx).composer.clone();
        window.focus(&gpui_kit::Focusable::focus_handle(&composer, cx), cx);
    });
    cx.simulate_input("first");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("later");
    cx.simulate_keystrokes("secondary-enter");
    cx.run_until_parked();
    assert_eq!(sent_texts(&fake), ["first"]);
    assert_eq!(cx.update(|_, cx| session.read(cx).queued.clone()), ["later"]);
}

/// A message the agent refuses for want of a sign-in: it says so, then its turn fails.
fn refused_for_sign_in() -> Vec<Event> {
    vec![Event::SignedOut, Event::TurnEnded(atelier_agents::session::TurnEnd { outcome: atelier_agents::session::TurnOutcome::Failed("Not logged in".into()), summary: None })]
}

fn answer(text: &str) -> Vec<Event> {
    vec![Event::Text { block: atelier_agents::session::BlockId(7), delta: text.into() }, ended()]
}

fn signed_out(session: &Entity<AgentSession>, cx: &mut gpui_kit::VisualTestContext) -> bool {
    cx.update(|_, cx| session.read(cx).conversation.signed_out())
}

/// A message refused for want of a sign-in goes again once the reader has signed in: the agent starts again on the
/// same session, and the reader does not type it twice.
#[gpui_kit::test]
fn signing_in_starts_the_agent_again_and_sends_the_refused_message_once_more(cx: &mut TestAppContext) {
    let (session, fake, cx) = crate::fake_agent::start_signing_in(cx, vec![refused_for_sign_in(), answer("hello")], Some(atelier_project::Command::new("true")));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hi".into(), cx)));
    cx.run_until_parked();
    assert!(signed_out(&session, cx));
    cx.update(|_, cx| session.update(cx, |s, cx| s.sign_in(cx)));
    cx.run_until_parked();
    assert!(!signed_out(&session, cx), "the sign-in is over");
    assert_eq!(fake.signed_in_as.lock().unwrap().as_slice(), ["default"], "the account the session runs on");
    let opened = fake.opened.lock().unwrap().clone();
    assert_eq!(opened.len(), 2, "the agent started again");
    assert_eq!(opened[1].resume.as_ref().map(|id| id.as_str()), Some("fake-1"), "on the same session");
    let sent: Vec<_> = fake.received.lock().unwrap().iter().filter_map(|c| if let Command::Send { text, .. } = c { Some(text.clone()) } else { None }).collect();
    assert_eq!(sent, ["hi", "hi"], "the refused message went again");
    let items = cx.update(|_, cx| session.read(cx).conversation.items().to_vec());
    assert_eq!(items.iter().filter(|i| matches!(i, atelier_agents::session::Item::User { .. })).count(), 1, "and shows once: {items:?}");
}

/// A sign-in that does not finish keeps the notice, and says why.
#[gpui_kit::test]
fn a_sign_in_that_fails_keeps_the_notice_and_says_why(cx: &mut TestAppContext) {
    let failing = atelier_project::Command::new("sh").args(["-c", "echo 'browser closed' >&2; exit 1"]);
    let (session, fake, cx) = crate::fake_agent::start_signing_in(cx, vec![refused_for_sign_in()], Some(failing));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hi".into(), cx)));
    cx.run_until_parked();
    cx.update(|_, cx| session.update(cx, |s, cx| s.sign_in(cx)));
    cx.run_until_parked();
    assert!(signed_out(&session, cx), "still signed out");
    let state = cx.update(|_, cx| session.read(cx).sign_in_state());
    assert!(matches!(&state, Some(atelier_ui::SignInState::Failed(why)) if why.contains("browser closed")), "{state:?}");
    assert_eq!(fake.opened.lock().unwrap().len(), 1, "the agent did not start again");
}

/// An agent atelier has no sign-in for is told so in the notice, which still offers the way on.
#[gpui_kit::test]
fn an_agent_with_no_sign_in_command_says_to_sign_in_by_hand(cx: &mut TestAppContext) {
    let (session, _, cx) = crate::fake_agent::start_signing_in(cx, vec![refused_for_sign_in()], None);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hi".into(), cx)));
    cx.run_until_parked();
    cx.update(|_, cx| session.update(cx, |s, cx| s.sign_in(cx)));
    cx.run_until_parked();
    let state = cx.update(|_, cx| session.read(cx).sign_in_state());
    assert!(matches!(&state, Some(atelier_ui::SignInState::Failed(why)) if why.contains("by hand")), "{state:?}");
}

/// `/login` is atelier's: it signs in, it does not go to the agent as text.
#[gpui_kit::test]
fn typing_login_signs_in_and_the_agent_never_hears_it(cx: &mut TestAppContext) {
    let (session, fake, cx) = crate::fake_agent::start_signing_in(cx, vec![refused_for_sign_in()], Some(atelier_project::Command::new("true")));
    cx.update(|_, cx| session.update(cx, |s, cx| s.run_command("login", "", cx)));
    cx.run_until_parked();
    assert_eq!(fake.signed_in_as.lock().unwrap().len(), 1);
    assert!(fake.received.lock().unwrap().iter().all(|c| !matches!(c, Command::Send { .. })), "nothing was sent to the agent");
}

/// The notice over the composer shows while the agent has no sign-in; its button signs in, and the notice goes with it.
#[gpui_kit::test]
fn the_notice_shows_a_sign_in_button_until_the_reader_has_signed_in(cx: &mut TestAppContext) {
    let (session, _, cx) = crate::fake_agent::start_signing_in(cx, vec![refused_for_sign_in(), answer("hello")], Some(atelier_project::Command::new("true")));
    assert!(cx.debug_bounds("sign-in-notice").is_none(), "an agent that works needs no notice");
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hi".into(), cx)));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("sign-in-notice").is_some(), "the agent has no sign-in");
    let button = cx.debug_bounds("sign-in-button").expect("with its button");
    cx.simulate_click(button.center(), gpui_kit::Modifiers::default());
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("sign-in-notice").is_none(), "signed in");
}

/// The notice is not a dead end: the reader can hand the session off instead of signing in.
#[gpui_kit::test]
fn the_notice_offers_a_handoff_when_there_is_somewhere_to_go(cx: &mut TestAppContext) {
    let (session, _, cx) = crate::fake_agent::start_signing_in(cx, vec![refused_for_sign_in()], None);
    cx.update(|_, cx| {
        session.update(cx, |s, _| s.handoff_branches = vec![atelier_ui::menu::Branch { id: "cursor".into(), label: "Cursor".into(), lead: None, branches: Vec::new() }])
    });
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hi".into(), cx)));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("sign-in-handoff").is_some(), "the way on is beside the button");
}

/// With nowhere to go, the notice does not offer an empty menu.
#[gpui_kit::test]
fn the_notice_has_no_handoff_with_nowhere_to_go(cx: &mut TestAppContext) {
    let (session, _, cx) = crate::fake_agent::start_signing_in(cx, vec![refused_for_sign_in()], None);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hi".into(), cx)));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("sign-in-notice").is_some());
    assert!(cx.debug_bounds("sign-in-handoff").is_none());
}

/// The reader signed in some other way and sent the message again: the agent works, and the notice about the sign-in
/// that did not finish goes.
#[gpui_kit::test]
fn a_turn_that_works_takes_the_failed_sign_in_notice_away(cx: &mut TestAppContext) {
    let failing = atelier_project::Command::new("sh").args(["-c", "exit 1"]);
    let (session, _, cx) = crate::fake_agent::start_signing_in(cx, vec![refused_for_sign_in(), answer("hello")], Some(failing));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hi".into(), cx)));
    cx.run_until_parked();
    cx.update(|_, cx| session.update(cx, |s, cx| s.sign_in(cx)));
    cx.run_until_parked();
    assert!(cx.update(|_, cx| session.read(cx).sign_in_state()).is_some());
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("again".into(), cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| session.read(cx).sign_in_state()), None);
}

/// The browser wait has a way out: cancelling stops the sign-in command, and the notice offers the button again. The
/// agent does not start, for the sign-in did not finish.
#[gpui_kit::test]
fn cancelling_the_wait_for_the_browser_stops_the_sign_in_and_offers_it_again(cx: &mut TestAppContext) {
    let waiting = atelier_project::Command::new("sleep").args(["30"]);
    let (session, fake, cx) = crate::fake_agent::start_signing_in(cx, vec![refused_for_sign_in()], Some(waiting));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hi".into(), cx)));
    cx.run_until_parked();
    cx.update(|_, cx| session.update(cx, |s, cx| s.sign_in(cx)));
    assert_eq!(cx.update(|_, cx| session.read(cx).sign_in_state()), Some(atelier_ui::SignInState::Waiting));
    cx.update(|_, cx| session.update(cx, |s, cx| s.cancel_sign_in(cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| session.read(cx).sign_in_state()), Some(atelier_ui::SignInState::Ready), "no failure to read: the reader chose this");
    assert_eq!(fake.opened.lock().unwrap().len(), 1, "the agent did not start again");
}

/// The notice's Cancel button is that way out. It shows only while the browser waits.
#[gpui_kit::test]
fn the_notice_shows_cancel_while_the_browser_waits(cx: &mut TestAppContext) {
    let waiting = atelier_project::Command::new("sleep").args(["30"]);
    let (session, _, cx) = crate::fake_agent::start_signing_in(cx, vec![refused_for_sign_in()], Some(waiting));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hi".into(), cx)));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("sign-in-cancel").is_none(), "nothing to cancel before the browser opens");
    cx.update(|_, cx| session.update(cx, |s, cx| s.sign_in(cx)));
    // Not parked: parking waits for the command, and the command waits for the reader.
    cx.update(|window, _| window.refresh());
    cx.executor().tick();
    // The click is the component's own test: a click parks the test until the command ends, and it waits for the reader.
    assert!(cx.debug_bounds("sign-in-cancel").is_some(), "while the browser waits");
    cx.update(|_, cx| session.update(cx, |s, cx| s.cancel_sign_in(cx)));
}

/// A session resumed from a named account, with no provider of its own, signs in to the account that holds it: the
/// default account would sign in a different one than the session can use.
#[gpui_kit::test]
fn a_session_with_no_provider_signs_in_to_the_account_that_holds_it(cx: &mut TestAppContext) {
    let (session, fake, cx) = crate::fake_agent::start_signing_in_held_by(cx, vec![refused_for_sign_in(), answer("hello")], Some(atelier_project::Command::new("true")), "work");
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hi".into(), cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| session.read(cx).sign_in_account()).as_deref(), Some("work"), "the notice names it");
    cx.update(|_, cx| session.update(cx, |s, cx| s.sign_in(cx)));
    cx.run_until_parked();
    assert_eq!(fake.signed_in_as.lock().unwrap().as_slice(), ["work"]);
}

/// The usual account is not named, and is the one signed in to.
#[gpui_kit::test]
fn a_session_held_by_no_named_account_signs_in_to_the_usual_one(cx: &mut TestAppContext) {
    let (session, fake, cx) = crate::fake_agent::start_signing_in(cx, vec![refused_for_sign_in(), answer("hello")], Some(atelier_project::Command::new("true")));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hi".into(), cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| session.read(cx).sign_in_account()), None);
    cx.update(|_, cx| session.update(cx, |s, cx| s.sign_in(cx)));
    cx.run_until_parked();
    assert_eq!(fake.signed_in_as.lock().unwrap().as_slice(), ["default"]);
}

/// A message sent to an agent that has not said a word yet shows the waiting line at once, and keeps it.
#[gpui_kit::test]
fn a_first_message_shows_the_waiting_line_while_the_agent_is_silent(cx: &mut TestAppContext) {
    let (session, _fake, cx) = start(cx, vec![], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hello".into(), cx)));
    let last = |cx: &mut gpui_kit::VisualTestContext| cx.update(|_, cx| session.read(cx).shown.last().copied());
    assert_eq!(last(cx), Some(crate::list_diff::Row::Waiting), "at once");
    cx.run_until_parked();
    assert_eq!(last(cx), Some(crate::list_diff::Row::Waiting), "after the turn was handed over");
}

/// The same when the agent could not start and starts again for the message: the line shows while it connects.
#[gpui_kit::test]
fn a_message_shows_the_waiting_line_while_the_agent_connects(cx: &mut TestAppContext) {
    let (session, _fake, cx) = start(cx, vec![], true);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hello".into(), cx)));
    let last = |cx: &mut gpui_kit::VisualTestContext| cx.update(|_, cx| session.read(cx).shown.last().copied());
    assert_eq!(last(cx), Some(crate::list_diff::Row::Waiting), "while it connects");
    cx.run_until_parked();
    assert_eq!(last(cx), Some(crate::list_diff::Row::Waiting), "once it has connected");
}

fn picture() -> gpui_kit::Image {
    gpui_kit::Image::from_bytes(gpui_kit::ImageFormat::Png, vec![0x89, b'P', b'N', b'G', 1, 2, 3])
}

/// What is pasted on the composer becomes a chip, and goes with the message that carries it: the text as a fenced block,
/// the picture as a picture.
#[gpui_kit::test]
fn a_paste_is_a_chip_that_goes_with_the_message(cx: &mut TestAppContext) {
    use atelier_agents::session::{Attachment, ImageFormat};
    let (session, fake, cx) = start(cx, vec![vec![ended()]], false);
    cx.update(|_, cx| {
        session.update(cx, |s, cx| {
            s.pasted(atelier_ui::Pasted::Text("let x = 1;\nlet y = 2;".into()), cx);
            s.pasted(atelier_ui::Pasted::Image(std::sync::Arc::new(picture())), cx);
        });
    });
    let labels = cx.update(|_, cx| session.read(cx).composer.read(cx).chips().iter().map(|c| c.label.to_string()).collect::<Vec<_>>());
    assert_eq!(labels, ["Pasted text · 2 lines", "Image"]);

    // The composer's own Enter is tested in atelier-ui; here the message it emits.
    cx.update(|_, cx| {
        let composer = session.read(cx).composer.clone();
        let chips = composer.read(cx).chips().to_vec();
        composer.update(cx, |_, cx| cx.emit(atelier_ui::PromptInputEvent::Submit(atelier_ui::Message { text: "what is wrong here?".into(), chips })));
    });
    cx.run_until_parked();

    let sent = fake.received.lock().unwrap().iter().find_map(|c| match c {
        Command::Send { text, attachments } => Some((text.clone(), attachments.clone())),
        _ => None,
    });
    let (text, attachments) = sent.expect("the message went");
    assert_eq!(text, "what is wrong here?");
    assert_eq!(attachments, [
        Attachment::Text { text: "let x = 1;\nlet y = 2;".into() },
        Attachment::Image { format: ImageFormat::Png, bytes: vec![0x89, b'P', b'N', b'G', 1, 2, 3].into() },
    ]);
    let shown = cx.update(|_, cx| session.read(cx).conversation.items().to_vec());
    assert!(matches!(shown.first(), Some(atelier_agents::session::Item::User { text }) if text.contains("Pasted text · 2 lines") && text.contains("Image · 1 KB")), "the bubble says what came with the words: {shown:?}");
}

/// Words replied to become a quote chip. Pressing the chip opens the reply on it again, and adding there changes the chip
/// where it stands. The quote and the note go with the message, and the bubble says what was quoted.
#[gpui_kit::test]
fn a_reply_is_a_quote_chip_that_can_be_changed_and_goes_with_the_message(cx: &mut TestAppContext) {
    use atelier_agents::session::Attachment;
    use atelier_ui::SelectionReplyEvent;
    let (session, fake, cx) = start(cx, vec![vec![ended()]], false);
    let reply = |cx: &mut gpui_kit::VisualTestContext, quote: &str, note: &str, key: Option<&str>| {
        let event = SelectionReplyEvent::Reply { quote: quote.to_string().into(), note: note.to_string().into(), key: key.map(|k| k.to_string().into()) };
        cx.update(|_, cx| session.read(cx).reply.clone().update(cx, |_, cx| cx.emit(event)));
        cx.run_until_parked();
    };
    let chips = |cx: &mut gpui_kit::VisualTestContext| {
        cx.update(|_, cx| session.read(cx).composer.read(cx).chips().iter().map(|c| (c.id.to_string(), c.label.to_string())).collect::<Vec<_>>())
    };
    reply(cx, "The build fails\non the second run", "why?", None);
    cx.update(|_, cx| session.update(cx, |s, cx| s.pasted(atelier_ui::Pasted::Text("log".into()), cx)));
    let held = chips(cx);
    assert_eq!(held.len(), 2, "a quote and a paste: {held:?}");
    let (id, label) = held[0].clone();
    assert_eq!(label, "“The build fails on the secon…”");

    // Pressing the chip opens the box on it; adding there changes it and keeps its place before the paste.
    let pressed: gpui_kit::SharedString = id.clone().into();
    cx.update(|window, cx| session.update(cx, |s, cx| s.chip_pressed(&pressed, window, cx)));
    assert!(cx.update(|_, cx| session.read(cx).reply.read(cx).showing()), "the box is open on the chip");
    reply(cx, "The build fails\non the second run", "why? it is the cache", Some(&id));
    let after = chips(cx);
    assert_eq!(after.iter().map(|c| c.0.as_str()).collect::<Vec<_>>()[0], id, "{after:?}");
    assert_eq!(after.len(), 2);

    cx.update(|_, cx| {
        let composer = session.read(cx).composer.clone();
        let chips = composer.read(cx).chips().to_vec();
        composer.update(cx, |_, cx| cx.emit(atelier_ui::PromptInputEvent::Submit(atelier_ui::Message { text: "look".into(), chips })));
    });
    cx.run_until_parked();
    let sent = fake.received.lock().unwrap().iter().find_map(|c| match c {
        Command::Send { attachments, .. } => Some(attachments.clone()),
        _ => None,
    });
    assert_eq!(
        sent.expect("the message went"),
        [
            Attachment::Quote { quote: "The build fails\non the second run".into(), note: "why? it is the cache".into() },
            Attachment::Text { text: "log".into() },
        ]
    );
    let shown = cx.update(|_, cx| session.read(cx).conversation.items().to_vec());
    assert!(
        matches!(shown.first(), Some(atelier_agents::session::Item::User { text }) if text.contains("Quoted “The build fails on the second run”") && text.contains("why? it is the cache")),
        "{shown:?}"
    );
}

/// A chip alone is a message; one queued while a turn runs keeps its attachments until it goes.
#[gpui_kit::test]
fn a_queued_message_keeps_what_came_with_it(cx: &mut TestAppContext) {
    use atelier_agents::session::Attachment;
    let (session, fake, cx) = start(cx, vec![vec![], vec![]], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("first".into(), cx)));
    cx.run_until_parked();
    cx.update(|_, cx| session.update(cx, |s, cx| s.queue(super::chips::Draft { text: String::new(), attachments: vec![Attachment::Text { text: "log".into() }] }, cx)));
    let agent = fake.sinks.lock().unwrap()[0].clone();
    agent(ended());
    cx.run_until_parked();
    let last = fake.received.lock().unwrap().last().cloned();
    assert!(matches!(last, Some(Command::Send { text, attachments }) if text.is_empty() && attachments == [Attachment::Text { text: "log".into() }]));
}

/// A picture of a kind the agent cannot read is refused with a reason, not dropped silently.
#[gpui_kit::test]
fn a_picture_the_agent_cannot_read_is_refused_with_a_reason(cx: &mut TestAppContext) {
    let (session, _, cx) = start(cx, vec![], false);
    let tiff = gpui_kit::Image::from_bytes(gpui_kit::ImageFormat::Tiff, vec![1, 2, 3]);
    cx.update(|_, cx| session.update(cx, |s, cx| s.pasted(atelier_ui::Pasted::Image(std::sync::Arc::new(tiff)), cx)));
    let (chips, problem) = cx.update(|_, cx| (session.read(cx).composer.read(cx).chips().len(), session.read(cx).problem.clone()));
    assert_eq!(chips, 0);
    assert!(problem.is_some());
}

/// The composer's mode label says what the agent runs in. Codex starts in Auto review, not in the first mode the
/// composer lists, and a mode set in the session tells the label too.
#[gpui_kit::test]
fn the_composer_shows_the_mode_the_agent_reports(cx: &mut TestAppContext) {
    use atelier_agents::session::{PermissionMode, Started};
    let (session, fake, cx) = crate::fake_agent::start_with_modes(cx);
    let word = |cx: &mut gpui_kit::VisualTestContext| cx.update(|_, cx| session.read(cx).composer.read(cx).mode().map(|m| m.to_string()));
    let agent = fake.sinks.lock().unwrap()[0].clone();
    agent(Event::Started(Started { session: atelier_agents::session::SessionId::new("s"), model: None, mode: Some(PermissionMode::Auto), commands: Vec::new() }));
    cx.run_until_parked();
    assert_eq!(word(cx), Some(crate::agent_session::helpers::mode_word(PermissionMode::Auto).to_string()), "the label follows the agent");
    agent(Event::Started(Started { session: atelier_agents::session::SessionId::new("s"), model: None, mode: Some(PermissionMode::Ask), commands: Vec::new() }));
    cx.run_until_parked();
    assert_eq!(word(cx), Some(crate::agent_session::helpers::mode_word(PermissionMode::Ask).to_string()), "and follows it back");
}

/// A message sent while a question waits queues behind it: the session still needs the reader, and does not say it works.
#[gpui_kit::test]
fn a_message_sent_over_a_waiting_question_leaves_the_session_needing_the_reader(cx: &mut TestAppContext) {
    let (session, _fake, cx) = start(cx, vec![vec![Event::Permission(ask())]], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("write a.txt".into(), cx)));
    cx.run_until_parked();
    let waiting = SessionStatus::NeedsYou(atelier_ui::session_status::Need::Approval);
    assert_eq!(cx.update(|_, cx| session.read(cx).status.clone()), waiting);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("and one more thing".into(), cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| session.read(cx).status.clone()), waiting, "the question still waits");
}
