use std::fs::{self, DirEntry};
use std::io::ErrorKind;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::usage_history::structs::{AccountRoot, Candidate, Roots};
use crate::usage_history::types::Provider;

/// The log files of every account that changed at or after `min_mtime_secs` (epoch seconds). Folders and files
/// that cannot be read add to `skipped`; a folder that does not exist adds nothing.
pub(crate) fn discover(roots: &Roots, min_mtime_secs: i64, skipped: &mut usize) -> Vec<Candidate> {
    let mut out = Vec::new();
    for (account, root) in roots.accounts().iter().enumerate() {
        match root.provider {
            Provider::Claude => claude(account, root, min_mtime_secs, &mut out, skipped),
            Provider::Codex => codex(account, root, min_mtime_secs, &mut out, skipped),
        }
    }
    out
}

fn list_dir(path: &Path, skipped: &mut usize) -> Vec<DirEntry> {
    match fs::read_dir(path) {
        Ok(read) => read
            .filter_map(|entry| match entry {
                Ok(entry) => Some(entry),
                Err(_) => {
                    *skipped += 1;
                    None
                }
            })
            .collect(),
        Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
        Err(_) => {
            *skipped += 1;
            Vec::new()
        }
    }
}

fn is_jsonl(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "jsonl")
}

/// Adds the file when it changed recently enough.
fn push_file(
    entry: &DirEntry,
    account: usize,
    provider: Provider,
    session_hint: Option<String>,
    min_mtime_secs: i64,
    out: &mut Vec<Candidate>,
    skipped: &mut usize,
) {
    let Ok(meta) = entry.metadata() else {
        *skipped += 1;
        return;
    };
    let Ok(mtime) = meta.modified() else {
        *skipped += 1;
        return;
    };
    let secs = mtime.duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    if secs < min_mtime_secs {
        return;
    }
    out.push(Candidate {
        account,
        provider,
        path: entry.path(),
        mtime,
        size: meta.len(),
        subagent: session_hint.is_some(),
        session_hint,
    });
}

fn claude(account: usize, root: &AccountRoot, min: i64, out: &mut Vec<Candidate>, skipped: &mut usize) {
    for project in list_dir(&root.dir.join("projects"), skipped) {
        for entry in list_dir(&project.path(), skipped) {
            let Ok(kind) = entry.file_type() else {
                *skipped += 1;
                continue;
            };
            if kind.is_file() && is_jsonl(&entry.path()) {
                push_file(&entry, account, Provider::Claude, None, min, out, skipped);
            } else if kind.is_dir() {
                let session = entry.file_name().to_string_lossy().into_owned();
                for sub in list_dir(&entry.path().join("subagents"), skipped) {
                    if sub.file_type().is_ok_and(|k| k.is_file()) && is_jsonl(&sub.path()) {
                        push_file(&sub, account, Provider::Claude, Some(session.clone()), min, out, skipped);
                    }
                }
            }
        }
    }
}

fn codex(account: usize, root: &AccountRoot, min: i64, out: &mut Vec<Candidate>, skipped: &mut usize) {
    let mut pending = vec![root.dir.join("sessions")];
    while let Some(dir) = pending.pop() {
        for entry in list_dir(&dir, skipped) {
            let Ok(kind) = entry.file_type() else {
                *skipped += 1;
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() && name.starts_with("rollout-") && is_jsonl(&entry.path()) {
                push_file(&entry, account, Provider::Codex, None, min, out, skipped);
            }
        }
    }
}
