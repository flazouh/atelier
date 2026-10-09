use std::time::Duration;

pub(crate) const PROVIDER: &str = "gmail";

/// How many threads one Gmail search page shows in the browser. `gmailcli` reads one page, so nothing beyond this many is
/// reachable through it. This is the browser's page size as observed on the web page; it is not read from the tool.
pub(crate) const VISIBLE_THREADS: usize = 50;
pub(crate) const DEFAULT_LIMIT: usize = 20;

/// What the screen waits after a rate limit. Gmail gives no hint, so this is a guess.
pub(crate) const RATE_LIMIT_WAIT_MS: u64 = 60_000;

/// One call of `gmailcli` takes 6 to 10 s, so the default poll is slow on purpose.
pub(crate) const POLL_DEFAULT: Duration = Duration::from_secs(60);

/// How many inbox threads the poll reads.
pub(crate) const POLL_THREADS: usize = 20;

/// Why a run did not give output. The text of a failure is what `gmailcli` printed, which says what went wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunFailure {
    /// The program did not start.
    Spawn(String),
    /// The program ran and failed. `gmailcli` prints its error on stdout.
    Exit {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },
}

impl RunFailure {
    /// Everything the run printed, for the failure mapping.
    pub fn text(&self) -> String {
        match self {
            Self::Spawn(message) => message.clone(),
            Self::Exit { stdout, stderr, .. } => format!("{stdout}\n{stderr}"),
        }
    }
}
