use std::time::Duration;

use atelier_forge::{ForgeError, PullState};

/// How often to ask, as the last answers say.
#[derive(Clone, Debug)]
pub struct Cadence {
    pub(super) base: Duration,
    /// The slowest a quiet pull request is asked about.
    pub(super) quiet_max: Duration,
    /// The slowest after failures.
    pub(super) error_max: Duration,
    pub(super) unchanged: u32,
    pub(super) failures: u32,
    /// The wait the last answer or failure asked for.
    pub(super) wait: Duration,
}

impl Cadence {
    /// Asks every `base` while things happen.
    pub fn new(base: Duration) -> Self {
        Self { base, quiet_max: base * 4, error_max: Duration::from_secs(300), unchanged: 0, failures: 0, wait: base }
    }

    /// The wait before the next question, as the last answer or failure set it.
    pub fn next_wait(&self) -> Duration {
        self.wait
    }

    /// The wait after an answer. Something changed: `base` again. Nothing did: a little longer each time,
    /// up to four times `base`.
    pub fn after_answer(&mut self, changed: bool) -> Duration {
        self.failures = 0;
        if changed {
            self.unchanged = 0;
            self.wait = self.base;
            return self.wait;
        }
        self.unchanged += 1;
        self.wait = (self.base + self.base * (self.unchanged / 2)).min(self.quiet_max);
        self.wait
    }

    /// The wait after a failure, or `None` when asking again cannot help until the reader acts: the sign-in,
    /// a missing tool, a pull request the reader may not see.
    pub fn after_failure(&mut self, error: &ForgeError) -> Option<Duration> {
        match error {
            ForgeError::NotSignedIn | ForgeError::ToolMissing { .. } | ForgeError::UnknownRemote(_) | ForgeError::Denied(_) | ForgeError::NotFound(_) => None,
            ForgeError::RateLimited { retry_after } => {
                self.failures += 1;
                let asked = retry_after.map_or(Duration::ZERO, |s| Duration::from_secs(s + 1));
                self.wait = asked.max(self.backoff()).min(Duration::from_secs(3600));
                Some(self.wait)
            }
            _ => {
                self.failures += 1;
                self.wait = self.backoff();
                Some(self.wait)
            }
        }
    }

    pub(super) fn backoff(&self) -> Duration {
        (self.base * 2u32.saturating_pow(self.failures)).min(self.error_max)
    }
}

/// What a refresh found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refreshed {
    pub changed: bool,
    pub head_moved: bool,
}

/// What changed between two readings, for a line under the header ("2 new comments, a new push").
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Delta {
    pub head_moved: bool,
    pub new_comments: usize,
    pub state: Option<PullState>,
    pub checks_changed: bool,
    pub review_changed: bool,
}

impl Delta {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The change in a few words, or `None` when nothing worth telling changed.
    pub fn words(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        if self.head_moved {
            parts.push("a new push".into());
        }
        match self.new_comments {
            0 => {}
            1 => parts.push("1 new comment".into()),
            n => parts.push(format!("{n} new comments")),
        }
        match self.state {
            Some(PullState::Merged) => parts.push("merged".into()),
            Some(PullState::Closed) => parts.push("closed".into()),
            Some(PullState::Draft) => parts.push("now a draft".into()),
            Some(PullState::Open) => parts.push("open again".into()),
            None => {}
        }
        if self.checks_changed {
            parts.push("checks changed".into());
        }
        if self.review_changed {
            parts.push("a review".into());
        }
        if parts.is_empty() {
            return None;
        }
        let mut text = parts.join(", ");
        if let Some(first) = text.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        Some(text)
    }
}
