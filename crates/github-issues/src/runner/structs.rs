use std::{
    io::Write,
    process::{Command, Stdio},
};

use super::{
    Gh,
    helpers::{classify, parse_reply},
    types::{Failure, Method},
};

/// One request to the REST API. `path` starts with `/` and may hold a query.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub method: Method,
    pub path: String,
    pub body: Option<serde_json::Value>,
    /// Sent as `If-None-Match`: GitHub answers 304 with no body, and a 304 does not count against the rate limit.
    pub if_none_match: Option<String>,
}

impl Call {
    pub fn get(path: impl Into<String>) -> Self {
        Self {
            method: Method::Get,
            path: path.into(),
            body: None,
            if_none_match: None,
        }
    }

    pub fn post(path: impl Into<String>, body: serde_json::Value) -> Self {
        Self {
            method: Method::Post,
            body: Some(body),
            ..Self::get(path)
        }
    }

    pub fn patch(path: impl Into<String>, body: serde_json::Value) -> Self {
        Self {
            method: Method::Patch,
            body: Some(body),
            ..Self::get(path)
        }
    }

    pub fn with_etag(mut self, etag: Option<String>) -> Self {
        self.if_none_match = etag;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    /// Names in lower case.
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Reply {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// `gh api` as a child process. It reads the sign-in of the person who runs the app.
pub struct GhCli {
    program: String,
}

impl Default for GhCli {
    fn default() -> Self {
        Self {
            program: "gh".into(),
        }
    }
}

impl GhCli {
    /// Another program than `gh` on the path: a test's stand-in.
    pub fn with_program(mut self, program: impl Into<String>) -> Self {
        self.program = program.into();
        self
    }
}

impl Gh for GhCli {
    fn send(&self, call: &Call) -> Result<Reply, Failure> {
        // `--include` puts the status line and the headers before the body, so an error status keeps its body and a
        // rate limit shows its reset.
        let mut command = Command::new(&self.program);
        command.args(["api", "--include", "--method", call.method.as_str()]);
        if let Some(tag) = &call.if_none_match {
            command.args(["-H", &format!("If-None-Match: {tag}")]);
        }
        if call.body.is_some() {
            command.args(["--input", "-"]);
        }
        let mut child = command
            .arg(&call.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => Failure::ToolMissing,
                _ => Failure::Other(error.to_string()),
            })?;
        if let Some(mut stdin) = child.stdin.take()
            && let Some(body) = &call.body
        {
            // A gh that quit before it read the body says why on stderr and in its exit code: that is read below.
            let _ = stdin.write_all(body.to_string().as_bytes());
        }
        let output = child
            .wait_with_output()
            .map_err(|error| Failure::Other(error.to_string()))?;
        match parse_reply(&String::from_utf8_lossy(&output.stdout)) {
            Some(reply) => Ok(reply),
            None => Err(classify(
                output.status.code(),
                &String::from_utf8_lossy(&output.stderr),
            )),
        }
    }
}
