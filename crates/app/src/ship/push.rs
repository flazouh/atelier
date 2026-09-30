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

/// The ssh command git is to run, told not to prompt: the reader's own, from `GIT_SSH_COMMAND` or
/// else `core.sshCommand`, as git picks them, or plain ssh when they set none.
pub fn batch_ssh(env: Option<&str>, config: Option<&str>) -> String {
    let own = [env, config].into_iter().flatten().map(str::trim).find(|c| !c.is_empty());
    format!("{} -o BatchMode=yes", own.unwrap_or("ssh"))
}

/// `git args` for a call that reaches the remote, with no login prompt, in the project's root. The ssh command is read where git runs,
/// which for a remote project is the host.
fn remote_git(project: &dyn Project, args: &[&str]) -> Result<String, String> {
    let env = run(project, Command::new("sh").args(["-c", "printf %s \"${GIT_SSH_COMMAND-}\""]), None).ok();
    let config = plain(project, &["config", "core.sshCommand"]).ok();
    let mut command = Command::new("git").args(args.iter().copied());
    command.env.push(("GIT_TERMINAL_PROMPT".into(), "0".into()));
    command.env.push(("GIT_SSH_COMMAND".into(), batch_ssh(env.as_deref(), config.as_deref())));
    run(project, command, None)
}

/// `git args` as they are, for what needs no remote.
fn plain(project: &dyn Project, args: &[&str]) -> Result<String, String> {
    run(project, Command::new("git").args(args.iter().copied()), None)
}

fn has_origin(project: &dyn Project) -> bool {
    plain(project, &["remote", "get-url", "origin"]).is_ok()
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
    remote_git(project, &["push", "-u", "origin", branch]).map(drop).map_err(|words| classify(&words))
}

/// The stash entry's message when the reader sets their edits aside for a rebase.
pub const ENTRY_NAME: &str = "lathe: edits set aside to pull and rebase";

/// What a pull and rebase did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rebased {
    /// Each of the branch's own commits, by its id before the rebase and after it.
    pub moved: Vec<(String, String)>,
    /// What became of the edits set aside, when the reader set them aside.
    pub edits: Option<PutBack>,
}

/// The reader's edits after the rebase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PutBack {
    /// They are back in the working tree, and the stash entry is gone.
    Back,
    /// They clash with the new commits in `files`: the files show the clash, and the edits stay in
    /// the stash `entry` (such as `stash@{0}`).
    Kept { entry: String, files: Vec<String> },
}

impl std::fmt::Display for PutBack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Back => write!(f, "Your edits are back"),
            Self::Kept { entry, files } => write!(
                f,
                "Your edits clash with the new commits in {}. The files show the clash, and your edits stay in {entry} (\"{ENTRY_NAME}\")",
                files.join(", ")
            ),
        }
    }
}

/// Pulls `origin/<branch>` and puts the branch's own commits on top. The reader's other edits stop
/// it before it starts; a conflict undoes the rebase.
pub fn pull_rebase(project: &dyn Project, branch: &str) -> Result<Rebased, RebaseError> {
    if !has_origin(project) {
        return Err(RebaseError::Git("no remote named origin".into()));
    }
    // The reader's other edits are theirs: this rebase does not put them aside.
    if has_edits(project)? {
        return Err(RebaseError::OtherEdits);
    }
    let moved = rebase(project, branch)?;
    Ok(Rebased { moved, edits: None })
}

/// As `pull_rebase`, on the reader's word to set their edits aside: they go to a stash entry named
/// `ENTRY_NAME` and come back after the rebase. A rebase that stops puts them back as they were.
pub fn pull_rebase_setting_aside(project: &dyn Project, branch: &str) -> Result<Rebased, RebaseError> {
    if !has_origin(project) {
        return Err(RebaseError::Git("no remote named origin".into()));
    }
    if !has_edits(project)? {
        return pull_rebase(project, branch);
    }
    plain(project, &["stash", "push", "-m", ENTRY_NAME]).map_err(RebaseError::Git)?;
    let moved = match rebase(project, branch) {
        Ok(moved) => moved,
        Err(error) => {
            // The branch is as it was, so the edits go back as they were.
            plain(project, &["stash", "pop", "--index"]).map_err(RebaseError::Git)?;
            return Err(error);
        }
    };
    let edits = match plain(project, &["stash", "pop"]) {
        Ok(_) => PutBack::Back,
        Err(_) => {
            let files = unmerged(project);
            let list = plain(project, &["stash", "list", "--format=%gd %s"]).unwrap_or_default();
            let entry = list.lines().find(|l| l.contains(ENTRY_NAME)).and_then(|l| l.split(' ').next()).unwrap_or("the stash").to_string();
            PutBack::Kept { entry, files }
        }
    };
    Ok(Rebased { moved, edits: Some(edits) })
}

/// Whether the working tree holds edits to tracked files that no commit has.
fn has_edits(project: &dyn Project) -> Result<bool, RebaseError> {
    let edits = plain(project, &["status", "--porcelain", "--untracked-files=no"]).map_err(RebaseError::Git)?;
    Ok(!edits.trim().is_empty())
}

/// The files git left with a clash to settle.
fn unmerged(project: &dyn Project) -> Vec<String> {
    let listed = plain(project, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
    listed.lines().map(str::to_string).filter(|l| !l.is_empty()).collect()
}

/// The branch's own commits over `origin/<branch>`, oldest first; none when origin has no such branch.
fn own_commits(project: &dyn Project, branch: &str) -> Vec<String> {
    let range = format!("origin/{branch}..HEAD");
    plain(project, &["rev-list", "--reverse", "--no-merges", &range]).unwrap_or_default().lines().map(str::to_string).collect()
}

/// Each commit's patch id, which a rebase keeps while the commit's id changes: (patch id, commit).
fn patch_ids(project: &dyn Project, commits: &[String]) -> Vec<(String, String)> {
    if commits.is_empty() {
        return Vec::new();
    }
    let mut args = vec!["show", "--format=commit %H", "-p"];
    args.extend(commits.iter().map(String::as_str));
    let Ok(shown) = plain(project, &args) else { return Vec::new() };
    let ids = run(project, Command::new("git").args(["patch-id", "--stable"]), Some(shown.into_bytes())).unwrap_or_default();
    ids.lines().filter_map(|l| l.split_once(' ')).map(|(patch, commit)| (patch.to_string(), commit.to_string())).collect()
}

/// The pull with a rebase itself, and the old id of each own commit matched to its new id.
fn rebase(project: &dyn Project, branch: &str) -> Result<Vec<(String, String)>, RebaseError> {
    // Fetch first, so the commits that are the branch's own are known before the rebase moves them.
    let tracking = format!("+refs/heads/{branch}:refs/remotes/origin/{branch}");
    remote_git(project, &["fetch", "origin", &tracking]).map_err(|words| match classify(&words) {
        PushError::Offline(words) => RebaseError::Offline(words),
        _ => RebaseError::Git(words),
    })?;
    let before = patch_ids(project, &own_commits(project, branch));
    let onto = format!("origin/{branch}");
    let Err(words) = plain(project, &["-c", "rebase.autoStash=false", "rebase", "--no-autostash", &onto]) else {
        let after = patch_ids(project, &own_commits(project, branch));
        return Ok(before
            .into_iter()
            .filter_map(|(patch, old)| after.iter().find(|(p, _)| *p == patch).map(|(_, new)| (old, new.clone())))
            .filter(|(old, new)| old != new)
            .collect());
    };
    let files = unmerged(project);
    let rebasing = ["rebase-merge", "rebase-apply"].iter().any(|dir| {
        plain(project, &["rev-parse", "--git-path", dir]).is_ok_and(|path| project.root().join(path.trim()).exists())
    });
    if rebasing {
        plain(project, &["rebase", "--abort"]).map_err(RebaseError::Git)?;
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
