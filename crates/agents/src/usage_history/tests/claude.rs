use super::support::{assistant, claude_roots, claude_session, config_dir, user};
use crate::usage_history::{read, Day, Provider, Roots, Tokens};

const SINCE: Day = Day { year: 2026, month: 9, day: 1 };

#[test]
fn one_session_by_day_model_and_totals() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_dir(tmp.path(), ".claude-a");
    claude_session(
        &config,
        "p",
        "s1",
        &[
            user("Fix the login bug please"),
            assistant("m1", "r1", "claude-sonnet-4-6", "2026-09-25T10:00:00.000Z", (10, 20, 30, 40)),
            assistant("m2", "r2", "claude-opus-5-5", "2026-09-26T10:00:00.000Z", (1, 2, 3, 4)),
        ],
    );
    let h = read(&claude_roots(&config), SINCE, 0);
    assert_eq!(h.skipped, 0);
    assert_eq!(h.accounts.len(), 1);
    let a = &h.accounts[0];
    assert_eq!((a.provider, a.label.as_str()), (Provider::Claude, "claude-a"));
    let s = &a.sessions[0];
    assert_eq!((s.id.as_str(), s.title.as_str(), s.project.as_str()), ("s1", "Fix the login bug please", "projA"));
    assert_eq!(s.model_main, "claude-sonnet-4-6");
    assert_eq!(s.totals, Tokens { input: 11, output: 22, cache_write: 33, cache_read: 44 });
    assert_eq!(s.days.len(), 2);
    assert_eq!(s.days[0].day, Day::new(2026, 9, 25));
    assert_eq!(s.days[1].model, "claude-opus-5-5");
}

#[test]
fn custom_title_beats_first_prompt_and_titles_are_cut() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_dir(tmp.path(), ".claude");
    claude_session(
        &config,
        "p",
        "s1",
        &[
            user("first words"),
            r#"{"type":"custom-title","customTitle":"Old name","sessionId":"s1"}"#.to_owned(),
            r#"{"type":"custom-title","customTitle":"New name","sessionId":"s1"}"#.to_owned(),
            assistant("m1", "r1", "claude-sonnet-4-6", "2026-09-25T10:00:00Z", (1, 1, 0, 0)),
        ],
    );
    let long = "word ".repeat(40);
    claude_session(&config, "p", "s2", &[user(&long), assistant("m1", "r1", "claude-sonnet-4-6", "2026-09-25T10:00:00Z", (1, 1, 0, 0))]);
    let h = read(&claude_roots(&config), SINCE, 0);
    let title = |id: &str| h.accounts[0].sessions.iter().find(|s| s.id == id).unwrap().title.clone();
    assert_eq!(title("s1"), "New name");
    assert!(title("s2").chars().count() <= 60 && title("s2").ends_with('…'));
}

#[test]
fn repeated_message_lines_count_once_and_the_last_wins() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_dir(tmp.path(), ".claude");
    claude_session(
        &config,
        "p",
        "s1",
        &[
            assistant("m1", "r1", "claude-sonnet-4-6", "2026-09-25T10:00:00Z", (5, 5, 0, 0)),
            assistant("m1", "r1", "claude-sonnet-4-6", "2026-09-25T10:00:01Z", (5, 50, 0, 0)),
            assistant("m1", "r2", "claude-sonnet-4-6", "2026-09-25T10:00:02Z", (7, 7, 0, 0)),
        ],
    );
    let t = read(&claude_roots(&config), SINCE, 0).accounts[0].sessions[0].totals;
    assert_eq!((t.input, t.output), (12, 57));
}

#[test]
fn lines_without_both_ids_all_count() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_dir(tmp.path(), ".claude");
    let no_request = assistant("m1", "r1", "claude-sonnet-4-6", "2026-09-25T10:00:00Z", (3, 0, 0, 0)).replace(r#""requestId":"r1","#, "");
    claude_session(&config, "p", "s1", &[no_request.clone(), no_request]);
    let t = read(&claude_roots(&config), SINCE, 0).accounts[0].sessions[0].totals;
    assert_eq!(t.input, 6);
}

#[test]
fn the_offset_decides_the_day() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_dir(tmp.path(), ".claude");
    claude_session(&config, "p", "s1", &[assistant("m1", "r1", "claude-sonnet-4-6", "2026-09-25T23:30:00Z", (1, 1, 0, 0))]);
    let day = |offset| read(&claude_roots(&config), SINCE, offset).accounts[0].sessions[0].days[0].day;
    assert_eq!(day(0), Day::new(2026, 9, 25));
    assert_eq!(day(2 * 3600), Day::new(2026, 9, 26));
    assert_eq!(day(-5 * 3600), Day::new(2026, 9, 25));
}

#[test]
fn days_before_since_are_dropped() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_dir(tmp.path(), ".claude");
    claude_session(
        &config,
        "p",
        "s1",
        &[
            assistant("m1", "r1", "claude-sonnet-4-6", "2026-09-20T10:00:00Z", (100, 0, 0, 0)),
            assistant("m2", "r2", "claude-sonnet-4-6", "2026-09-25T10:00:00Z", (1, 0, 0, 0)),
        ],
    );
    let h = read(&claude_roots(&config), Day::new(2026, 9, 25), 0);
    assert_eq!(h.accounts[0].sessions[0].totals.input, 1);
    let none = read(&claude_roots(&config), Day::new(2026, 9, 26), 0);
    assert!(none.accounts[0].sessions.is_empty());
}

#[test]
fn corrupt_lines_are_skipped_and_counted() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_dir(tmp.path(), ".claude");
    claude_session(
        &config,
        "p",
        "s1",
        &[
            assistant("m1", "r1", "claude-sonnet-4-6", "2026-09-25T10:00:00Z", (1, 1, 0, 0)),
            "not json at all".to_owned(),
            r#"{"type":"assistant","usage":}"#.to_owned(),
            r#"{"type":"queue-operation"}"#.to_owned(),
        ],
    );
    let h = read(&claude_roots(&config), SINCE, 0);
    assert_eq!(h.skipped, 2);
    assert_eq!(h.accounts[0].sessions[0].totals.input, 1);
}

#[test]
fn a_half_written_last_line_is_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_dir(tmp.path(), ".claude");
    let path = claude_session(&config, "p", "s1", &[assistant("m1", "r1", "claude-sonnet-4-6", "2026-09-25T10:00:00Z", (1, 1, 0, 0))]);
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(r#"{"type":"assistant","timestamp":"2026-09-25T10:0"#);
    std::fs::write(&path, text).unwrap();
    let h = read(&claude_roots(&config), SINCE, 0);
    assert_eq!(h.skipped, 1);
    assert_eq!(h.accounts[0].sessions[0].totals.input, 1);
}

#[test]
fn empty_and_missing_folders_do_not_fail() {
    let tmp = tempfile::tempdir().unwrap();
    let empty = config_dir(tmp.path(), ".claude");
    let roots = claude_roots(&empty).with_dir(Provider::Claude, tmp.path().join(".claude-gone"));
    let h = read(&roots, SINCE, 0);
    assert_eq!(h.accounts.len(), 2);
    assert!(h.accounts.iter().all(|a| a.sessions.is_empty()));
    assert_eq!(h.skipped, 0);
    assert_eq!(read(&Roots::new(), SINCE, 0).accounts.len(), 0);
}

#[test]
fn old_files_are_not_opened() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_dir(tmp.path(), ".claude");
    let path = claude_session(&config, "p", "s1", &[assistant("m1", "r1", "claude-sonnet-4-6", "2020-01-01T10:00:00Z", (1, 1, 0, 0))]);
    let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_577_836_800);
    std::fs::File::options().write(true).open(&path).unwrap().set_modified(old).unwrap();
    let h = read(&claude_roots(&config), Day::new(2026, 9, 1), 0);
    assert!(h.accounts[0].sessions.is_empty());
}

#[test]
fn three_accounts_stay_apart() {
    let tmp = tempfile::tempdir().unwrap();
    let mut roots = Roots::new();
    for (i, name) in [".claude", ".claude-work", ".claude-lab"].iter().enumerate() {
        let config = config_dir(tmp.path(), name);
        claude_session(&config, "p", &format!("s{i}"), &[assistant("m", "r", "claude-sonnet-4-6", "2026-09-25T10:00:00Z", (i as u64 + 1, 0, 0, 0))]);
        roots = roots.with_dir(Provider::Claude, config);
    }
    let h = read(&roots, SINCE, 0);
    let labels: Vec<_> = h.accounts.iter().map(|a| a.label.as_str()).collect();
    assert_eq!(labels, ["claude", "claude-work", "claude-lab"]);
    let inputs: Vec<_> = h.accounts.iter().map(|a| a.sessions[0].totals.input).collect();
    assert_eq!(inputs, [1, 2, 3]);
}

#[test]
fn a_subagent_file_counts_for_its_parent_session() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_dir(tmp.path(), ".claude");
    claude_session(&config, "p", "s1", &[user("Parent task"), assistant("m1", "r1", "claude-sonnet-4-6", "2026-09-25T10:00:00Z", (1, 0, 0, 0))]);
    let sub = config.join("projects/p/s1/subagents");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("agent-a.jsonl"), assistant("m9", "r9", "claude-haiku-4-5", "2026-09-25T10:05:00Z", (4, 0, 0, 0)) + "\n").unwrap();
    let h = read(&claude_roots(&config), SINCE, 0);
    assert_eq!(h.accounts[0].sessions.len(), 1);
    let s = &h.accounts[0].sessions[0];
    assert_eq!((s.title.as_str(), s.totals.input), ("Parent task", 5));
}

#[test]
fn the_cost_is_an_estimate_and_unknown_models_have_none() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_dir(tmp.path(), ".claude");
    claude_session(
        &config,
        "p",
        "known",
        &[assistant("m1", "r1", "claude-sonnet-4-6", "2026-09-25T10:00:00Z", (1_000_000, 100_000, 0, 1_000_000))],
    );
    claude_session(&config, "p", "odd", &[assistant("m1", "r1", "mystery-1", "2026-09-25T10:00:00Z", (10, 10, 0, 0))]);
    let h = read(&claude_roots(&config), SINCE, 0);
    let cost = |id: &str| h.accounts[0].sessions.iter().find(|s| s.id == id).unwrap().est_cost_usd;
    // 1M input at $3, 100k output at $15 per M, 1M cache read at $0.30: 3 + 1.5 + 0.3
    assert!((cost("known").unwrap() - 4.8).abs() < 1e-9);
    assert_eq!(cost("odd"), None);
}
