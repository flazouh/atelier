//! The transport that runs `gh api` through the project, so a remote project uses its host's `gh`. `gh`
//! holds the token and adds it to the request itself: it never passes through lathe, so lathe has none
//! to store, log or print.
use std::{
    io::{Read, Write},
    sync::Arc,
};

use lathe_project::{Command, Project};

use super::transport::{Reply, Request, Transport, TransportError};

/// `gh` exits with 4 when it needs a sign-in.
const NEEDS_SIGN_IN: i32 = 4;

pub struct GhCli {
    project: Arc<dyn Project>,
    program: String,
}

impl GhCli {
    pub fn new(project: Arc<dyn Project>) -> Self {
        Self { project, program: "gh".into() }
    }

    pub fn with_program(mut self, program: impl Into<String>) -> Self {
        self.program = program.into();
        self
    }

    fn command(&self, request: &Request) -> Command {
        // `--include` puts the status line and the headers before the body, so a failure keeps its body
        // and a rate limit shows its reset.
        let mut args = vec!["api".to_string(), "--include".into(), "--method".into(), request.method.into()];
        if request.body.is_some() {
            args.extend(["--input".into(), "-".into()]);
        }
        args.push(request.path.clone());
        Command::new(&self.program).args(args)
    }
}

impl Transport for GhCli {
    fn send(&self, request: &Request) -> Result<Reply, TransportError> {
        let mut process = self.project.spawn(&self.command(request)).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => TransportError::ToolMissing,
            _ => TransportError::Failed(error.to_string()),
        })?;
        if let Some(body) = &request.body {
            process.stdin.write_all(body.as_bytes()).map_err(|e| TransportError::Failed(e.to_string()))?;
        }
        drop(process.stdin);
        let mut bytes = Vec::new();
        process.stdout.read_to_end(&mut bytes).map_err(|e| TransportError::Failed(e.to_string()))?;
        let code = process.control.wait().ok().flatten();
        let text = String::from_utf8_lossy(&bytes);
        match parse_reply(&text) {
            Some(reply) => Ok(reply),
            // Nothing came back. `gh` exits 4 when signed out; a failure with no reply at all is a
            // connection that never opened.
            None if code == Some(NEEDS_SIGN_IN) => Err(TransportError::NotSignedIn),
            None if code == Some(1) => Err(TransportError::Offline),
            None => Err(TransportError::Failed(format!("gh ended with {code:?} and no reply"))),
        }
    }
}

/// `gh api --include`'s output: `HTTP/2.0 200 OK`, the headers, a blank line, then the body.
pub(super) fn parse_reply(text: &str) -> Option<Reply> {
    let text = text.strip_prefix("HTTP/")?;
    let (head, body) = text.split_once("\r\n\r\n").or_else(|| text.split_once("\n\n")).unwrap_or((text, ""));
    let mut lines = head.lines();
    let status = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    Some(Reply { status, headers, body: body.to_string() })
}

#[cfg(test)]
mod tests;
