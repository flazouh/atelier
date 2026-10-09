use std::fmt::Write as _;
use std::time::Instant;

use super::support::{claude_roots, config_dir};
use crate::usage_history::{read_cached, Cache, Day};

/// 10,000 sessions of 20 messages over 30 days. Run with
/// `cargo test -p atelier-agents --release usage_history::tests::perf -- --ignored --nocapture`.
#[test]
#[ignore = "slow: writes about 100 MB"]
fn ten_thousand_sessions_read_fast() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_dir(tmp.path(), ".claude");
    let filler = "x".repeat(400);
    for i in 0..10_000u32 {
        let dir = config.join("projects").join(format!("proj-{}", i % 20));
        std::fs::create_dir_all(&dir).unwrap();
        let day = 1 + i % 30;
        let mut text = String::new();
        for j in 0..20u32 {
            write!(
                text,
                r#"{{"type":"assistant","timestamp":"2026-09-{day:02}T{:02}:{:02}:00.000Z","cwd":"/w/p","requestId":"r{i}-{j}","message":{{"id":"m{i}-{j}","model":"claude-sonnet-4-6","usage":{{"input_tokens":10,"output_tokens":20,"cache_creation_input_tokens":30,"cache_read_input_tokens":40}},"content":[{{"type":"text","text":"{filler}"}}]}}}}"#,
                j % 24,
                j * 2
            )
            .unwrap();
            text.push('\n');
        }
        std::fs::write(dir.join(format!("s{i}.jsonl")), text).unwrap();
    }
    let roots = claude_roots(&config);
    let mut cache = Cache::new();
    let cold = Instant::now();
    let h = read_cached(&roots, Day::new(2026, 9, 1), 0, &mut cache);
    let cold = cold.elapsed();
    let warm = Instant::now();
    let again = read_cached(&roots, Day::new(2026, 9, 1), 0, &mut cache);
    let warm = warm.elapsed();
    eprintln!("cold {cold:?}, warm {warm:?}, sessions {}", h.accounts[0].sessions.len());
    assert_eq!(h.accounts[0].sessions.len(), 10_000);
    assert_eq!(again.accounts[0].sessions.len(), 10_000);
    assert_eq!(cache.parsed_last_read(), 0);
    assert!(cold.as_secs_f64() < 10.0, "cold read took {cold:?}");
    assert!(warm.as_secs_f64() < 1.0, "warm read took {warm:?}");
}

/// Reads the logs of the machine it runs on and says how long that took. Run it by hand:
/// `cargo test --release -p atelier-agents real_logs -- --ignored --nocapture`.
#[test]
#[ignore = "reads this machine's real logs"]
fn real_logs_are_read_in_a_few_seconds_and_again_in_a_few_ms() {
    use crate::usage_history::{Cache, Day, Roots, read_cached};
    let roots = Roots::from_env();
    let today = Day::from_epoch_secs(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64, 0);
    let mut cache = Cache::default();
    let cold = std::time::Instant::now();
    let first = read_cached(&roots, today.minus_days(30), 0, &mut cache);
    let cold = cold.elapsed();
    let warm = std::time::Instant::now();
    let second = read_cached(&roots, today.minus_days(30), 0, &mut cache);
    let warm = warm.elapsed();
    let sessions: usize = first.accounts.iter().map(|a| a.sessions.len()).sum();
    println!("accounts {} sessions {sessions} skipped {} cold {cold:?} warm {warm:?}", first.accounts.len(), first.skipped);
    assert_eq!(first.accounts.len(), second.accounts.len());
}
