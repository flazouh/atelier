//! Runs the real Codex adapter (`npx -y @agentclientprotocol/codex-acp`) with a Codex home that holds no login, and checks
//! that atelier tells the session it is signed out: the adapter's first method wants an API key from the environment and
//! answers "internal error" without one, which is not the words to show. It needs Node, not an account or a network once
//! the adapter is fetched, so it is ignored unless asked for:
//! `cargo test -p atelier-agents --test codex_sign_in_live -- --ignored --nocapture`
use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use atelier_agents::{
    acp::Acp,
    codex::Codex,
    session::{Backend, Command, Event, EventSink, OpenRequest},
};
use atelier_project::{LocalProject, Project};

const WAIT: Duration = Duration::from_secs(120);

#[test]
#[ignore = "runs the real Codex adapter"]
fn a_codex_with_no_login_tells_it_is_signed_out() {
    let home = tempfile::tempdir().unwrap();
    // SAFETY: this test is the only one that runs in its process.
    unsafe {
        std::env::set_var("CODEX_HOME", home.path());
        std::env::remove_var("CODEX_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
    }
    let dir = tempfile::tempdir().unwrap();
    let project: Arc<dyn Project> = Arc::new(LocalProject::open(dir.path()).unwrap());
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let sink: EventSink = Arc::new(move |event| drop(tx.lock().unwrap().send(event)));
    let session = Acp::new(Codex::agent()).open(project, OpenRequest::default(), sink).expect("the adapter starts");
    session.send(Command::send("hi")).unwrap();
    let mut seen = Vec::new();
    let deadline = Instant::now() + WAIT;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(event) = rx.recv_timeout(left) else { break };
        let ended = matches!(event, Event::TurnEnded(_) | Event::Ended(_));
        seen.push(event);
        if ended {
            break;
        }
    }
    assert!(seen.contains(&Event::SignedOut), "{seen:#?}");
}
