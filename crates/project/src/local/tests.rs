use std::{
    fs,
    io::{Read, Write},
    sync::mpsc,
    time::Duration,
};

use super::*;

fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, LocalProject) {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in files {
        let at = dir.path().join(path);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, text).unwrap();
    }
    let project = LocalProject::open(dir.path()).unwrap();
    (dir, project)
}

#[test]
fn the_list_keeps_dotfiles_and_leaves_out_git_and_ignored_paths() {
    let (_dir, p) = project(&[
        (".gitignore", "target/\n*.log\n"),
        ("src/main.rs", "fn main() {}"),
        ("target/debug/app", "binary"),
        ("run.log", "noise"),
        (".git/HEAD", "ref: refs/heads/main"),
        (".env.example", "KEY="),
    ]);
    let paths: Vec<(String, bool)> = p.list().unwrap().into_iter().map(|e| (e.path, e.dir)).collect();
    assert_eq!(
        paths,
        [(".env.example".into(), false), (".gitignore".into(), false), ("src".into(), true), ("src/main.rs".into(), false)]
    );
}

#[test]
fn a_write_replaces_the_file_whole_and_keeps_its_mode() {
    let (dir, p) = project(&[("run.sh", "echo old")]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir.path().join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    p.write("run.sh", b"echo new").unwrap();
    assert_eq!(p.read("run.sh").unwrap(), b"echo new");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(dir.path().join("run.sh")).unwrap().permissions().mode() & 0o777, 0o755);
    }
    let left: Vec<_> = fs::read_dir(dir.path()).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(left.len(), 1, "no temporary file is left behind: {left:?}");
}

#[test]
fn a_path_outside_the_project_is_refused() {
    let (_dir, p) = project(&[("a.txt", "a")]);
    for path in ["../secret", "src/../../x", "/etc/passwd"] {
        let refused = if path.starts_with('/') { p.read(path.trim_start_matches('/')).is_err() } else { p.read(path).is_err() };
        assert!(refused, "{path} is refused");
    }
    assert!(p.write("../escape.txt", b"x").is_err());
    assert_eq!(crate::host_path(p.root(), "./a.txt").unwrap(), p.root().join("a.txt"));
}

#[test]
fn search_finds_literal_and_regex_lines_skips_binary_and_stops_at_the_limit() {
    let (_dir, p) = project(&[
        ("a.rs", "let x = 1;\nfn detach() {}\nlet y = x.detach();\n"),
        ("b.bin", "detach\0\0\0"),
        ("c.md", "Detach the stream."),
    ]);
    let query = |pattern: &str, regex, case_sensitive, limit| Query { pattern: pattern.into(), regex, case_sensitive, limit };
    // Literal: the parentheses are text, not a group.
    let hits = p.search(&query("x.detach()", false, true, 100)).unwrap();
    assert_eq!(hits, [Match { path: "a.rs".into(), line: 2, text: "let y = x.detach();".into() }]);
    let hits = p.search(&query("detach", false, false, 100)).unwrap();
    assert_eq!(hits.iter().map(|m| (m.path.as_str(), m.line)).collect::<Vec<_>>(), [("a.rs", 1), ("a.rs", 2), ("c.md", 0)]);
    assert_eq!(p.search(&query(r"fn \w+\(", true, true, 100)).unwrap().len(), 1);
    assert_eq!(p.search(&query("detach", false, false, 2)).unwrap().len(), 2);
    assert!(p.search(&query("(", true, true, 1)).is_err(), "a broken regex is an error, not an empty answer");
}

#[test]
fn a_watch_reports_a_write_and_a_removal_in_batches() {
    let (_dir, p) = project(&[("a.txt", "a"), (".gitignore", "*.log\n")]);
    let (tx, rx) = mpsc::channel();
    let watch = p.watch(Box::new(move |batch| tx.send(batch).unwrap())).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    p.write("a.txt", b"b").unwrap();
    fs::write(p.root().join("noise.log"), "x").unwrap();
    let mut seen = Vec::new();
    while let Ok(batch) = rx.recv_timeout(Duration::from_secs(3)) {
        seen.extend(batch);
        if seen.iter().any(|c: &Change| c.path == "a.txt") {
            break;
        }
    }
    assert!(seen.iter().any(|c| c.path == "a.txt" && c.kind != ChangeKind::Removed), "the write is reported: {seen:?}");
    assert!(seen.iter().all(|c| c.path != "noise.log" && !c.path.ends_with(".lathe-save")), "ignored and temporary files are not: {seen:?}");
    fs::remove_file(p.root().join("a.txt")).unwrap();
    let removed = rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(removed.contains(&Change { path: "a.txt".into(), kind: ChangeKind::Removed }), "{removed:?}");
    drop(watch);
}

#[test]
fn a_spawned_process_talks_over_its_pipes() {
    let (_dir, p) = project(&[]);
    let mut cat = p.spawn(&Command::new("cat")).unwrap();
    cat.stdin.write_all(b"hello\n").unwrap();
    drop(cat.stdin);
    let mut out = String::new();
    cat.stdout.read_to_string(&mut out).unwrap();
    assert_eq!(out, "hello\n");
    assert_eq!(cat.control.wait().unwrap(), Some(0));
    let mut sleeper = p.spawn(&Command::new("sleep").args(["30"])).unwrap();
    assert!(sleeper.control.running());
    sleeper.control.kill().unwrap();
    sleeper.control.wait().unwrap();
    assert!(!sleeper.control.running());
    assert!(p.spawn(&Command::new("lathe-no-such-program")).is_err());
}

#[test]
fn a_process_that_fails_leaves_its_stderr_and_only_the_tail_of_a_long_one() {
    let (_dir, p) = project(&[]);
    let mut failing = p.spawn(&Command::new("sh").args(["-c", "echo 'no such model' >&2; exit 3"])).unwrap();
    assert_eq!(failing.control.wait().unwrap(), Some(3));
    // The reader thread may still hold the last chunk for a moment after the exit.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while failing.control.stderr().is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(failing.control.stderr(), "no such model\n");
    let tail = crate::Tail::default();
    tail.push(&vec![b'a'; crate::STDERR_KEEP]);
    tail.push(b"the end");
    let text = tail.text();
    assert_eq!(text.len(), crate::STDERR_KEEP);
    assert!(text.ends_with("the end"));
}

#[test]
fn git_runs_in_the_root_and_reports_a_folder_with_no_repository() {
    let (_dir, p) = project(&[("a.txt", "a")]);
    let status = p.git(&["status", "--porcelain"]).unwrap();
    assert!(!status.ok(), "no repository yet");
    assert!(p.git(&["init", "-q"]).unwrap().ok());
    let status = p.git(&["status", "--porcelain"]).unwrap();
    assert!(status.ok());
    assert_eq!(status.stdout, "?? a.txt\n");
}

#[test]
fn a_file_is_not_a_project() {
    let (dir, _p) = project(&[("a.txt", "a")]);
    assert!(LocalProject::open(dir.path().join("a.txt")).is_err());
    assert!(LocalProject::open(dir.path().join("missing")).is_err());
}
