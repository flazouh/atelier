//! The review path's numbers, against the targets in `docs/performance.md`. Run in release, on the HP:
//!   cargo test -p lathe-review --release --test perf -- --ignored --nocapture --test-threads=1
use std::{
    fs,
    path::Path,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

use beui::inline_review::Decision;
use lathe_agents::session::{Event, ToolCall, ToolId, ToolKind, ToolStatus};
use lathe_project::LocalProject;
use lathe_review::{FileReview, Merged, TurnTracker};

const RUNS: usize = 15;

fn measure(mut run: impl FnMut()) -> (Duration, Duration) {
    let mut times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            run();
            start.elapsed()
        })
        .collect();
    times.sort();
    (times[RUNS / 2], times[(RUNS * 95).div_ceil(100) - 1])
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.
}

/// A file of `lines` rows of code-like text.
fn source(lines: usize, changed_every: usize, mark: &str) -> String {
    (0..lines)
        .map(|i| {
            if changed_every > 0 && i % changed_every == 7 {
                format!("    let value_{i} = compute({i}, \"{mark}\");\n")
            } else {
                format!("    let value_{i} = compute({i}, \"base\");\n")
            }
        })
        .collect()
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
#[ignore = "a measurement, run in release"]
fn a_single_edit_re_diffs_a_20000_line_file_in_under_5_milliseconds() {
    let before = source(20_000, 0, "");
    let mut after = before.clone();
    after = after.replacen("value_10000 = compute(10000, \"base\")", "value_10000 = compute(10000, \"edited\")", 1);
    let (median, p95) = measure(|| {
        let merged = Merged::diff(&before, &after);
        assert_eq!(merged.hunks().len(), 1);
    });
    println!("one edit in a 20,000-line file, diffed: median {:.3} ms, p95 {:.3} ms", ms(median), ms(p95));
    assert!(median < Duration::from_millis(5));

    // The same, as the review does it when the agent writes the file again with the review open.
    let reviewing = Merged::diff(&before, &after);
    let again = after.replacen("value_15000 = compute(15000, \"base\")", "value_15000 = compute(15000, \"again\")", 1);
    let (median, p95) = measure(|| {
        assert_eq!(reviewing.rebased_on(&again).hunks().len(), 2);
    });
    println!("the agent edits again, rebased: median {:.3} ms, p95 {:.3} ms", ms(median), ms(p95));
    assert!(median < Duration::from_millis(5));

    // A decision, and the user typing one character in the merged buffer.
    let id = reviewing.hunks()[0].id.clone();
    let (median, p95) = measure(|| {
        assert!(reviewing.decide(&id, Decision::Accept).is_some());
    });
    println!("one decision on the merged text: median {:.3} ms, p95 {:.3} ms", ms(median), ms(p95));
    let typed = reviewing.text().replacen("value_5 =", "value_5x =", 1);
    let (median, p95) = measure(|| {
        assert_eq!(reviewing.edited(&typed).hunks().len(), 1);
    });
    println!("one keystroke moving the hunks: median {:.3} ms, p95 {:.3} ms", ms(median), ms(p95));
}

#[test]
#[ignore = "a measurement, run in release"]
fn a_20000_line_file_changed_all_over_diffs_in_under_50_milliseconds() {
    let before = source(20_000, 0, "");
    let after = source(20_000, 40, "changed");
    let (median, p95) = measure(|| {
        let merged = Merged::diff(&before, &after);
        assert_eq!(merged.hunks().len(), 500);
    });
    println!("a 20,000-line file with 500 hunks: median {:.2} ms, p95 {:.2} ms", ms(median), ms(p95));
    assert!(median < Duration::from_millis(50));
}

#[test]
#[ignore = "a measurement, run in release"]
fn a_turn_of_200_files_one_of_them_20000_lines_has_all_its_hunks_in_under_100_milliseconds() {
    // The diff and the merge of 200 files, from texts in memory.
    let mut pairs: Vec<(String, String)> = (0..199).map(|i| (source(150 + i, 0, ""), source(150 + i, 20, "changed"))).collect();
    pairs.push((source(20_000, 0, ""), source(20_000, 100, "changed")));
    let (median, p95) = measure(|| {
        let files: Vec<FileReview> = pairs
            .iter()
            .enumerate()
            .map(|(i, (before, after))| FileReview::from_texts(format!("src/file_{i}.rs"), Some(before.clone()), Some(after.clone()), true))
            .collect();
        assert_eq!(files.len(), 200);
    });
    println!("200 files (one of 20,000 lines), hunks from texts in memory: median {:.1} ms, p95 {:.1} ms", ms(median), ms(p95));
    assert!(median < Duration::from_millis(100));

    // The whole finish, as the app runs it: git status, hashes, reads and hunks, on real files.
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "t@example.com"]);
    git(dir.path(), &["config", "user.name", "T"]);
    for (i, (before, _)) in pairs.iter().enumerate() {
        let path = dir.path().join(format!("src/file_{i}.rs"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, before).unwrap();
    }
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "first"]);
    let project = Arc::new(LocalProject::open(dir.path()).unwrap());
    let mut times = Vec::new();
    for _ in 0..7 {
        let mut tracker = TurnTracker::begin(project.as_ref());
        // Half the files are named by a tool call; the rest are found by git.
        for (i, (_, after)) in pairs.iter().enumerate() {
            let path = dir.path().join(format!("src/file_{i}.rs"));
            if i % 2 == 0 {
                let call = ToolCall {
                    id: ToolId::new(format!("t{i}")),
                    name: "Edit".into(),
                    kind: ToolKind::Edit,
                    input: serde_json::Value::Null,
                    file: Some(path.to_string_lossy().to_string()),
                    parent: None,
                    status: ToolStatus::Running,
                };
                tracker.observe(project.as_ref(), &Event::ToolStarted(call));
            }
            fs::write(path, after).unwrap();
        }
        let start = Instant::now();
        let turn = tracker.finish(project.as_ref());
        times.push(start.elapsed());
        assert_eq!(turn.files().len(), 200);
        // Back to the first commit for the next run.
        git(dir.path(), &["checkout", "-q", "--", "."]);
    }
    times.sort();
    println!("the same 200 files through TurnTracker::finish on disk: median {:.1} ms, max {:.1} ms", ms(times[3]), ms(times[6]));
}
