//! Pushing the branch to `origin`, and pulling with a rebase when the remote moved on. Never forced.
//! Git never waits on a login prompt: with none, a push that needs one fails and says so. Blocking:
//! call it off the UI thread.
use lathe_project::{Command, Project};

use crate::ship::commit::run;

/// Why a push did not happen, as the reader's next step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PushError {
    /// The repository has no remote named `origin`.
    NoRemote,
    /// The remote has commits the branch lacks.
    Rejected,
    /// The remote could not be reached; git's words.
    Offline(String),
    /// The remote refused the login, or none was given; git's words.
    Login(String),
    /// Anything else, such as a rule on the remote; git's words.
    Git(String),
}

impl std::fmt::Display for PushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRemote => write!(f, "This repository has no remote named origin, so there is nowhere to push"),
            Self::Rejected => write!(f, "The remote has commits this branch does not have. Pull and rebase, then push"),
            Self::Offline(words) => write!(f, "Could not reach the remote ({}). Push again when the network is back", first(words)),
            Self::Login(words) => write!(f, "The remote refused the login ({}). Sign in with gh auth login, then push again", first(words)),
            Self::Git(words) => write!(f, "The push failed: {}", first(words)),
        }
    }
}

/// Why a pull with a rebase stopped. Each leaves the branch and the files as they were.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RebaseError {
    /// The working tree holds edits no commit has; they are the reader's, and stay.
    OtherEdits,
    /// The remote's commits and the branch's change the same lines of these files.
    Conflict(Vec<String>),
    Offline(String),
    Git(String),
}

impl std::fmt::Display for RebaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OtherEdits => write!(f, "Pull and rebase needs the other edits committed or undone first; nothing changed"),
            Self::Conflict(files) => write!(f, "The remote changed the same lines in {}. Nothing changed: the rebase was undone", files.join(", ")),
            Self::Offline(words) => write!(f, "Could not reach the remote ({})", first(words)),
            Self::Git(words) => write!(f, "Pull and rebase failed: {}", first(words)),
        }
    }
}

/// The line of git's words that says what happened: the first `fatal:` or `error:` line, else the
/// first line.
fn first(words: &str) -> &str {
    let lines = || words.lines().map(str::trim).filter(|l| !l.is_empty());
    lines().find(|l| l.starts_with("fatal:") || l.starts_with("error:")).or_else(|| lines().next()).unwrap_or(words)
}

/// `git args` with no login prompt, in the project's root.
fn git(project: &dyn Project, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git").args(args.iter().copied());
    command.env.push(("GIT_TERMINAL_PROMPT".into(), "0".into()));
    command.env.push(("GIT_SSH_COMMAND".into(), "ssh -o BatchMode=yes".into()));
    run(project, command, None)
}

fn has_origin(project: &dyn Project) -> bool {
    git(project, &["remote", "get-url", "origin"]).is_ok()
}

/// Sorts git's words for a failed push into the reader's next step.
pub fn classify(words: &str) -> PushError {
    let lower = words.to_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));
    if any(&["[rejected]", "non-fast-forward", "fetch first"]) {
        PushError::Rejected
    } else if any(&["could not resolve host", "network is unreachable", "connection timed out", "connection refused", "could not connect", "failed to connect", "temporary failure in name resolution"]) {
        PushError::Offline(words.to_string())
    } else if any(&["authentication failed", "permission denied", "could not read username", "returned error: 403", "returned error: 401", "denied to"]) {
        PushError::Login(words.to_string())
    } else {
        PushError::Git(words.to_string())
    }
}

/// Pushes `branch` to `origin` and makes it track `origin/<branch>`.
pub fn push(project: &dyn Project, branch: &str) -> Result<(), PushError> {
    if !has_origin(project) {
        return Err(PushError::NoRemote);
    }
    git(project, &["push", "-u", "origin", branch]).map(drop).map_err(|words| classify(&words))
}

/// Pulls `origin/<branch>` and puts the branch's own commits on top. A conflict undoes the rebase.
pub fn pull_rebase(project: &dyn Project, branch: &str) -> Result<(), RebaseError> {
    if !has_origin(project) {
        return Err(RebaseError::Git("no remote named origin".into()));
    }
    // The reader's other edits are theirs: the rebase does not put them aside.
    let edits = git(project, &["status", "--porcelain", "--untracked-files=no"]).map_err(RebaseError::Git)?;
    if !edits.trim().is_empty() {
        return Err(RebaseError::OtherEdits);
    }
    let Err(words) = git(project, &["-c", "rebase.autoStash=false", "pull", "--rebase", "--no-autostash", "origin", branch]) else {
        return Ok(());
    };
    let conflicted = git(project, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
    let files: Vec<String> = conflicted.lines().map(str::to_string).filter(|l| !l.is_empty()).collect();
    let rebasing = ["rebase-merge", "rebase-apply"].iter().any(|dir| {
        git(project, &["rev-parse", "--git-path", dir]).is_ok_and(|path| project.root().join(path.trim()).exists())
    });
    if rebasing {
        git(project, &["rebase", "--abort"]).map_err(RebaseError::Git)?;
    }
    if !files.is_empty() {
        return Err(RebaseError::Conflict(files));
    }
    Err(match classify(&words) {
        PushError::Offline(words) => RebaseError::Offline(words),
        _ => RebaseError::Git(words),
    })
}

#[cfg(test)]
mod tests;
