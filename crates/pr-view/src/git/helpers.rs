use atelier_forge::{Change, RepoRef};

use super::structs::{Commit, FileEntry};
use super::types::{Blob, GitError, GitResult, MAX_TEXT};

pub fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

/// A commit id: 4 to 64 hex digits. Anything else never reaches git as a revision.
pub fn is_sha(text: &str) -> bool {
    (4..=64).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_hexdigit())
}

pub(super) fn check_sha(text: &str) -> GitResult<&str> {
    if is_sha(text) { Ok(text) } else { Err(GitError::Invalid(text.chars().take(40).collect())) }
}

/// A branch name safe to put in a ref: no leading dash, no control characters, spaces, `..`, `\`, `~^:?*[`.
pub(super) fn check_branch(name: &str) -> GitResult<&str> {
    let bad = name.is_empty()
        || name.starts_with(['-', '/'])
        || name.ends_with(['/', '.'])
        || name.contains("..")
        || name.contains("//")
        || name.contains("@{")
        || name.chars().any(|c| c.is_control() || " \\~^:?*[".contains(c));
    if bad { Err(GitError::Invalid(name.chars().take(60).collect())) } else { Ok(name) }
}

/// The URL of the remote in a `git remote -v` listing that names `repo`.
pub fn remote_for(listing: &str, repo: &RepoRef) -> Option<String> {
    let mut found: Option<String> = None;
    for line in listing.lines() {
        let mut parts = line.split_whitespace();
        let (Some(_name), Some(url), Some(kind)) = (parts.next(), parts.next(), parts.next()) else { continue };
        if kind == "(fetch)" && RepoRef::from_remote(url).as_ref() == Some(repo) {
            // A remote called `origin` wins over the others that name the same repository.
            if line.starts_with("origin") || found.is_none() {
                found = Some(url.to_string());
            }
        }
    }
    found
}

/// `git log -z --format=%H%x1f%an%x1f%at%x1f%s` as commits.
pub fn parse_commits(output: &str) -> Vec<Commit> {
    output
        .split('\0')
        .filter_map(|record| {
            let record = record.trim_start_matches('\n');
            let mut fields = record.split('\u{1f}');
            let (sha, author, at, title) = (fields.next()?, fields.next()?, fields.next()?, fields.next()?);
            is_sha(sha).then(|| Commit { sha: sha.into(), title: title.into(), author: author.into(), at: at.parse().unwrap_or(0) })
        })
        .collect()
}

pub(super) fn change_of(letter: char) -> Change {
    match letter {
        'A' => Change::Added,
        'D' => Change::Deleted,
        'R' => Change::Renamed,
        'C' => Change::Copied,
        _ => Change::Modified,
    }
}

/// `git diff --raw -z` joined to `git diff --numstat -z`, both with `-M`, so they list the same files in
/// the same order.
pub fn parse_files(raw: &[u8], numstat: &[u8]) -> Vec<FileEntry> {
    let raw = String::from_utf8_lossy(raw);
    let mut fields = raw.split('\0').filter(|f| !f.is_empty());
    let mut entries: Vec<FileEntry> = Vec::new();
    while let Some(meta) = fields.next() {
        let Some(meta) = meta.strip_prefix(':') else { continue };
        let mut parts = meta.split_whitespace();
        let (Some(_old_mode), Some(new_mode), Some(old_sha), Some(new_sha), Some(status)) = (parts.next(), parts.next(), parts.next(), parts.next(), parts.next()) else { continue };
        let letter = status.chars().next().unwrap_or('M');
        let (old_path, path) = if matches!(letter, 'R' | 'C') {
            let (Some(old), Some(new)) = (fields.next(), fields.next()) else { break };
            (Some(old.to_string()), new.to_string())
        } else {
            let Some(path) = fields.next() else { break };
            (None, path.to_string())
        };
        let zero = |sha: &str| sha.bytes().all(|b| b == b'0');
        entries.push(FileEntry {
            path,
            old_path,
            change: change_of(letter),
            additions: 0,
            deletions: 0,
            // A submodule is a commit id, not text.
            binary: new_mode == "160000",
            old_blob: (!zero(old_sha)).then(|| old_sha.to_string()),
            new_blob: (!zero(new_sha)).then(|| new_sha.to_string()),
        });
    }
    let stat = String::from_utf8_lossy(numstat);
    let mut parts = stat.split('\0').filter(|f| !f.is_empty()).peekable();
    let mut at = 0;
    while let Some(record) = parts.next() {
        let mut cells = record.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(name)) = (cells.next(), cells.next(), cells.next()) else { continue };
        // A rename has an empty name here, and the two names follow.
        if name.is_empty() {
            parts.next();
            parts.next();
        }
        if let Some(entry) = entries.get_mut(at) {
            match (added.parse::<u32>(), deleted.parse::<u32>()) {
                (Ok(a), Ok(d)) => {
                    entry.additions = a;
                    entry.deletions = d;
                }
                _ => entry.binary = true,
            }
        }
        at += 1;
    }
    entries
}

/// `git cat-file --batch` output: `<name> blob <size>\n<bytes>\n` for a hit, `<name> missing\n` for a miss.
pub fn parse_batch(output: &[u8]) -> Vec<Blob> {
    let mut blobs = Vec::new();
    let mut at = 0;
    while at < output.len() {
        let Some(end) = output[at..].iter().position(|b| *b == b'\n') else { break };
        let header = String::from_utf8_lossy(&output[at..at + end]).into_owned();
        at += end + 1;
        if header.ends_with(" missing") || header.ends_with(" ambiguous") {
            blobs.push(Blob::Missing);
            continue;
        }
        // The name comes first and may hold spaces, so the kind and the size are read from the end.
        let mut fields = header.rsplitn(3, ' ');
        let (Some(size), Some(kind), Some(_name)) = (fields.next(), fields.next(), fields.next()) else { break };
        let Ok(size) = size.parse::<usize>() else { break };
        if at + size > output.len() {
            break;
        }
        let body = &output[at..at + size];
        at += size + 1;
        blobs.push(if kind != "blob" {
            Blob::Binary
        } else if size as u64 > MAX_TEXT {
            Blob::TooLarge(size as u64)
        } else if body.iter().take(8000).any(|b| *b == 0) {
            Blob::Binary
        } else {
            match String::from_utf8(body.to_vec()) {
                Ok(text) => Blob::Text(text),
                Err(_) => Blob::Binary,
            }
        });
    }
    blobs
}
