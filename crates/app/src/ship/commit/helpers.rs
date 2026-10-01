use std::io::{Read, Write};

use atelier_project::{Command, Project};

use crate::ship::kept::Kept;
use super::structs::Committed;
use super::types::CommitError;

/// Runs `git args` in the project's root, on `index` when it is given, with `stdin` written on a thread
/// of its own while stdout is read, so neither pipe can stall the other; stdout, or git's own words.
pub(crate) fn git(project: &dyn Project, args: &[&str], index: Option<&str>, stdin: Option<Vec<u8>>) -> Result<String, String> {
    let mut command = Command::new("git").args(args.iter().copied());
    if let Some(index) = index {
        command.env.push(("GIT_INDEX_FILE".into(), index.into()));
    }
    run(project, command, stdin)
}

/// Runs `command` in the project's root, as `git` does.
pub(crate) fn run(project: &dyn Project, command: Command, stdin: Option<Vec<u8>>) -> Result<String, String> {
    let process = project.spawn(&command).map_err(|e| e.to_string())?;
    let (mut input, mut output, mut control) = (process.stdin, process.stdout, process.control);
    let feeding = std::thread::spawn(move || {
        let written = stdin.map_or(Ok(()), |bytes| input.write_all(&bytes));
        drop(input);
        written
    });
    let mut out = String::new();
    let read = output.read_to_string(&mut out);
    let fed = feeding.join().unwrap_or(Ok(()));
    let code = control.wait().map_err(|e| e.to_string())?;
    if code == Some(0) && read.is_ok() && fed.is_ok() {
        return Ok(out);
    }
    let words = format!("{}\n{}", out.trim(), control.stderr().trim());
    Err(words.trim().to_string())
}

/// The sha a revision names, or `None` when it names nothing (`HEAD` in a new repository).
pub(super) fn sha(project: &dyn Project, revision: &str) -> Option<String> {
    git(project, &["rev-parse", "-q", "--verify", revision], None, None).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// One `git fast-import` stream that writes every text as a blob and answers each one's sha.
pub(super) fn blobs(project: &dyn Project, texts: &[&str]) -> Result<Vec<String>, String> {
    let mut stream = Vec::new();
    for (i, text) in texts.iter().enumerate() {
        stream.extend_from_slice(format!("blob\nmark :{}\ndata {}\n", i + 1, text.len()).as_bytes());
        stream.extend_from_slice(text.as_bytes());
        stream.extend_from_slice(format!("\nget-mark :{}\n", i + 1).as_bytes());
    }
    let out = git(project, &["fast-import", "--quiet"], None, Some(stream))?;
    let shas: Vec<String> = out.lines().map(str::to_string).collect();
    if shas.len() != texts.len() {
        return Err(format!("git fast-import answered {} blobs of {}", shas.len(), texts.len()));
    }
    Ok(shas)
}

/// A step before the commit that failed, in git's words.
pub(super) fn step<T>(result: Result<T, String>) -> Result<T, CommitError> {
    result.map_err(CommitError::Git)
}

/// Commits `kept` with `message` on the branch checked out.
pub fn commit(project: &dyn Project, kept: &[Kept], message: &str) -> Result<Committed, CommitError> {
    commit_with(project, kept, message, &|| {})
}

/// [`commit`], with `before_commit` run just before HEAD is read again: a test's seam for a branch that
/// moves meanwhile.
pub(crate) fn commit_with(project: &dyn Project, kept: &[Kept], message: &str, before_commit: &dyn Fn()) -> Result<Committed, CommitError> {
    if kept.is_empty() {
        return Err(CommitError::Nothing);
    }
    let index_path = step(git(project, &["rev-parse", "--git-path", "atelier-commit-index"], None, None))?;
    let index_path = match std::path::Path::new(index_path.trim()) {
        path if path.is_absolute() => path.to_path_buf(),
        path => project.root().join(path),
    };
    let index = index_path.display().to_string();
    let head = sha(project, "HEAD");
    // The index file is on the project's host, so it is removed there too.
    let forget = || drop(project.spawn(&Command::new("rm").args(["-f", index.as_str()])).and_then(|mut p| p.control.wait()));
    let result = (|| {
        step(git(project, &["read-tree", head.as_deref().unwrap_or("--empty")], Some(&index), None))?;
        // Each file's mode in HEAD; a new file is a plain one.
        let mut modes = std::collections::HashMap::new();
        if let Some(head) = &head {
            let mut args = vec!["ls-tree", "-z", head.as_str(), "--"];
            args.extend(kept.iter().map(|k| k.path.as_str()));
            for entry in step(git(project, &args, None, None))?.split('\0').filter(|e| !e.is_empty()) {
                if let (Some(mode), Some((_, path))) = (entry.split(' ').next(), entry.split_once('\t')) {
                    modes.insert(path.to_string(), mode.to_string());
                }
            }
        }
        let texts: Vec<&str> = kept.iter().filter_map(|k| k.text.as_deref()).collect();
        let mut shas = step(blobs(project, &texts))?.into_iter();
        let mut info = String::new();
        for file in kept {
            match &file.text {
                Some(_) => {
                    let mode = modes.get(&file.path).map_or("100644", String::as_str);
                    info.push_str(&format!("{mode} {}\t{}\0", shas.next().unwrap_or_default(), file.path));
                }
                // A removal: mode 0 and a zero sha.
                None => info.push_str(&format!("0 {}\t{}\0", "0".repeat(40), file.path)),
            }
        }
        let place = |index: Option<&str>| step(git(project, &["update-index", "-z", "--index-info"], index, Some(info.clone().into_bytes())));
        place(Some(index.as_str()))?;
        before_commit();
        if sha(project, "HEAD") != head {
            return Err(CommitError::Moved);
        }
        git(project, &["commit", "-q", "-F", "-"], Some(&index), Some(message.as_bytes().to_vec())).map_err(CommitError::Refused)?;
        let made = sha(project, "HEAD").unwrap_or_default();
        if sha(project, "HEAD^") != head {
            return Err(CommitError::Raced(made));
        }
        // The reader's index takes what was committed, for those paths alone.
        place(None)?;
        Ok(Committed { sha: made })
    })();
    forget();
    result
}
