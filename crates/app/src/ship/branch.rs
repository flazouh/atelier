//! The branch a commit lands on: the one checked out, unless it is the repository's default branch,
//! where the review asks for a new one first. Blocking: call it off the UI thread.

use lathe_project::Project;

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

/// Makes `name` and checks it out, keeping the working tree and the index as they are.
pub fn create(project: &dyn Project, name: &str) -> Result<(), String> {
    let words = |out: lathe_project::GitOutput| out.stderr.trim().to_string();
    let checked = project.git(&["check-ref-format", "--branch", name]).map_err(|e| e.to_string())?;
    if !checked.ok() {
        return Err(format!("{name} is not a branch name git takes"));
    }
    let switched = project.git(&["switch", "-c", name]).map_err(|e| e.to_string())?;
    if switched.ok() { Ok(()) } else { Err(words(switched)) }
}

#[cfg(test)]
mod tests;
