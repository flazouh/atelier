use std::collections::HashMap;
use std::sync::Arc;

use super::assemble::assemble;
use super::discover::discover;
use super::parallel::parallel_map;
use super::read_claude::read_claude;
use super::read_codex::read_codex;
use crate::usage_history::consts::MTIME_SLACK_SECS;
use crate::usage_history::structs::{Cache, CacheEntry, Candidate, Day, FileUsage, Roots, UsageHistory};
use crate::usage_history::types::Provider;

/// Reads the logs of `roots` for the days from `since` on. `offset_secs` is the local zone's offset east of UTC.
/// It never panics: what cannot be read is counted in [`UsageHistory::skipped`]. Use [`read_cached`] to keep
/// parsed files between reads.
pub fn read(roots: &Roots, since: Day, offset_secs: i32) -> UsageHistory {
    read_cached(roots, since, offset_secs, &mut Cache::new())
}

/// [`read`] with a [`Cache`]: a file whose path, mtime and size did not change is not parsed again.
pub fn read_cached(roots: &Roots, since: Day, offset_secs: i32, cache: &mut Cache) -> UsageHistory {
    let mut skipped = 0usize;
    let min_mtime = since.start_secs(offset_secs) - MTIME_SLACK_SECS;
    let candidates = discover(roots, min_mtime, &mut skipped);

    let stale: Vec<&Candidate> = candidates
        .iter()
        .filter(|c| !cache.entries.get(&c.path).is_some_and(|e| e.mtime == c.mtime && e.size == c.size))
        .collect();
    let parsed = parallel_map(&stale, |c| parse(c));
    cache.parsed_last_read = stale.len();
    let mut fresh: HashMap<&std::path::Path, Arc<FileUsage>> = HashMap::new();
    for (candidate, result) in stale.iter().zip(parsed) {
        match result {
            Ok(usage) => {
                fresh.insert(candidate.path.as_path(), Arc::new(usage));
            }
            Err(_) => skipped += 1,
        }
    }

    let mut entries = HashMap::new();
    let mut files = Vec::with_capacity(candidates.len());
    for c in &candidates {
        let usage = match fresh.get(c.path.as_path()) {
            Some(usage) => Arc::clone(usage),
            None => match cache.entries.get(&c.path) {
                Some(entry) if entry.mtime == c.mtime && entry.size == c.size => Arc::clone(&entry.usage),
                _ => continue, // it failed to parse and was counted
            },
        };
        skipped += usage.skipped;
        entries.insert(c.path.clone(), CacheEntry { mtime: c.mtime, size: c.size, usage: Arc::clone(&usage) });
        files.push((c.account, usage));
    }
    cache.entries = entries;

    UsageHistory { accounts: assemble(roots, &files, since, offset_secs), skipped }
}

fn parse(c: &Candidate) -> std::io::Result<FileUsage> {
    match c.provider {
        Provider::Claude => {
            let id = c.session_hint.clone().unwrap_or_else(|| stem(c));
            read_claude(&c.path, id, c.subagent)
        }
        Provider::Codex => read_codex(&c.path, stem(c)),
    }
}

fn stem(c: &Candidate) -> String {
    c.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
