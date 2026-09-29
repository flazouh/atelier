//! Our own agent against the real Anthropic API. Ignored by default: it needs `ANTHROPIC_API_KEY` and it
//! spends a few cents. Run it by hand:
//! `ANTHROPIC_API_KEY=... cargo test -p lathe-agents --test own_live -- --ignored --nocapture --test-threads=1`
//! It uses Haiku 4.5, the cheapest model. The key is read from the environment and is never printed.
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use lathe_agents::{
    own::{OwnAgent, OwnOptions, Secret, anthropic_models},
    session::{Backend, Command, Event, OpenRequest, PermissionMode, TurnOutcome},
};
use lathe_project::LocalProject;

fn key() -> Option<Secret> {
    std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.trim().is_empty()).map(|k| Secret::new(k.trim()))
}

fn agent(key: Secret) -> OwnAgent {
    let model = lathe_agents::own::Anthropic::new(key);
    let options = OwnOptions { default_model: "claude-haiku-4-5".into(), models: anthropic_models(), max_tokens: 2000, max_steps: 6, ..OwnOptions::default() };
    OwnAgent::new(Arc::new(model), options)
}

/// Runs one message to the end of its turn and returns every event.
fn run(agent: &OwnAgent, project: Arc<LocalProject>, message: &str, mode: PermissionMode) -> Vec<Event> {
    let events: Arc<Mutex<Vec<Event>>> = Arc::default();
    let log = events.clone();
    let sink: lathe_agents::session::EventSink = Arc::new(move |e| log.lock().unwrap().push(e));
    let session = agent.open(project, OpenRequest { mode: Some(mode), ..OpenRequest::default() }, sink).unwrap();
    session.send(Command::send(message)).unwrap();
    let start = Instant::now();
    loop {
        if events.lock().unwrap().iter().any(|e| matches!(e, Event::TurnEnded(_))) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(120), "the turn did not end: {:#?}", events.lock().unwrap());
        std::thread::sleep(Duration::from_millis(50));
    }
    events.lock().unwrap().clone()
}

fn turn_outcome(events: &[Event]) -> TurnOutcome {
    events.iter().find_map(|e| if let Event::TurnEnded(t) = e { Some(t.outcome.clone()) } else { None }).unwrap()
}

#[test]
#[ignore = "calls the real API; needs ANTHROPIC_API_KEY"]
fn a_short_turn_streams_text_and_reports_usage() {
    let Some(key) = key() else { return println!("ANTHROPIC_API_KEY is not set: nothing to run") };
    let dir = tempfile::tempdir().unwrap();
    let project = Arc::new(LocalProject::open(dir.path()).unwrap());
    let events = run(&agent(key.clone()), project, "Reply with exactly one word: pong", PermissionMode::Ask);
    assert_eq!(turn_outcome(&events), TurnOutcome::Completed, "{events:#?}");
    let text: String = events.iter().filter_map(|e| if let Event::Text { delta, .. } = e { Some(delta.as_str()) } else { None }).collect();
    assert!(text.to_lowercase().contains("pong"), "{text}");
    let usage = events.iter().find_map(|e| if let Event::Usage(u) = e { Some(*u) } else { None }).expect("usage");
    assert!(usage.input_tokens + usage.cache_read_tokens + usage.cache_write_tokens > 0 && usage.output_tokens > 0, "{usage:?}");
    assert!(!format!("{events:?}").contains(key.expose()), "the key must not appear in any event");
    println!("live turn ok: {text:?}, usage {usage:?}");
}

#[test]
#[ignore = "calls the real API; needs ANTHROPIC_API_KEY"]
fn a_tool_turn_reads_a_file_and_answers_from_it() {
    let Some(key) = key() else { return println!("ANTHROPIC_API_KEY is not set: nothing to run") };
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("secret.txt"), "The magic word is tangerine.\n").unwrap();
    let project = Arc::new(LocalProject::open(dir.path()).unwrap());
    let events = run(&agent(key), project, "Read secret.txt with your read tool and tell me the magic word.", PermissionMode::Ask);
    assert_eq!(turn_outcome(&events), TurnOutcome::Completed, "{events:#?}");
    assert!(events.iter().any(|e| matches!(e, Event::ToolStarted(c) if c.name == "read")), "the model used the read tool");
    let text: String = events.iter().filter_map(|e| if let Event::Text { delta, .. } = e { Some(delta.as_str()) } else { None }).collect();
    assert!(text.to_lowercase().contains("tangerine"), "{text}");
    println!("live tool turn ok: {text:?}");
}
