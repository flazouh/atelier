use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use atelier_project::{Process, Project};

use super::super::{
    AcpAgent,
    protocol::{Found, Goal, Protocol},
};
use crate::{
    session::{Event, SessionError, SessionId, SessionSummary},
    subprocess,
};
use super::types::{EXIT_GRACE, PATIENCE};

pub(in super::super) fn list(agent: Arc<AcpAgent>, project: &dyn Project) -> Result<Vec<SessionSummary>, SessionError> {
    match ask(agent, project, Goal::List)? {
        Found::Sessions(sessions) => Ok(sessions),
        Found::History(_) => Err(SessionError::Read("the agent answered a list with a history".into())),
    }
}

pub(in super::super) fn history(agent: Arc<AcpAgent>, project: &dyn Project, session: &SessionId) -> Result<Vec<Event>, SessionError> {
    match ask(agent, project, Goal::History(session.clone()))? {
        Found::History(events) => Ok(events),
        Found::Sessions(_) => Err(SessionError::Read("the agent answered a history with a list".into())),
    }
}

/// Starts the agent, runs the protocol for `goal` until it has found what it asked, and stops the agent.
pub(super) fn ask(agent: Arc<AcpAgent>, project: &dyn Project, goal: Goal) -> Result<Found, SessionError> {
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
