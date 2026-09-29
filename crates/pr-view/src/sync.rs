//! Keeping a pull request current while it is open. A refresh asks the forge for the header first; when the
//! header is the one the reader has, nothing else is asked, so a quiet pull request costs one small call.
//! When it changed, the parts that change while people work are read again, and the files too when the
//! head moved. How often to ask is [`Cadence`]: often while things happen, less while they do not, and
//! further apart after failures, with a rate limit obeyed. Nothing here draws or blocks the UI.
use std::time::Duration;

use lathe_forge::{Forge, ForgeError, PullRef, PullState};

use crate::{
    data::{Part, PartKind, PullData},
    load::load_parts,
};

/// How often to ask, as the last answers say.
#[derive(Clone, Debug)]
pub struct Cadence {
    base: Duration,
    /// The slowest a quiet pull request is asked about.
    quiet_max: Duration,
    /// The slowest after failures.
    error_max: Duration,
    unchanged: u32,
    failures: u32,
}

impl Cadence {
    /// Asks every `base` while things happen.
    pub fn new(base: Duration) -> Self {
        Self { base, quiet_max: base * 4, error_max: Duration::from_secs(300), unchanged: 0, failures: 0 }
    }

    /// The wait after an answer. Something changed: `base` again. Nothing did: a little longer each time,
    /// up to four times `base`.
    pub fn after_answer(&mut self, changed: bool) -> Duration {
        self.failures = 0;
        if changed {
            self.unchanged = 0;
            return self.base;
        }
        self.unchanged += 1;
        (self.base + self.base * (self.unchanged / 2)).min(self.quiet_max)
    }

    /// The wait after a failure, or `None` when asking again cannot help until the reader acts: the sign-in,
    /// a missing tool, a pull request the reader may not see.
    pub fn after_failure(&mut self, error: &ForgeError) -> Option<Duration> {
        match error {
            ForgeError::NotSignedIn | ForgeError::ToolMissing { .. } | ForgeError::UnknownRemote(_) | ForgeError::Denied(_) | ForgeError::NotFound(_) => None,
            ForgeError::RateLimited { retry_after } => {
                self.failures += 1;
                let asked = retry_after.map_or(Duration::ZERO, |s| Duration::from_secs(s + 1));
                Some(asked.max(self.backoff()).min(Duration::from_secs(3600)))
            }
            _ => {
                self.failures += 1;
                Some(self.backoff())
            }
        }
    }

    fn backoff(&self) -> Duration {
        (self.base * 2u32.saturating_pow(self.failures)).min(self.error_max)
    }
}

/// What a refresh found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refreshed {
    pub changed: bool,
    pub head_moved: bool,
}

/// One refresh of `known`. Parts go to `send` as they arrive. An error is the header's: without it there is
/// nothing to compare, and the failure is the caller's to time.
pub fn refresh(forge: &dyn Forge, reference: &PullRef, known: &PullData, send: &(dyn Fn(Part) + Sync)) -> Result<Refreshed, ForgeError> {
    let pull = forge.pull(reference)?;
    if known.pull.as_ref() == Some(&pull) {
        return Ok(Refreshed::default());
    }
    let head_moved = known.pull.as_ref().is_none_or(|old| old.head_sha != pull.head_sha);
    send(Part::Pull(Box::new(pull)));
    let mut kinds = vec![PartKind::Threads, PartKind::Remarks, PartKind::Checks, PartKind::Held];
    if head_moved {
        kinds.extend([PartKind::Files, PartKind::ReviewPoint]);
    }
    load_parts(forge, reference, &kinds, send);
    Ok(Refreshed { changed: true, head_moved })
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

pub fn delta(old: &PullData, new: &PullData) -> Delta {
    let (Some(a), Some(b)) = (&old.pull, &new.pull) else { return Delta::default() };
    Delta {
        head_moved: a.head_sha != b.head_sha,
        new_comments: new.comment_count().saturating_sub(old.comment_count()),
        state: (a.state != b.state).then_some(b.state),
        checks_changed: a.checks != b.checks || old.checks != new.checks,
        review_changed: a.review != b.review || a.opinions != b.opinions,
    }
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

/// The file to keep open after the file list changed. The same path if it is still there; else the file that
/// now stands where it stood; else the first; `None` for no files.
pub fn keep_place(current: Option<&str>, before: &[String], after: &[String]) -> Option<String> {
    if after.is_empty() {
        return None;
    }
    let current = current?;
    if after.iter().any(|p| p == current) {
        return Some(current.to_string());
    }
    let at = before.iter().position(|p| p == current).unwrap_or(0);
    Some(after[at.min(after.len() - 1)].clone())
}
