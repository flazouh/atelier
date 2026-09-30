//! The commit of what a review kept (`kept.rs`). It is built on a temporary index, so the reader's own
//! staged work is neither taken nor lost: the index starts from `HEAD` (empty in a repository with no
//! commit yet), takes each kept file's text as a blob, and `git commit -F -` commits it, so the
//! repository's pre-commit and commit-msg hooks run and `commit.gpgsign` holds. Only then does the
//! reader's index take the committed blobs, for those paths alone. A hook that refuses stops everything
//! and changes nothing. Blocking: call it off the UI thread.

use std::io::{Read, Write};

use lathe_project::{Command, Project};

use crate::ship::kept::Kept;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Committed {
    pub sha: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitError {
    /// The review kept nothing to commit.
    Nothing,
    /// A hook, or git itself, refused the commit: its words.
    Refused(String),
    /// Git could not be run, or a step before the commit failed.
    Git(String),
}

impl std::fmt::Display for CommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nothing => f.write_str("The review kept nothing to commit"),
            Self::Refused(words) => write!(f, "The commit was refused: {words}"),
            Self::Git(words) => write!(f, "Git failed: {words}"),
        }
    }
}

/// Runs `git args` in the project's root, on `index` when it is given, with `stdin`; its stdout, or its
/// own words when it fails.
fn git(project: &dyn Project, args: &[&str], index: Option<&str>, stdin: Option<&[u8]>) -> Result<String, String> {
    let mut command = Command::new("git").args(args.iter().copied());
    if let Some(index) = index {
        command.env.push(("GIT_INDEX_FILE".into(), index.into()));
    }
    let mut process = project.spawn(&command).map_err(|e| e.to_string())?;
    if let Some(bytes) = stdin {
        process.stdin.write_all(bytes).map_err(|e| e.to_string())?;
    }
    drop(process.stdin);
    let mut out = String::new();
    process.stdout.read_to_string(&mut out).map_err(|e| e.to_string())?;
    match process.control.wait().map_err(|e| e.to_string())? {
        Some(0) => Ok(out),
        _ => {
            let words = format!("{}\n{}", out.trim(), process.control.stderr().trim());
            Err(words.trim().to_string())
        }
    }
}

/// Commits `kept` with `message` on the branch checked out.
pub fn commit(project: &dyn Project, kept: &[Kept], message: &str) -> Result<Committed, CommitError> {
    if kept.is_empty() {
        return Err(CommitError::Nothing);
    }
    let step = |r: Result<String, String>| r.map_err(CommitError::Git);
    let index_path = step(git(project, &["rev-parse", "--git-path", "lathe-commit-index"], None, None))?;
    let index_path = match std::path::Path::new(index_path.trim()) {
        path if path.is_absolute() => path.to_path_buf(),
        path => project.root().join(path),
    };
    let index = index_path.display().to_string();
    let has_head = git(project, &["rev-parse", "--verify", "-q", "HEAD"], None, None).is_ok();
    // The index file is on the project's host, so it is made and removed there too.
    let forget = || drop(project.spawn(&Command::new("rm").args(["-f", index.as_str()])).and_then(|mut p| p.control.wait()));
    let result = (|| {
        let base = if has_head { "HEAD" } else { "--empty" };
        step(git(project, &["read-tree", base], Some(&index), None))?;
        // Each kept file as a blob, with its mode in HEAD (a new file is a plain file).
        let mut entries = Vec::new();
        for file in kept {
            match &file.text {
                Some(text) => {
                    let blob = step(git(project, &["hash-object", "-w", "--stdin"], None, Some(text.as_bytes())))?.trim().to_string();
                    let mode = has_head
                        .then(|| git(project, &["ls-tree", "HEAD", "--", &file.path], None, None).ok())
                        .flatten()
                        .and_then(|line| line.split_whitespace().next().map(str::to_string))
                        .unwrap_or_else(|| "100644".into());
                    entries.push((file.path.clone(), Some((mode, blob))));
                }
                None => entries.push((file.path.clone(), None)),
            }
        }
        let place = |index: Option<&str>| -> Result<(), CommitError> {
            for (path, entry) in &entries {
                match entry {
                    Some((mode, blob)) => step(git(project, &["update-index", "--add", "--cacheinfo", &format!("{mode},{blob},{path}")], index, None))?,
                    None => step(git(project, &["update-index", "--force-remove", "--", path], index, None))?,
                };
            }
            Ok(())
        };
        place(Some(&index))?;
        git(project, &["commit", "-q", "-F", "-"], Some(&index), Some(message.as_bytes())).map_err(CommitError::Refused)?;
        // The reader's index takes what was committed, for those paths alone.
        place(None)?;
        let sha = step(git(project, &["rev-parse", "HEAD"], None, None))?.trim().to_string();
        Ok(Committed { sha })
    })();
    forget();
    result
}

#[cfg(test)]
mod tests;
