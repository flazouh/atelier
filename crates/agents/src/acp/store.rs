//! An ACP agent keeps its own sessions, so atelier asks it for them: a short connection that lists the
//! project's sessions (`session/list`) or replays one (`session/load`), then stops the agent. It runs
//! through the project, so a remote project's sessions are read on its host.
use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use atelier_project::{Process, Project};

use super::{
    AcpAgent,
    protocol::{Found, Goal, Protocol},
};
use crate::{
    session::{Event, SessionError, SessionId, SessionSummary},
    subprocess,
};

/// How long a list or a history may take before atelier gives up on the agent.
const PATIENCE: Duration = Duration::from_secs(60);
/// How long an agent that closed its stdout has to exit before atelier stops it: it says nothing more.
const EXIT_GRACE: Duration = Duration::from_secs(2);

pub(super) fn list(agent: Arc<AcpAgent>, project: &dyn Project) -> Result<Vec<SessionSummary>, SessionError> {
    match ask(agent, project, Goal::List)? {
        Found::Sessions(sessions) => Ok(sessions),
        Found::History(_) => Err(SessionError::Read("the agent answered a list with a history".into())),
    }
}

pub(super) fn history(agent: Arc<AcpAgent>, project: &dyn Project, session: &SessionId) -> Result<Vec<Event>, SessionError> {
    match ask(agent, project, Goal::History(session.clone()))? {
        Found::History(events) => Ok(events),
        Found::Sessions(_) => Err(SessionError::Read("the agent answered a history with a list".into())),
    }
}

/// Starts the agent, runs the protocol for `goal` until it has found what it asked, and stops the agent.
fn ask(agent: Arc<AcpAgent>, project: &dyn Project, goal: Goal) -> Result<Found, SessionError> {
    let Process { mut stdin, stdout, mut control } = subprocess::start(project, &agent.command())?;
    let (mut protocol, first) = Protocol::new(agent, project.root().display().to_string(), goal);
    let (lines, read) = mpsc::channel();
    thread::spawn(move || subprocess::lines(stdout).try_for_each(|line| lines.send(line)));
    let deadline = Instant::now() + PATIENCE;
    let mut write = |lines: Vec<String>| -> Result<(), SessionError> {
        use std::io::Write;
        lines.into_iter().try_for_each(|line| writeln!(stdin, "{line}").and_then(|()| stdin.flush()))
            .map_err(|e| SessionError::Read(format!("could not write to the agent: {e}")))
    };
    let found = (|| {
        write(first)?;
        loop {
            let left = deadline.checked_duration_since(Instant::now()).unwrap_or_default();
            match read.recv_timeout(left) {
                Ok(line) => {
                    let step = protocol.line(&line, Instant::now());
                    write(step.lines)?;
                    if step.done {
                        return protocol.found().unwrap_or(Err(SessionError::Read("the agent found nothing".into())));
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => return Err(SessionError::Read("the agent did not answer in time".into())),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let grace = Instant::now() + EXIT_GRACE;
                    while control.running() && Instant::now() < grace {
                        thread::sleep(Duration::from_millis(20));
                    }
                    if control.running() {
                        return Err(SessionError::Read("the agent closed its output and did not exit".into()));
                    }
                    let code = control.wait().ok().flatten();
                    protocol.exited(code, &control.stderr(), Instant::now());
                    return protocol.found().unwrap_or(Err(SessionError::Read("the agent stopped".into())));
                }
            }
        }
    })();
    let _ = control.kill();
    let _ = control.wait();
    found
}
