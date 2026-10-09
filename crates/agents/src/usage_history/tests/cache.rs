use std::fs::OpenOptions;
use std::io::Write;

use super::support::{assistant, claude_roots, claude_session, config_dir};
use crate::usage_history::{read_cached, Cache, Day};

const SINCE: Day = Day { year: 2026, month: 9, day: 1 };

#[test]
fn a_second_read_parses_only_changed_files() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_dir(tmp.path(), ".claude");
    let line = assistant("m1", "r1", "claude-sonnet-4-6", "2026-09-25T10:00:00Z", (1, 1, 0, 0));
    let a = claude_session(&config, "p", "a", std::slice::from_ref(&line));
    claude_session(&config, "p", "b", &[line]);
    let roots = claude_roots(&config);
    let mut cache = Cache::new();

    let first = read_cached(&roots, SINCE, 0, &mut cache);
    assert_eq!(cache.parsed_last_read(), 2);
    let second = read_cached(&roots, SINCE, 3600, &mut cache);
    assert_eq!(cache.parsed_last_read(), 0, "an unchanged file comes from the cache, whatever the offset");
    assert_eq!(first.accounts[0].sessions.len(), second.accounts[0].sessions.len());

    let more = assistant("m2", "r2", "claude-sonnet-4-6", "2026-09-25T11:00:00Z", (5, 0, 0, 0));
    OpenOptions::new().append(true).open(&a).unwrap().write_all(format!("{more}\n").as_bytes()).unwrap();
    let third = read_cached(&roots, SINCE, 0, &mut cache);
    assert_eq!(cache.parsed_last_read(), 1);
    let total: u64 = third.accounts[0].sessions.iter().map(|s| s.totals.input).sum();
    assert_eq!(total, 1 + 1 + 5);
    assert_eq!(cache.len(), 2);
}
