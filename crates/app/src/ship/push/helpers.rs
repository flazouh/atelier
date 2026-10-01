use atelier_project::{Command, Project};

use crate::ship::commit::run;
use super::structs::Rebased;
use super::types::{ENTRY_NAME, PushError, PutBack, RebaseError};

/// The line of git's words that says what happened: the first `fatal:` or `error:` line, else the
/// first line.
pub(super) fn first(words: &str) -> &str {
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
pub(crate) fn remote_git(project: &dyn Project, args: &[&str]) -> Result<String, String> {
    #[cfg(test)]
    local_only(project);
    let env = run(project, Command::new("sh").args(["-c", "printf %s \"${GIT_SSH_COMMAND-}\""]), None).ok();
    let config = plain(project, &["config", "core.sshCommand"]).ok();
    let mut command = Command::new("git").args(args.iter().copied());
    command.env.push(("GIT_TERMINAL_PROMPT".into(), "0".into()));
    command.env.push(("GIT_SSH_COMMAND".into(), batch_ssh(env.as_deref(), config.as_deref())));
    run(project, command, None)
}

/// In tests, stops a call that would reach a remote other than a folder on this machine or a
/// `.invalid` host, which never resolves. The address is the one git uses, after any insteadOf rule.
#[cfg(test)]
pub(super) fn local_only(project: &dyn Project) {
    let Ok(url) = plain(project, &["remote", "get-url", "--push", "origin"]) else { return };
    let url = url.trim();
    let local = url.starts_with('/') || url.starts_with("file://");
    let host = url.split_once("://").map_or(url, |(_, rest)| rest).split(['/', ':']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or(host);
    assert!(local || host.ends_with(".invalid"), "a test reached for a network remote: {url}");
}

/// `git args` as they are, for what needs no remote.
pub(super) fn plain(project: &dyn Project, args: &[&str]) -> Result<String, String> {
    run(project, Command::new("git").args(args.iter().copied()), None)
}

pub(super) fn has_origin(project: &dyn Project) -> bool {
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
    pull_rebase_setting_aside_with(project, branch, &|| {})
}

/// As `pull_rebase_setting_aside`, with `during` run between setting the edits aside and the rebase:
/// the seam where the tests make another entry or break the rebase.
pub(crate) fn pull_rebase_setting_aside_with(project: &dyn Project, branch: &str, during: &dyn Fn()) -> Result<Rebased, RebaseError> {
    if !has_origin(project) {
        return Err(RebaseError::Git("no remote named origin".into()));
    }
    if !has_edits(project)? {
        return pull_rebase(project, branch);
    }
    plain(project, &["stash", "push", "-m", ENTRY_NAME]).map_err(RebaseError::Git)?;
    // The entry is known by its id: the reader, or another tool, may push an entry meanwhile.
    let made = plain(project, &["rev-parse", "stash@{0}"]).map_err(RebaseError::Git)?.trim().to_string();
    during();
    let moved = match rebase(project, branch) {
        Ok(moved) => moved,
        Err(error) => {
            // The branch is as it was, so the edits go back as they were.
            let entry = entry_of(project, &made);
            let back = entry.as_deref().map(|entry| plain(project, &["stash", "pop", "--index", entry]));
            return Err(match (back, entry) {
                (Some(Ok(_)), _) => error,
                (_, entry) => RebaseError::EditsKept { why: error.to_string(), entry: entry.unwrap_or_else(|| ENTRY_NAME.into()) },
            });
        }
    };
    let Some(entry) = entry_of(project, &made) else {
        return Err(RebaseError::EditsKept { why: "The rebase worked, but the entry with your edits is gone from the stash list".into(), entry: made });
    };
    let edits = match plain(project, &["stash", "pop", &entry]) {
        Ok(_) => PutBack::Back,
        Err(_) => PutBack::Kept { entry: entry_of(project, &made).unwrap_or(entry), files: unmerged(project) },
    };
    Ok(Rebased { moved, edits: Some(edits) })
}

/// The stash ref (`stash@{n}`) of the entry whose commit is `made`, wherever it sits in the list now.
pub(super) fn entry_of(project: &dyn Project, made: &str) -> Option<String> {
    let list = plain(project, &["stash", "list", "--format=%gd %H"]).ok()?;
    list.lines().find_map(|line| line.split_once(' ').filter(|(_, sha)| *sha == made).map(|(entry, _)| entry.to_string()))
}

/// Whether the working tree holds edits to tracked files that no commit has.
pub(super) fn has_edits(project: &dyn Project) -> Result<bool, RebaseError> {
    let edits = plain(project, &["status", "--porcelain", "--untracked-files=no"]).map_err(RebaseError::Git)?;
    Ok(!edits.trim().is_empty())
}

/// The files git left with a clash to settle.
pub(super) fn unmerged(project: &dyn Project) -> Vec<String> {
    let listed = plain(project, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
    listed.lines().map(str::to_string).filter(|l| !l.is_empty()).collect()
}

/// The branch's own commits over `origin/<branch>`, oldest first; none when origin has no such branch.
pub(super) fn own_commits(project: &dyn Project, branch: &str) -> Vec<String> {
    let range = format!("origin/{branch}..HEAD");
    plain(project, &["rev-list", "--reverse", "--no-merges", &range]).unwrap_or_default().lines().map(str::to_string).collect()
}

/// Each commit's patch id, which a rebase keeps while the commit's id changes: (patch id, commit).
pub(super) fn patch_ids(project: &dyn Project, commits: &[String]) -> Vec<(String, String)> {
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
pub(super) fn rebase(project: &dyn Project, branch: &str) -> Result<Vec<(String, String)>, RebaseError> {
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
