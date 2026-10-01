//! What git says about the working tree, at the start of a turn and at its end. A shell command changes
//! files no tool call names; comparing the two states finds them.
use std::{
    collections::HashMap,
    io::{Read, Write},
};

use atelier_project::{Command, Project};

/// How many paths one `git hash-object` is given.
const HASH_BATCH: usize = 200;

/// One entry of `git status --porcelain=v2`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub untracked: bool,
    /// The file is gone from the working tree.
    pub deleted: bool,
    /// The path a rename came from.
    pub renamed_from: Option<String>,
}

/// The working tree as git sees it: which paths differ from the last commit, and what the files that
/// differ hold, as blob ids. Paths are relative to the project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct State {
    pub entries: HashMap<String, Entry>,
    pub blobs: HashMap<String, String>,
}

/// The state of `project`'s working tree, or `None` when it is not a git repository or git fails.
pub(crate) fn snapshot(project: &dyn Project) -> Option<State> {
    let prefix = project.git(&["rev-parse", "--show-prefix"]).ok().filter(|o| o.ok())?.stdout.trim().to_string();
    let status = project.git(&["status", "--porcelain=v2", "-z", "--untracked-files=all"]).ok().filter(|o| o.ok())?;
    let entries = parse_status(&status.stdout, &prefix);
    let alive: Vec<&str> = entries.iter().filter(|(_, e)| !e.deleted).map(|(path, _)| path.as_str()).collect();
    let blobs = hash_files(project, &alive);
    Some(State { entries, blobs })
}

/// How many paths one `git cat-file --batch` is given. The names go in before any answer is read, so
/// they must fit in the pipe.
const HEAD_BATCH: usize = 500;

/// The bytes of each path in the last commit, or `None` for a path that is not in it. One git process
/// for a batch of paths, since a turn can change hundreds of files. Paths are relative to the project.
pub(crate) fn head_files(project: &dyn Project, paths: &[&str]) -> HashMap<String, Option<Vec<u8>>> {
    let mut found = HashMap::with_capacity(paths.len());
    for batch in paths.chunks(HEAD_BATCH) {
        // A name with a line end cannot be asked for on a line of its own.
        let (asked, odd): (Vec<&str>, Vec<&str>) = batch.iter().copied().partition(|p| !p.contains('\n'));
        found.extend(odd.into_iter().map(|p| (p.to_string(), None)));
        let Some(answers) = cat_file(project, &asked) else {
            found.extend(asked.into_iter().map(|p| (p.to_string(), None)));
            continue;
        };
        found.extend(asked.into_iter().zip(answers).map(|(path, bytes)| (path.to_string(), bytes)));
    }
    found
}

/// `git cat-file --batch` for `HEAD:./path` of each path: one answer per path, in order. The answers
/// read `<id> blob <size>` and the bytes, or `<name> missing`.
fn cat_file(project: &dyn Project, paths: &[&str]) -> Option<Vec<Option<Vec<u8>>>> {
    let mut process = project.spawn(&Command::new("git").args(["cat-file", "--batch"])).ok()?;
    for path in paths {
        writeln!(process.stdin, "HEAD:./{path}").ok()?;
    }
    drop(process.stdin);
    let mut out = Vec::new();
    process.stdout.read_to_end(&mut out).ok()?;
    let _ = process.control.wait();
    let mut answers = Vec::with_capacity(paths.len());
    let mut at = 0;
    for _ in paths {
        let end = at + out[at..].iter().position(|b| *b == b'\n')?;
        let header = std::str::from_utf8(&out[at..end]).ok()?;
        at = end + 1;
        if header.ends_with(" missing") {
            answers.push(None);
            continue;
        }
        let mut parts = header.split(' ');
        let (_id, kind, size) = (parts.next()?, parts.next()?, parts.next()?.parse::<usize>().ok()?);
        let body = out.get(at..at + size)?;
        // A name that is a folder answers with a tree, which is not a file's text.
        answers.push((kind == "blob").then(|| body.to_vec()));
        at += size + 1;
    }
    Some(answers)
}

/// Reads `git status --porcelain=v2 -z`. Entries outside `prefix` (the project's folder inside the
/// repository) are left out, and the prefix is taken off the rest.
pub(crate) fn parse_status(output: &str, prefix: &str) -> HashMap<String, Entry> {
    let mut entries = HashMap::new();
    let mut fields = output.split('\0').filter(|f| !f.is_empty());
    let inside = |path: &str| path.strip_prefix(prefix).map(str::to_string);
    while let Some(field) = fields.next() {
        let mut parts = field.splitn(2, ' ');
        let (kind, rest) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        match kind {
            // `1 XY sub mH mI mW hH hI path`
            "1" => {
                let cols: Vec<&str> = rest.splitn(8, ' ').collect();
                if let (Some(xy), Some(path)) = (cols.first(), cols.get(7).and_then(|p| inside(p))) {
                    entries.insert(path, Entry { untracked: false, deleted: xy.ends_with('D') || xy.starts_with('D'), renamed_from: None });
                }
            }
            // `2 XY sub mH mI mW hH hI Xscore path`, then the path it came from as the next field.
            "2" => {
                let cols: Vec<&str> = rest.splitn(9, ' ').collect();
                let from = fields.next().and_then(inside);
                if let (Some(xy), Some(path)) = (cols.first(), cols.get(8).and_then(|p| inside(p))) {
                    entries.insert(path, Entry { untracked: false, deleted: xy.ends_with('D'), renamed_from: from });
                }
            }
            // `u XY sub m1 m2 m3 mW h1 h2 h3 path`
            "u" => {
                let cols: Vec<&str> = rest.splitn(10, ' ').collect();
                if let Some(path) = cols.get(9).and_then(|p| inside(p)) {
                    entries.insert(path, Entry { untracked: false, deleted: false, renamed_from: None });
                }
            }
            "?" => {
                if let Some(path) = inside(rest) {
                    entries.insert(path, Entry { untracked: true, deleted: false, renamed_from: None });
                }
            }
            _ => {}
        }
    }
    entries
}

/// The blob id of each file as it is in the working tree, paths relative to the project. A file git
/// cannot read has no entry.
fn hash_files(project: &dyn Project, paths: &[&str]) -> HashMap<String, String> {
    let mut blobs = HashMap::with_capacity(paths.len());
    for batch in paths.chunks(HASH_BATCH) {
        let mut args = vec!["hash-object", "--"];
        args.extend(batch.iter().copied());
        match project.git(&args) {
            Ok(out) if out.ok() && out.stdout.lines().count() == batch.len() => {
                blobs.extend(batch.iter().zip(out.stdout.lines()).map(|(path, id)| (path.to_string(), id.to_string())));
            }
            // One unreadable path fails the batch: ask for each on its own.
            _ => {
                for path in batch {
                    if let Ok(out) = project.git(&["hash-object", "--", path]).map(|o| o.stdout).map(|s| s.trim().to_string())
                        && !out.is_empty()
                    {
                        blobs.insert((*path).to_string(), out);
                    }
                }
            }
        }
    }
    blobs
}

#[cfg(test)]
mod tests;
