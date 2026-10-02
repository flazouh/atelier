use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use atelier_project::LocalProject;

use super::{fake::FakeBackend, *};

fn project() -> Arc<dyn atelier_project::Project> {
    Arc::new(LocalProject::open(std::env::temp_dir()).expect("the temp folder opens"))
}

fn collect() -> (EventSink, Arc<Mutex<Vec<Event>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let store = seen.clone();
    (Arc::new(move |event| store.lock().unwrap().push(event)), seen)
}

fn text(block: u64, delta: &str) -> Event {
    Event::Text { block: BlockId(block), delta: delta.into() }
}

#[test]
fn a_backend_with_no_process_runs_a_turn_through_the_trait() {
    let backend = FakeBackend::new(vec![vec![text(1, "Hello"), Event::TurnEnded(TurnEnd {
        outcome: TurnOutcome::Completed,
        summary: Some("Hello".into()),
    })]]);
    let (sink, seen) = collect();
    let session = backend.open(project(), OpenRequest::default(), sink).unwrap();
    session.send(Command::send("hi")).unwrap();
    drop(session);
    let events = seen.lock().unwrap().clone();
    assert!(matches!(events[0], Event::Started(_)));
    assert_eq!(events[1], text(1, "Hello"));
    assert!(matches!(events[2], Event::TurnEnded(TurnEnd { outcome: TurnOutcome::Completed, .. })));
    assert_eq!(events[3], Event::Ended(EndReason::Closed));
}

#[test]
fn the_fake_session_records_commands_and_resumes_by_id() {
    let backend = FakeBackend::new(vec![vec![]]);
    let (sink, seen) = collect();
    let request = OpenRequest { resume: Some(SessionId::new("old")), ..OpenRequest::default() };
    let session = backend.open(project(), request, sink).unwrap();
    session.send(Command::SetModel { model: "m".into() }).unwrap();
    session.send(Command::Interrupt).unwrap();
    assert_eq!(backend.received(), vec![Command::SetModel { model: "m".into() }, Command::Interrupt]);
    let events = seen.lock().unwrap().clone();
    let Event::Started(started) = &events[0] else { panic!("no Started event") };
    assert_eq!(started.session, SessionId::new("old"));
    assert!(matches!(events[1], Event::TurnEnded(TurnEnd { outcome: TurnOutcome::Interrupted, .. })));
}

#[test]
fn a_send_after_the_script_ends_reports_a_closed_session() {
    let backend = FakeBackend::new(vec![]);
    let (sink, _) = collect();
    let session = backend.open(project(), OpenRequest::default(), sink).unwrap();
    assert!(matches!(session.send(Command::send("hi")), Err(SessionError::Closed)));
}

#[test]
fn a_backend_offers_no_list_and_no_history_until_it_says_so() {
    let backend = FakeBackend::new(vec![]);
    let project = project();
    assert!(matches!(backend.sessions(project.as_ref()), Err(SessionError::Unsupported(_))));
    assert!(matches!(backend.history(project.as_ref(), &SessionId::new("x")), Err(SessionError::Unsupported(_))));
}

#[test]
fn capabilities_default_to_nothing_so_the_ui_hides_everything() {
    let none = Backend::capabilities(&FakeBackend::new(vec![]));
    assert_eq!(none, Capabilities::default());
    assert!(!none.resume && !none.interrupt && none.models.is_empty() && none.permission_modes.is_empty());
}

#[test]
fn a_backend_says_what_it_supports() {
    let offer = Capabilities { resume: true, permission_modes: vec![PermissionMode::Plan], ..Capabilities::default() };
    let backend = FakeBackend::new(vec![]).offering(offer.clone());
    assert_eq!(Backend::capabilities(&backend), offer);
}

#[test]
fn the_queue_joins_the_deltas_of_one_block_and_wakes_once_per_batch() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let counter = wakes.clone();
    let queue = EventQueue::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    let sink = queue.sink();
    for word in ["a", "b", "c"] {
        sink(text(1, word));
    }
    sink(text(2, "d"));
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
    assert_eq!(queue.drain(), vec![text(1, "abc"), text(2, "d")]);
    sink(text(2, "e"));
    assert_eq!(wakes.load(Ordering::SeqCst), 2);
    assert_eq!(queue.drain(), vec![text(2, "e")]);
    assert!(queue.drain().is_empty());
}

#[test]
fn thinking_deltas_join_but_text_and_thinking_stay_apart() {
    let queue = EventQueue::new(|| {});
    queue.push(Event::Thinking { block: BlockId(1), delta: "x".into() });
    queue.push(Event::Thinking { block: BlockId(1), delta: "y".into() });
    queue.push(text(1, "z"));
    assert_eq!(
        queue.drain(),
        vec![Event::Thinking { block: BlockId(1), delta: "xy".into() }, text(1, "z")]
    );
}

/// An edit is told again with each piece of its text; in one frame only the newest telling of a call counts.
#[test]
fn the_tellings_of_an_edit_in_one_batch_keep_only_the_newest() {
    let queue = EventQueue::new(|| {});
    let edit = |id: &str, new: &str| Event::ToolEdit { id: ToolId::new(id), edit: FileEdit { path: "a".into(), old: String::new(), new: new.into() } };
    queue.push(edit("t1", "a"));
    queue.push(edit("t1", "ab"));
    queue.push(edit("t2", "x"));
    queue.push(edit("t1", "abc"));
    assert_eq!(queue.drain(), vec![edit("t1", "ab"), edit("t2", "x"), edit("t1", "abc")]);
}

mod conversation {
    use std::time::Duration;

    use serde_json::{Value, json};

    use super::super::*;

    fn call(id: &str, name: &str, parent: Option<&str>) -> ToolCall {
        ToolCall {
            id: ToolId::new(id),
            name: name.into(),
            kind: ToolKind::Other,
            input: Value::Null,
            file: None,
            parent: parent.map(ToolId::new),
            status: ToolStatus::Running,
        }
    }

    fn fold(events: Vec<Event>) -> Conversation {
        let mut conversation = Conversation::new();
        events.iter().for_each(|event| conversation.apply(event));
        conversation
    }

    fn text(block: u64, delta: &str) -> Event {
        Event::Text { block: BlockId(block), delta: delta.into() }
    }

    #[test]
    fn deltas_of_a_block_join_and_a_new_block_starts_a_new_item() {
        let conversation = fold(vec![text(1, "Hel"), text(1, "lo"), text(2, "Again")]);
        assert_eq!(
            conversation.items(),
            [Item::Text { block: BlockId(1), text: "Hello".into() }, Item::Text { block: BlockId(2), text: "Again".into() }]
        );
    }

    #[test]
    fn thinking_gets_its_time_when_it_ends() {
        let conversation = fold(vec![
            Event::Thinking { block: BlockId(1), delta: String::new() },
            Event::Thinking { block: BlockId(1), delta: "hm".into() },
            Event::ThinkingDone { block: BlockId(1), took: Duration::from_secs(2) },
        ]);
        assert_eq!(
            conversation.items(),
            [Item::Thinking { block: BlockId(1), text: "hm".into(), took: Some(Duration::from_secs(2)) }]
        );
    }

    #[test]
    fn a_tool_call_goes_from_running_to_done_with_its_input_file_and_output() {
        let id = ToolId::new("t1");
        let conversation = fold(vec![
            Event::ToolStarted(call("t1", "Read", None)),
            Event::ToolInput { id: id.clone(), input: json!({"a": 1}), file: Some("/a.rs".into()) },
            Event::ToolFinished { id, output: ToolOutput { text: "x".into(), is_error: false, truncated: false, full_at: None } },
        ]);
        let [Item::Tool(done)] = conversation.items() else { panic!("{:?}", conversation.items()) };
        assert_eq!((done.call.status, done.call.file.as_deref()), (ToolStatus::Done, Some("/a.rs")));
        assert_eq!(done.call.input, json!({"a": 1}));
        assert_eq!(done.output.as_ref().unwrap().text, "x");
    }

    /// An edit's text, as far as the agent has written it, is kept on its call; each telling replaces the last, and a
    /// subagent's call keeps its own.
    #[test]
    fn the_text_of_an_edit_is_kept_on_its_call_as_it_grows() {
        let edit = |new: &str| FileEdit { path: "/w/a.rs".into(), old: "a".into(), new: new.into() };
        let conversation = fold(vec![
            Event::ToolStarted(call("t1", "Edit", None)),
            Event::ToolEdit { id: ToolId::new("t1"), edit: edit("b") },
            Event::ToolEdit { id: ToolId::new("t1"), edit: edit("b\nc") },
        ]);
        let [Item::Tool(call)] = conversation.items() else { panic!("{:?}", conversation.items()) };
        assert_eq!(call.edit, Some(edit("b\nc")));
        assert_eq!(call.call.input, serde_json::Value::Null, "the input stays the agent's own, unknown until it is whole");
    }

    #[test]
    fn a_failed_output_marks_the_call_failed() {
        let conversation = fold(vec![
            Event::ToolStarted(call("t1", "Bash", None)),
            Event::ToolFinished {
                id: ToolId::new("t1"),
                output: ToolOutput { text: "no".into(), is_error: true, truncated: false, full_at: None },
            },
        ]);
        assert!(matches!(&conversation.items()[0], Item::Tool(c) if c.call.status == ToolStatus::Failed));
    }

    #[test]
    fn a_subagent_holds_its_own_calls_and_ends_with_a_status() {
        let id = ToolId::new("s1");
        let conversation = fold(vec![
            Event::SubagentStarted(Subagent { id: id.clone(), task: "look".into(), kind: None, model: None }),
            Event::ToolStarted(call("t1", "Read", Some("s1"))),
            Event::SubagentProgress { id: id.clone(), activity: "Reading".into() },
            Event::ToolFinished {
                id: ToolId::new("t1"),
                output: ToolOutput { text: "ok".into(), is_error: false, truncated: false, full_at: None },
            },
            Event::ToolStarted(call("t2", "Bash", None)),
            Event::SubagentEnded { id, ok: true, summary: Some("done".into()) },
        ]);
        assert_eq!(conversation.items().len(), 2, "the subagent and the top-level call");
        let Item::Subagent { status, calls, activity, summary, .. } = &conversation.items()[0] else { panic!() };
        assert_eq!((*status, activity.as_deref(), summary.as_deref()), (SubagentStatus::Done, None, Some("done")));
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].call.status, ToolStatus::Done);
        assert_eq!(conversation.running_subagents(), 0);
    }

    #[test]
    fn a_permission_question_is_asking_then_answered_or_withdrawn() {
        let request = |id: &str| PermissionRequest {
            id: RequestId::new(id),
            call: call("t", "Write", None),
            reason: None,
            choices: vec![],
        };
        let mut conversation = fold(vec![Event::Permission(request("a")), Event::Permission(request("b"))]);
        conversation.answered(&RequestId::new("a"), ChoiceKind::Allow);
        conversation.apply(&Event::PermissionCancelled(RequestId::new("b")));
        conversation.apply(&Event::PermissionCancelled(RequestId::new("a")));
        let answers: Vec<_> = conversation
            .items()
            .iter()
            .filter_map(|item| if let Item::Permission { answer, .. } = item { Some(*answer) } else { None })
            .collect();
        assert_eq!(answers, [Answer::Answered(ChoiceKind::Allow), Answer::Withdrawn], "an answered request stays answered");
    }

    #[test]
    fn a_turn_opens_when_the_user_sends_and_closes_with_its_outcome() {
        let mut conversation = Conversation::new();
        assert!(!conversation.working());
        conversation.user_sent("hi");
        assert!(conversation.working());
        conversation.apply(&Event::TurnEnded(TurnEnd { outcome: TurnOutcome::Failed("boom".into()), summary: None }));
        assert!(!conversation.working());
        assert_eq!(conversation.items(), [Item::User { text: "hi".into() }, Item::Notice("boom".into())]);
    }

    #[test]
    fn usage_adds_up_across_turns_and_cost_stays_unknown_until_reported() {
        let usage = |cost| Event::Usage(Usage { input_tokens: 10, output_tokens: 5, cost_usd: cost, ..Usage::default() });
        let mut conversation = fold(vec![usage(None), usage(None)]);
        assert_eq!((conversation.usage().input_tokens, conversation.usage().cost_usd), (20, None));
        conversation.apply(&usage(Some(0.25)));
        assert_eq!((conversation.usage().output_tokens, conversation.usage().cost_usd), (15, Some(0.25)));
    }

    #[test]
    fn the_todo_list_is_the_last_one_sent() {
        let todo = |text: &str| Todo { id: "1".into(), text: text.into(), status: TodoStatus::Pending };
        let conversation = fold(vec![Event::Todos(vec![todo("a"), todo("b")]), Event::Todos(vec![todo("c")])]);
        assert_eq!(conversation.todos(), [todo("c")]);
    }

    #[test]
    fn an_end_stops_the_work_and_keeps_the_reason() {
        let mut conversation = Conversation::new();
        conversation.user_sent("hi");
        conversation.apply(&Event::Ended(EndReason::Exited { code: Some(1), stderr: "boom".into() }));
        assert!(!conversation.working());
        assert_eq!(conversation.ended(), Some(&EndReason::Exited { code: Some(1), stderr: "boom".into() }));
    }

    #[test]
    fn events_about_a_call_that_was_never_started_change_nothing() {
        let conversation = fold(vec![
            Event::ToolStatus { id: ToolId::new("ghost"), status: ToolStatus::Done },
            Event::SubagentProgress { id: ToolId::new("ghost"), activity: "x".into() },
            Event::ThinkingDone { block: BlockId(9), took: Duration::ZERO },
        ]);
        assert!(conversation.items().is_empty());
    }
}

mod attachments {
    use super::super::*;

    #[test]
    fn one_line_is_said_in_the_singular_and_a_range_in_the_plural() {
        let one = Attachment::LineComment { path: "a".into(), first_line: 3, last_line: 3, removed: false, quote: "x".into(), body: "b".into() };
        assert_eq!(one.render(), "Review comment on a, line 3:\n> x\nb");
    }

    #[test]
    fn a_message_with_no_attachment_is_its_text_alone() {
        assert_eq!(message_text("hi", &[]), "hi");
        assert_eq!(Command::send("hi"), Command::Send { text: "hi".into(), attachments: vec![] });
    }

    #[test]
    fn a_session_receives_the_attachments_the_message_carried() {
        let backend = fake::FakeBackend::new(vec![vec![]]);
        let sink: EventSink = std::sync::Arc::new(|_| {});
        let project: std::sync::Arc<dyn atelier_project::Project> =
            std::sync::Arc::new(atelier_project::LocalProject::open(std::env::temp_dir()).unwrap());
        let session = backend.open(project, OpenRequest::default(), sink).unwrap();
        let file = Attachment::File { path: "a".into() };
        session.send(Command::Send { text: "look".into(), attachments: vec![file.clone()] }).unwrap();
        assert_eq!(backend.received(), vec![Command::Send { text: "look".into(), attachments: vec![file] }]);
    }
}
