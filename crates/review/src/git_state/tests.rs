//! `git status --porcelain=v2 -z` as git prints it, and the snapshot on a real repository.
use std::{fs, path::Path, process::Command, sync::Arc};

use lathe_project::LocalProject;

use super::{head_files, parse_status, snapshot};

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

fn repo() -> (tempfile::TempDir, Arc<LocalProject>) {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "t@example.com"]);
    git(dir.path(), &["config", "user.name", "T"]);
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    fs::create_dir_all(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub/b.txt"), "b\n").unwrap();
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "first"]);
    let project = Arc::new(LocalProject::open(dir.path()).unwrap());
    (dir, project)
}

#[test]
fn every_kind_of_entry_is_read() {
    let out = [
        "1 .M N... 100644 100644 100644 aaaa bbbb changed.rs",
        "1 .D N... 100644 100644 000000 aaaa bbbb gone.rs",
        "1 D. N... 100644 000000 000000 aaaa 0000 staged gone.rs",
        "2 R. N... 100644 100644 100644 aaaa bbbb R100 moved to.rs",
        "moved from.rs",
        "u UU N... 100644 100644 100644 100644 aaaa bbbb cccc conflicted.rs",
        "? new file.rs",
        "! ignored.rs",
    ]
    .join("\0")
        + "\0";
    let entries = parse_status(&out, "");
    let get = |p: &str| entries.get(p).unwrap_or_else(|| panic!("no {p}: {entries:?}"));
    assert!(!get("changed.rs").untracked && !get("changed.rs").deleted);
    assert!(get("gone.rs").deleted);
    assert!(get("staged gone.rs").deleted);
    assert_eq!(get("moved to.rs").renamed_from.as_deref(), Some("moved from.rs"));
    assert!(!entries.contains_key("moved from.rs"), "the old name is not its own entry");
    assert!(get("new file.rs").untracked);
    assert!(entries.contains_key("conflicted.rs"));
    assert!(!entries.contains_key("ignored.rs"));
    assert_eq!(entries.len(), 6);
}

#[test]
fn a_path_with_spaces_or_unusual_characters_is_kept_whole() {
    let out = "? dir with space/ünï cödé.rs\0";
    assert!(parse_status(out, "").contains_key("dir with space/ünï cödé.rs"));
}

#[test]
fn only_entries_inside_the_projects_folder_stay_and_lose_the_folders_name() {
    let out = "1 .M N... 100644 100644 100644 a b web/a.rs\x001 .M N... 100644 100644 100644 a b api/b.rs\0? web/new.rs\0";
    let entries = parse_status(out, "web/");
    let mut paths: Vec<_> = entries.keys().cloned().collect();
    paths.sort();
    assert_eq!(paths, ["a.rs", "new.rs"]);
}

#[test]
fn empty_or_broken_output_gives_no_entries_and_no_panic() {
    for out in ["", "\0\0", "1", "1 ", "1 .M", "2 R.", "? ", "x y z\0", "u UU"] {
        let _ = parse_status(out, "");
    }
    assert!(parse_status("", "").is_empty());
}

#[test]
fn a_clean_repository_has_no_entries() {
    let (_dir, project) = repo();
    let state = snapshot(project.as_ref()).unwrap();
    assert!(state.entries.is_empty() && state.blobs.is_empty());
}

#[test]
fn a_changed_file_has_an_entry_and_the_blob_of_what_it_holds() {
    let (dir, project) = repo();
    fs::write(dir.path().join("a.txt"), "a changed\n").unwrap();
    fs::write(dir.path().join("sub/new.txt"), "n\n").unwrap();
    fs::remove_file(dir.path().join("sub/b.txt")).unwrap();
    let state = snapshot(project.as_ref()).unwrap();
    assert!(!state.entries["a.txt"].untracked);
    assert!(state.entries["sub/new.txt"].untracked);
    assert!(state.entries["sub/b.txt"].deleted);
    assert_eq!(state.blobs.len(), 2, "a deleted file has no blob");
    assert_ne!(state.blobs["a.txt"], state.blobs["sub/new.txt"]);
    // The same content is the same blob, so a file changed back is seen as unchanged.
    let before = state.blobs["a.txt"].clone();
    fs::write(dir.path().join("a.txt"), "a changed\n").unwrap();
    assert_eq!(snapshot(project.as_ref()).unwrap().blobs["a.txt"], before);
    fs::write(dir.path().join("a.txt"), "a changed again\n").unwrap();
    assert_ne!(snapshot(project.as_ref()).unwrap().blobs["a.txt"], before);
}

#[test]
fn a_folder_that_is_not_a_repository_has_no_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let project = LocalProject::open(dir.path()).unwrap();
    assert!(snapshot(&project).is_none());
}

#[test]
fn many_changed_files_are_hashed_in_batches() {
    let (dir, project) = repo();
    for i in 0..450 {
        fs::write(dir.path().join(format!("f{i}.txt")), format!("{i}\n")).unwrap();
    }
    let state = snapshot(project.as_ref()).unwrap();
    assert_eq!(state.blobs.len(), 450);
    assert_ne!(state.blobs["f0.txt"], state.blobs["f449.txt"]);
}

#[test]
fn the_bytes_of_files_in_the_last_commit_come_from_one_process_and_missing_ones_are_none() {
    let (dir, project) = repo();
    fs::write(dir.path().join("a.txt"), "changed\n").unwrap();
    fs::write(dir.path().join("sub/bin.dat"), [0u8, 1, 2, 255]).unwrap();
    git(dir.path(), &["add", "sub/bin.dat"]);
    git(dir.path(), &["commit", "-q", "-m", "bin"]);
    let heads = head_files(project.as_ref(), &["a.txt", "nope.txt", "sub/b.txt", "sub", "sub/bin.dat", "odd\nname"]);
    assert_eq!(heads["a.txt"].as_deref(), Some(&b"a\n"[..]));
    assert_eq!(heads["nope.txt"], None);
    assert_eq!(heads["sub/b.txt"].as_deref(), Some(&b"b\n"[..]));
    assert_eq!(heads["sub"], None, "a folder is not a file");
    assert_eq!(heads["sub/bin.dat"].as_deref(), Some(&[0u8, 1, 2, 255][..]));
    assert_eq!(heads["odd\nname"], None);
    let inner = LocalProject::open(dir.path().join("sub")).unwrap();
    assert_eq!(head_files(&inner, &["b.txt"])["b.txt"].as_deref(), Some(&b"b\n"[..]));
}

#[test]
fn many_files_are_asked_for_in_batches_and_come_back_each_to_its_path() {
    let (dir, project) = repo();
    for i in 0..1200 {
        fs::write(dir.path().join(format!("f{i}.txt")), format!("content {i}\n")).unwrap();
    }
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "many"]);
    let names: Vec<String> = (0..1200).map(|i| format!("f{i}.txt")).collect();
    let paths: Vec<&str> = names.iter().map(String::as_str).collect();
    let heads = head_files(project.as_ref(), &paths);
    assert_eq!(heads.len(), 1200);
    assert_eq!(heads["f0.txt"].as_deref(), Some(&b"content 0\n"[..]));
    assert_eq!(heads["f1199.txt"].as_deref(), Some(&b"content 1199\n"[..]));
}

#[test]
fn a_folder_that_is_not_a_repository_has_no_head_files() {
    let dir = tempfile::tempdir().unwrap();
    let project = LocalProject::open(dir.path()).unwrap();
    assert_eq!(head_files(&project, &["a.txt"])["a.txt"], None);
}
