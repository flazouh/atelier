use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use lathe_project::LocalProject;

use super::{fake::FakeBackend, *};

fn project() -> Arc<dyn lathe_project::Project> {
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
    session.send(Command::Send { text: "hi".into() }).unwrap();
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
    assert!(matches!(session.send(Command::Send { text: "hi".into() }), Err(SessionError::Closed)));
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
