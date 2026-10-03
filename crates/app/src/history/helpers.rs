use atelier_ui::DiffLine;

use super::types::{Commit, CommitFile, Shown};

/// How many commits the log reads: enough to scroll back a while, few enough to read at once.
pub const LOG_LIMIT: usize = 300;

/// Separates the fields of a commit, and the commits, in what [`log_args`] asks for.
const FIELD: char = '\u{1f}';
const RECORD: char = '\u{1e}';

/// The arguments of `git log` for the newest commits of `rev`, one record each.
pub fn log_args(rev: &str) -> Vec<String> {
    vec![
        "log".into(),
        format!("-n{LOG_LIMIT}"),
        "--format=%H%x1f%h%x1f%an%x1f%at%x1f%s%x1e".into(),
        rev.into(),
        "--".into(),
    ]
}

/// The commits in what [`log_args`] printed, newest first. A record that does not read is skipped.
pub fn parse_log(out: &str) -> Vec<Commit> {
    out.split(RECORD)
        .filter_map(|record| {
            let mut f = record.trim_start_matches('\n').split(FIELD);
            let (sha, short, author, at, subject) = (f.next()?, f.next()?, f.next()?, f.next()?, f.next()?);
            if sha.is_empty() {
                return None;
            }
            Some(Commit {
                sha: sha.to_string().into(),
                short: short.to_string().into(),
                author: author.to_string().into(),
                at: at.trim().parse().ok()?,
                subject: subject.to_string().into(),
            })
        })
        .collect()
}

/// The arguments of `git show` for `sha`: its message, then its patch. A merge shows what it brought
/// against its first parent, which reads as a plain diff.
pub fn show_args(sha: &str) -> Vec<String> {
    vec![
        "show".into(),
        "--format=%B%x1e".into(),
        "--patch".into(),
        "--no-color".into(),
        "--no-ext-diff".into(),
        "--diff-merges=first-parent".into(),
        sha.into(),
        "--".into(),
    ]
}

/// The commit in what [`show_args`] printed: the message, and each file's lines. A file's path is the new
/// one, or the old one for a file the commit deleted.
pub fn split_show(sha: &str, out: &str) -> Shown {
    let (message, patch) = out.split_once(RECORD).unwrap_or((out, ""));
    Shown { sha: sha.to_string().into(), message: message.trim().to_string().into(), files: split_patch(patch) }
}

/// Each file's lines in a patch, in its order: what `git diff` or `git show` prints after the message.
pub fn split_patch(patch: &str) -> Vec<CommitFile> {
    let mut files: Vec<CommitFile> = Vec::new();
    let mut chunk = String::new();
    let mut path: Option<String> = None;
    let mut flush = |path: &mut Option<String>, chunk: &mut String| {
        if let Some(p) = path.take() {
            files.push(CommitFile { path: p.into(), lines: DiffLine::parse(chunk) });
        }
        chunk.clear();
    };
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            flush(&mut path, &mut chunk);
            path = Some(header_path(rest));
        } else if let Some(new) = line.strip_prefix("+++ b/") {
            path = Some(new.to_string());
        }
        chunk.push_str(line);
        chunk.push('\n');
    }
    flush(&mut path, &mut chunk);
    files
}

/// The tree of a repository with no commit, to diff a first checkout against.
pub const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// The arguments of `git diff` for what the checkout holds that `base` does not, staged or not.
pub fn diff_args(base: &str) -> Vec<String> {
    ["diff", "--no-color", "--no-ext-diff", base, "--"].map(String::from).to_vec()
}

/// The arguments of `git ls-files` for the files git does not track and does not ignore.
pub const UNTRACKED_ARGS: [&str; 4] = ["ls-files", "--others", "--exclude-standard", "-z"];

/// The arguments of `git diff` for an untracked file, as a file that is all new. It exits 1 when it prints one.
pub fn new_file_args(path: &str) -> Vec<String> {
    ["diff", "--no-color", "--no-ext-diff", "--no-index", "--", "/dev/null", path].map(String::from).to_vec()
}

/// The path in `a/<path> b/<path>`, the rest of a `diff --git` line. Both halves name the same file
/// unless it moved, and a deleted file has no `+++ b/` line to say better.
fn header_path(rest: &str) -> String {
    let rest = rest.strip_prefix("a/").unwrap_or(rest);
    match rest.find(" b/") {
        Some(at) => rest[..at].to_string(),
        None => rest.to_string(),
    }
}
