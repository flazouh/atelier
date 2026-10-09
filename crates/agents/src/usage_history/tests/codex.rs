use std::fs;

use super::support::config_dir;
use crate::usage_history::{read, Day, Provider, Roots, Tokens};

fn counts(ts: &str, total: (u64, u64, u64), last: (u64, u64, u64)) -> String {
    let raw = |t: (u64, u64, u64)| {
        format!(r#"{{"input_tokens":{},"cached_input_tokens":{},"cache_write_input_tokens":0,"output_tokens":{}}}"#, t.0, t.1, t.2)
    };
    format!(
        r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{},"last_token_usage":{}}}}}}}"#,
        raw(total),
        raw(last)
    )
}

#[test]
fn counts_the_delta_of_the_running_totals_once() {
    let tmp = tempfile::tempdir().unwrap();
    let home = config_dir(tmp.path(), ".codex");
    let day = home.join("sessions/2026/09/25");
    fs::create_dir_all(&day).unwrap();
    let lines = [
        r#"{"timestamp":"2026-09-25T10:00:00Z","type":"session_meta","payload":{"id":"cx1","cwd":"/work/proj-x"}}"#.to_owned(),
        r#"{"timestamp":"2026-09-25T10:00:00Z","type":"turn_context","payload":{"model":"gpt-5-codex","cwd":"/work/proj-x"}}"#.to_owned(),
        r##"{"timestamp":"2026-09-25T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions"}]}}"##.to_owned(),
        r#"{"timestamp":"2026-09-25T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Refactor parser"}]}}"#.to_owned(),
        counts("2026-09-25T10:00:02.000Z", (1000, 600, 50), (1000, 600, 50)),
        counts("2026-09-25T10:00:03.000Z", (1000, 600, 50), (1000, 600, 50)), // the same event again
        counts("2026-09-25T10:00:04.000Z", (1800, 1100, 120), (800, 500, 70)),
    ];
    fs::write(day.join("rollout-2026-09-25T10-00-00-cx1.jsonl"), lines.join("\n") + "\n").unwrap();
    let h = read(&Roots::new().with_dir(Provider::Codex, &home), Day::new(2026, 9, 1), 0);
    assert_eq!(h.skipped, 0);
    let a = &h.accounts[0];
    assert_eq!((a.provider, a.label.as_str()), (Provider::Codex, "codex"));
    let s = &a.sessions[0];
    assert_eq!((s.id.as_str(), s.title.as_str(), s.project.as_str(), s.model_main.as_str()), ("cx1", "Refactor parser", "proj-x", "gpt-5-codex"));
    // fresh input = input - cached: 1800 - 1100 = 700
    assert_eq!(s.totals, Tokens { input: 700, output: 120, cache_read: 1100, cache_write: 0 });
    assert!(s.est_cost_usd.unwrap() > 0.0);
}

#[test]
fn totals_that_go_down_fall_back_to_the_last_usage() {
    let tmp = tempfile::tempdir().unwrap();
    let home = config_dir(tmp.path(), ".codex");
    let day = home.join("sessions/2026/09/25");
    fs::create_dir_all(&day).unwrap();
    let lines = [
        counts("2026-09-25T10:00:02Z", (1000, 0, 100), (1000, 0, 100)),
        counts("2026-09-25T10:00:04Z", (300, 0, 30), (300, 0, 30)),
    ];
    fs::write(day.join("rollout-x.jsonl"), lines.join("\n") + "\n").unwrap();
    let h = read(&Roots::new().with_dir(Provider::Codex, &home), Day::new(2026, 9, 1), 0);
    let s = &h.accounts[0].sessions[0];
    assert_eq!((s.totals.input, s.totals.output), (1300, 130));
    assert_eq!(s.id, "rollout-x");
}
