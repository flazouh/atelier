use crate::traits::Project;
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
    assert!(seen.iter().all(|c| c.path != "noise.log" && !c.path.ends_with(".atelier-save")), "ignored and temporary files are not: {seen:?}");
    fs::remove_file(p.root().join("a.txt")).unwrap();
    let removed = rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(removed.contains(&Change { path: "a.txt".into(), kind: ChangeKind::Removed }), "{removed:?}");
    drop(watch);
}

/// Writes `path` under the project, then waits for the watch to report `wanted`; every change seen on the way.
fn seen_until(p: &LocalProject, rx: &mpsc::Receiver<Vec<Change>>, writes: &[&str], wanted: &str) -> Vec<Change> {
    for path in writes {
        let at = p.root().join(path);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, "x").unwrap();
    }
    let mut seen = Vec::new();
    while let Ok(batch) = rx.recv_timeout(Duration::from_secs(3)) {
        seen.extend(batch);
        if seen.iter().any(|c: &Change| c.path == wanted) {
            break;
        }
    }
    seen
}

/// A folder of several checkouts: each one's own .gitignore keeps its build output and packages out, as the listing does.
#[test]
fn a_watch_keeps_out_what_a_nested_gitignore_ignores() {
    let (_dir, p) = project(&[("app/.gitignore", "target/\nnode_modules/\n"), ("app/src/main.rs", "a"), ("app/target/debug/a.o", "o"), ("app/node_modules/x/i.js", "j")]);
    let (tx, rx) = mpsc::channel();
    let watch = p.watch(Box::new(move |batch| tx.send(batch).unwrap())).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let seen = seen_until(&p, &rx, &["app/target/debug/b.o", "app/node_modules/x/j.js", "app/src/main.rs"], "app/src/main.rs");
    assert!(seen.iter().any(|c| c.path == "app/src/main.rs"), "the source is watched: {seen:?}");
    assert!(seen.iter().all(|c| !c.path.starts_with("app/target") && !c.path.starts_with("app/node_modules")), "the ignored folders are not: {seen:?}");
    drop(watch);
}

/// A folder made after the watch started is watched too, with what is made in it at once.
#[test]
fn a_watch_follows_a_folder_made_after_it_started() {
    let (_dir, p) = project(&[("a.txt", "a")]);
    let (tx, rx) = mpsc::channel();
    let watch = p.watch(Box::new(move |batch| tx.send(batch).unwrap())).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    fs::create_dir_all(p.root().join("new/deep")).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let seen = seen_until(&p, &rx, &["new/deep/b.txt"], "new/deep/b.txt");
    assert!(seen.iter().any(|c| c.path == "new/deep/b.txt"), "{seen:?}");
    drop(watch);
}

/// On Linux each watched folder costs the host one of its few inotify watches: only the folders the listing shows are watched.
#[test]
fn the_folders_watched_are_the_ones_listed() {
    let (_dir, p) = project(&[(".gitignore", "build/\n"), ("src/a.rs", "a"), ("build/x/y/z.o", "o"), ("app/.gitignore", "dist/\n"), ("app/dist/m.js", "m"), ("app/lib/b.rs", "b")]);
    let mut dirs: Vec<String> = super::helpers::watched_dirs(p.root()).iter().map(|d| d.strip_prefix(p.root()).unwrap().to_string_lossy().into_owned()).collect();
    dirs.sort();
    assert_eq!(dirs, ["", "app", "app/lib", "src"]);
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
    assert!(p.spawn(&Command::new("atelier-no-such-program")).is_err());
}

#[test]
fn a_process_that_fails_leaves_its_stderr_and_only_the_tail_of_a_long_one() {
    let (_dir, p) = project(&[]);
    let mut failing = p.spawn(&Command::new("sh").args(["-c", "echo 'no such model' >&2; exit 3"])).unwrap();
    assert_eq!(failing.control.wait().unwrap(), Some(3));
    assert_eq!(failing.control.stderr(), "no such model\n", "complete the moment wait returns");
    // A grandchild that keeps stderr open does not hold the wait up for long.
    let mut lingering = p.spawn(&Command::new("sh").args(["-c", "echo started >&2; sleep 5 >&2 & exit 0"])).unwrap();
    let at = std::time::Instant::now();
    assert_eq!(lingering.control.wait().unwrap(), Some(0));
    assert!(at.elapsed() < Duration::from_secs(2), "{:?}", at.elapsed());
    assert_eq!(lingering.control.stderr(), "started\n");
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

#[test]
fn a_removed_file_is_gone_and_a_folder_or_a_path_outside_is_refused() {
    let (dir, p) = project(&[("a.txt", "a"), ("sub/b.txt", "b")]);
    p.remove("a.txt").unwrap();
    assert!(!dir.path().join("a.txt").exists());
    assert_eq!(p.remove("a.txt").unwrap_err().kind(), io::ErrorKind::NotFound);
    assert!(p.remove("sub").is_err(), "a folder is not removed");
    assert!(dir.path().join("sub/b.txt").exists());
    assert_eq!(p.remove("../x").unwrap_err().kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn a_project_keeps_its_data_outside_the_repository_and_lists_it_newest_first() {
    let (dir, _) = project(&[("a.txt", "a")]);
    let data = tempfile::tempdir().unwrap();
    let p = LocalProject::open(dir.path()).unwrap().with_data_dir(data.path());
    p.data_write("agent/sessions/one.jsonl", b"1").unwrap();
    std::thread::sleep(Duration::from_millis(20));
    p.data_write("agent/sessions/two.jsonl", b"2").unwrap();
    p.data_write("review/x.json", b"x").unwrap();
    assert_eq!(p.data_read("agent/sessions/one.jsonl").unwrap(), b"1");
    let listed: Vec<String> = p.data_list("agent/sessions").unwrap().into_iter().map(|e| e.path).collect();
    assert_eq!(listed, ["agent/sessions/two.jsonl", "agent/sessions/one.jsonl"], "newest first, only under the prefix");
    assert!(p.data_list("nothing/here").unwrap().is_empty());
    assert!(!dir.path().join("agent").exists(), "nothing lands in the project");
    assert_eq!(p.data_write("../escape", b"x").unwrap_err().kind(), io::ErrorKind::InvalidInput);
    assert_eq!(p.data_read("a/../../x").unwrap_err().kind(), io::ErrorKind::InvalidInput);
    // The same root finds the same folder; another root another.
    let again = LocalProject::open(dir.path()).unwrap().with_data_dir(data.path());
    assert_eq!(again.data_read("review/x.json").unwrap(), b"x");
    let (other, _) = project(&[]);
    let other = LocalProject::open(other.path()).unwrap().with_data_dir(data.path());
    assert_eq!(other.data_read("review/x.json").unwrap_err().kind(), io::ErrorKind::NotFound);
}

#[test]
fn the_data_folders_path_is_where_data_write_puts_its_files() {
    let (dir, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let p = LocalProject::open(dir.path()).unwrap().with_data_dir(data.path());
    p.data_write("pr-view/x.txt", b"x").unwrap();
    let path = p.data_path().expect("a local project has a data folder");
    assert!(path.starts_with(data.path()), "{path:?}");
    assert_eq!(std::fs::read(path.join("pr-view/x.txt")).unwrap(), b"x");
    let other = tempfile::tempdir().unwrap();
    let q = LocalProject::open(other.path()).unwrap().with_data_dir(data.path());
    assert_ne!(q.data_path(), p.data_path(), "each project has its own");
}

#[test]
fn a_folder_lists_folders_first_by_name_and_a_link_to_a_folder_is_a_folder() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["beta", "Alpha", ".hidden"] {
        std::fs::create_dir(dir.path().join(name)).unwrap();
    }
    for name in ["zeta.txt", "a.txt"] {
        std::fs::write(dir.path().join(name), "x").unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path().join("beta"), dir.path().join("link-to-beta")).unwrap();
    let listed: Vec<_> = crate::read_local_dir(dir.path().to_str().unwrap()).unwrap().into_iter().map(|e| (e.name, e.dir)).collect();
    let mut want = vec![(".hidden", true), ("Alpha", true), ("beta", true)];
    #[cfg(unix)]
    want.push(("link-to-beta", true));
    want.extend([("a.txt", false), ("zeta.txt", false)]);
    let want: Vec<_> = want.into_iter().map(|(n, d)| (n.to_string(), d)).collect();
    assert_eq!(listed, want);
}

#[test]
fn a_tilde_is_the_home_folder_and_a_relative_path_is_refused() {
    let home = std::env::var("HOME").unwrap();
    assert_eq!(crate::expand_home("~").unwrap(), std::path::PathBuf::from(&home));
    assert_eq!(crate::expand_home("~/src").unwrap(), std::path::Path::new(&home).join("src"));
    assert_eq!(crate::expand_home("/etc").unwrap(), std::path::PathBuf::from("/etc"));
    assert!(crate::expand_home("~other/x").is_none(), "another user's home is not guessed");
    assert!(crate::expand_home("src/x").is_none());
    assert_eq!(crate::read_local_dir("src").unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn a_project_keeps_its_tasks_in_its_data_folder() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("atelier");
    fs::create_dir(&root).unwrap();
    let data = tempfile::tempdir().unwrap();
    let project = LocalProject::open(&root).unwrap().with_data_dir(data.path());
    let tracker = project.tracker().unwrap();
    let task = tracker.create(&atelier_tracker::NewTask::titled("Keep tasks"), "alex").unwrap();
    assert!(task.key.starts_with("ATE-"), "{}", task.key);
    assert!(project.data_path().unwrap().join(crate::TRACKER_FILE).exists());
    assert!(std::sync::Arc::ptr_eq(&tracker, &project.tracker().unwrap()), "each ask gets the same store");
    let again = LocalProject::open(&root).unwrap().with_data_dir(data.path());
    assert_eq!(again.tracker().unwrap().get(&task.id).unwrap(), Some(task));
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A repository with one commit on `main`, and a worktree of it on `fix` beside it.
fn repo_with_worktree() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    fs::create_dir(&main).unwrap();
    git(&main, &["init", "-q", "-b", "main"]);
    fs::write(main.join("a.txt"), "main").unwrap();
    git(&main, &["add", "."]);
    git(&main, &["commit", "-qm", "first"]);
    let tree = dir.path().join("fix");
    git(&main, &["worktree", "add", "-q", "-b", "fix", tree.to_str().unwrap()]);
    fs::write(tree.join("a.txt"), "fix").unwrap();
    (dir, main, tree)
}

#[test]
fn a_project_seen_from_its_worktree_works_there_and_keeps_its_data_and_tasks() {
    let (_dir, main, tree) = repo_with_worktree();
    let data = tempfile::tempdir().unwrap();
    let project = LocalProject::open(&main).unwrap().with_data_dir(data.path());
    let there = project.at(&tree).unwrap();
    assert_eq!(there.root(), fs::canonicalize(&tree).unwrap());
    assert_eq!(there.read("a.txt").unwrap(), b"fix");
    assert_eq!(project.read("a.txt").unwrap(), b"main", "the main checkout is left as it was");
    assert_eq!(there.git(&["branch", "--show-current"]).unwrap().stdout.trim(), "fix");
    let mut pwd = there.spawn(&Command::new("pwd")).unwrap();
    let mut out = String::new();
    pwd.stdout.read_to_string(&mut out).unwrap();
    assert_eq!(std::path::Path::new(out.trim()), fs::canonicalize(&tree).unwrap());
    assert_eq!(there.data_path(), project.data_path(), "one data folder for every worktree");
    let tasks = there.tracker().unwrap();
    assert!(std::sync::Arc::ptr_eq(&tasks, &project.tracker().unwrap()), "one tracker too");
    let task = tasks.create(&atelier_tracker::NewTask::titled("Ship"), "alex").unwrap();
    assert!(task.key.starts_with(&atelier_tracker::prefix_for("main")), "named for the project, not the worktree: {}", task.key);
    assert_eq!(project.at(&main).unwrap().root(), project.root(), "the main checkout is a worktree as well");
}

#[test]
fn a_folder_that_is_not_a_worktree_of_the_project_is_refused() {
    let (dir, main, _tree) = repo_with_worktree();
    let project = LocalProject::open(&main).unwrap();
    let plain = dir.path().join("plain");
    fs::create_dir(&plain).unwrap();
    assert_eq!(project.at(&plain).err().unwrap().kind(), io::ErrorKind::InvalidInput);
    let other = dir.path().join("other");
    fs::create_dir(&other).unwrap();
    git(&other, &["init", "-q"]);
    assert_eq!(project.at(&other).err().unwrap().kind(), io::ErrorKind::InvalidInput, "another repository");
    assert_eq!(project.at(&dir.path().join("gone")).err().unwrap().kind(), io::ErrorKind::NotFound);
}
