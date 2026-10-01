//! The branch a commit lands on: the one checked out, unless it is the repository's default branch,
//! where the review asks for a new one first. Blocking: call it off the UI thread.

use atelier_project::Project;

/// The branch checked out, or `None` when HEAD is detached.
pub fn current(project: &dyn Project) -> Option<String> {
    let out = project.git(&["symbolic-ref", "--short", "-q", "HEAD"]).ok().filter(|o| o.ok())?;
    Some(out.stdout.trim().to_string()).filter(|b| !b.is_empty())
}

/// The remote's default branch (`origin/HEAD`), when the clone knows it.
pub fn default_branch(project: &dyn Project) -> Option<String> {
    let out = project.git(&["symbolic-ref", "--short", "-q", "refs/remotes/origin/HEAD"]).ok().filter(|o| o.ok())?;
    out.stdout.trim().strip_prefix("origin/").map(str::to_string)
}

/// Whether `branch` is the default branch: the remote's when it is known, else `main` or `master`.
pub fn is_default(project: &dyn Project, branch: &str) -> bool {
    match default_branch(project) {
        Some(default) => default == branch,
        None => matches!(branch, "main" | "master"),
    }
}

/// `name`, or `name-2`, `name-3` and on when a branch has it already.
pub fn free(project: &dyn Project, name: &str) -> String {
    let taken = |candidate: &str| {
        let full = format!("refs/heads/{candidate}");
        project.git(&["rev-parse", "--verify", "-q", &full]).is_ok_and(|o| o.ok())
    };
    if !taken(name) {
        return name.to_string();
    }
    (2..).map(|n| format!("{name}-{n}")).find(|candidate| !taken(candidate)).expect("some number is free")
}
/// Runs `commit` on a new branch `name` at HEAD, and leaves the reader on it. If `commit` fails, HEAD
/// goes back to the branch it was on and the new branch goes, so a failed commit moves nothing. HEAD
/// moves by `symbolic-ref` alone: the new branch starts at the same commit, so the index and the files
/// stay as they are, and the commit itself runs as it would anywhere, with its hooks and its signing.
pub fn commit_on_new<T>(project: &dyn Project, name: &str, commit: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let back = |to: &str| {
        let full = format!("refs/heads/{to}");
        match project.git(&["symbolic-ref", "HEAD", &full]) {
            Ok(out) if out.ok() => Ok(()),
            Ok(out) => Err(out.stderr.trim().to_string()),
            Err(error) => Err(error.to_string()),
        }
    };
    commit_on_new_with(project, name, commit, back)
}

/// As `commit_on_new`, with `back` moving HEAD back to a branch: the seam where the tests fail it.
pub(crate) fn commit_on_new_with<T>(
    project: &dyn Project,
    name: &str,
    commit: impl FnOnce() -> Result<T, String>,
    back: impl FnOnce(&str) -> Result<(), String>,
) -> Result<T, String> {
    let git = |args: &[&str]| project.git(args).map_err(|e| e.to_string());
    let words = |out: atelier_project::GitOutput| out.stderr.trim().to_string();
    if !git(&["check-ref-format", "--branch", name])?.ok() {
        return Err(format!("{name} is not a branch name git takes"));
    }
    let full = format!("refs/heads/{name}");
    if git(&["rev-parse", "--verify", "-q", &full])?.ok() {
        return Err(format!("A branch named {name} is there already"));
    }
    let was = current(project).ok_or_else(|| "HEAD is not on a branch: check one out to commit".to_string())?;
    // A repository with no commit has nothing for the branch to start at: HEAD names it, unborn.
    let born = git(&["rev-parse", "--verify", "-q", "HEAD"])?.ok();
    if born {
        let made = git(&["branch", name])?;
        if !made.ok() {
            return Err(words(made));
        }
    }
    let moved = git(&["symbolic-ref", "HEAD", &full])?;
    if !moved.ok() {
        return Err(words(moved));
    }
    commit().map_err(|why| {
        // Each step back that fails is named, with where it leaves the reader.
        if let Err(stuck) = back(&was) {
            return format!("{why}, and HEAD could not move back to {was}: {stuck}. You are on {name}");
        }
        if born {
            match git(&["branch", "-D", name]) {
                Ok(out) if out.ok() => {}
                Ok(out) => return format!("{why}, and the branch {name} could not go: {}", out.stderr.trim()),
                Err(error) => return format!("{why}, and the branch {name} could not go: {error}"),
            }
        }
        why
    })
}
#[cfg(test)]
mod tests;
