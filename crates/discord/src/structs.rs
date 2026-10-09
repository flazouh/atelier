use std::{io::ErrorKind, process::Command, time::Duration};

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

/// Runs a real program: `discordcli` on the machine that holds the Discord login.
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

/// What one [`DiscordMessaging`](crate::DiscordMessaging) serves.
#[derive(Clone, Debug)]
pub struct DiscordConfig {
    /// The account in references: the id of one Discord server, or `dm` for the direct messages of the person.
    pub account: String,
    /// Also serve the direct messages, under the account `dm`. A server account that does this cannot search, because
    /// `discordcli` searches one server only.
    pub include_dms: bool,
    /// Whether `send` may run. It is `false` by default: the login is a user token, and Discord does not allow
    /// automation with one. See `docs/capabilities/discord-notes.md`.
    pub allow_writes: bool,
    /// How often a subscription looks for new messages. Every look is a request on a user token, so keep it slow.
    pub poll_every: Duration,
    /// The most channels one subscription polls. A subscription for more is refused, so a screen that asks for "all"
    /// cannot send hundreds of requests every few seconds.
    pub max_polled: usize,
}

impl DiscordConfig {
    /// A read-only provider for the server with this id, or for the direct messages when `account` is `dm`.
    pub fn new(account: &str) -> Self {
        Self {
            account: account.to_string(),
            include_dms: false,
            allow_writes: false,
            poll_every: Duration::from_secs(15),
            max_polled: 20,
        }
    }
}
