use std::{
    io::{Read, Write},
    sync::Arc,
};

use atelier_project::{Command, Project};

use super::super::transport::{Reply, Request, Transport, TransportError};
use super::helpers::{classify, parse_reply};

pub struct GhCli {
    pub(super) project: Arc<dyn Project>,
    pub(super) program: String,
}

impl GhCli {
    pub fn new(project: Arc<dyn Project>) -> Self {
        Self { project, program: "gh".into() }
    }

    pub fn with_program(mut self, program: impl Into<String>) -> Self {
        self.program = program.into();
        self
    }

    pub(super) fn command(&self, request: &Request) -> Command {
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
            None => Err(classify(code, &process.control.stderr())),
        }
    }
}
