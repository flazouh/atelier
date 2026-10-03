use gpui_kit::AppContext;
use gpui_kit::TestAppContext;
use atelier_agents::session::{Choice, ChoiceId, PermissionRequest, RequestId, ToolCall, ToolId, ToolKind, ToolStatus};

use super::*;
use crate::fake_agent::{ended, git_project, git_project_in, start, start_in};

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
    for name in ["files", "tasks", "review", "ship", "tidy"] {
        assert!(names.iter().any(|n| n == name), "{name} is offered: {names:?}");
    }
    assert!(!names.iter().any(|n| n == "goal" || n == "login"), "a command atelier cannot run yet is not offered: {names:?}");
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
    use gpui_kit::TestAppContext;

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
        cx.update(|_, cx| session.update(cx, |s, cx| s.continue_from(source(), cx)));
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
