use std::{io::ErrorKind, process::Command, time::Duration};

use atelier_capabilities::messaging::ChannelKind;

use crate::{RunError, Runner};

/// What a finished command printed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub fn ok(stdout: impl Into<String>) -> Self {
        Self {
            code: 0,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    pub fn failed(stderr: impl Into<String>) -> Self {
        Self {
            code: 1,
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }
}

/// Runs a real program: `slackcli` on the machine that holds the Slack login.
pub struct CommandRunner {
    program: String,
}

impl CommandRunner {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
        }
    }
}

impl Runner for CommandRunner {
    fn run(&self, args: &[String]) -> Result<Output, RunError> {
        let out = Command::new(&self.program)
            .args(args)
            .output()
            .map_err(|e| match e.kind() {
                ErrorKind::NotFound => RunError::NotInstalled(self.program.clone()),
                _ => RunError::Io(e.to_string()),
            })?;
        Ok(Output {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

/// A channel the screen lists. `slackcli` cannot list channels, so the app says which ones the person wants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelSpec {
    /// The Slack id: `C01` for a channel, `D01` for a dm, `G01` for a private channel or a group dm.
    pub id: String,
    pub name: String,
    pub kind: ChannelKind,
}

impl ChannelSpec {
    pub fn new(id: &str, name: &str, kind: ChannelKind) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            kind,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SlackConfig {
    /// The account in references: the workspace name, for example `acme`.
    pub account: String,
    pub channels: Vec<ChannelSpec>,
    /// How often a subscription looks for new messages.
    pub poll_every: Duration,
}

impl SlackConfig {
    pub fn new(account: &str, channels: Vec<ChannelSpec>) -> Self {
        Self {
            account: account.to_string(),
            channels,
            poll_every: Duration::from_secs(5),
        }
    }
}
