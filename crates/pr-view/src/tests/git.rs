use std::path::Path;

use lathe_forge::Change;

use super::repo::{Scenario, git, git_ok, put, remove};
use crate::git::{Blob, FileEntry, GitError, PrGit, is_sha, parse_batch, parse_commits, parse_files, remote_for};

const A: &str = "fn one() {}\nfn two() {}\nfn three() {}\n";

fn scenario() -> Scenario {
    Scenario::new(&[("src/a.rs", A), ("src/b.rs", "pub fn b() {}\n"), ("README.md", "readme\n"), ("old/name.rs", "let x = 1;\nlet y = 2;\nlet z = 3;\nlet w = 4;\n")])
}

#[test]
fn a_pull_request_is_fetched_into_a_cache_the_project_never_sees() {
    let s = scenario();
    let head = s.pull(5, "main", &[&|r: &Path| put(r, "src/a.rs", "fn one() {}\nfn two() { 2 }\nfn three() {}\n")]);
    let (_, pull) = s.pull_data(5, &head, &s.main_tip());
    let prepared = s.prgit().prepare(&pull).unwrap();
    assert_eq!(prepared.head, head);
    assert_eq!(prepared.merge_base, s.main_tip());
    assert!(prepared.cache.starts_with(s.data.to_str().unwrap()));
    // The project has not got the commit, and has no new ref.
    assert!(!git_ok(&s.work, &["cat-file", "-e", &format!("{head}^{{commit}}")]), "the project does not have the pull request's commit");
    assert_eq!(git(&s.work, &["for-each-ref", "--format=%(refname)"]), "refs/heads/main\nrefs/remotes/origin/HEAD\nrefs/remotes/origin/main");
    assert_eq!(git(&s.work, &["status", "--porcelain"]), "");
}

#[test]
fn a_second_prepare_of_the_same_head_fetches_nothing() {
    let s = scenario();
    let head = s.pull(5, "main", &[&|r: &Path| put(r, "src/b.rs", "pub fn b() { 1 }\n")]);
    let (_, pull) = s.pull_data(5, &head, &s.main_tip());
    let prgit = s.prgit();
    let first = prgit.prepare(&pull).unwrap();
    // The forge goes away: a second prepare must still work, from the cache alone.
    std::fs::remove_dir_all(&s.origin).unwrap();
    let second = prgit.prepare(&pull).unwrap();
    assert_eq!(first, second);
}

#[test]
fn the_pull_requests_own_changes_start_at_the_merge_base_even_when_main_moved_on() {
    let s = scenario();
    let base = s.main_tip();
    let head = s.pull(6, "main", &[&|r: &Path| put(r, "src/a.rs", "fn one() { 1 }\nfn two() {}\nfn three() {}\n")]);
    s.advance_main("README.md", "readme, changed on main\n");
    // GitHub says the base is where the pull request stands; main has moved since.
    let (_, pull) = s.pull_data(6, &head, &base);
    let prgit = s.prgit();
    let prepared = prgit.prepare(&pull).unwrap();
    assert_eq!(prepared.merge_base, base);
    let files = prgit.files(&prepared, &prepared.merge_base).unwrap();
    assert_eq!(files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["src/a.rs"], "main's change to README is not in it");
    // An empty base_sha (an older recording) falls back to the tip of the base branch and finds the same place.
    let (_, without) = s.pull_data(6, &head, "");
    assert_eq!(prgit.prepare(&without).unwrap().merge_base, base);
}

#[test]
fn a_merged_pull_request_is_still_the_diff_it_was() {
    // After a merge, main holds the head. The base the forge gives is main before the merge.
    let s = scenario();
    let base = s.main_tip();
    let head = s.pull(7, "main", &[&|r: &Path| put(r, "src/b.rs", "pub fn b() { 7 }\n")]);
    git(&s.dir.path().join("scratch"), &["checkout", "-q", "main"]);
    git(&s.dir.path().join("scratch"), &["merge", "-q", "--ff-only", &head]);
    git(&s.dir.path().join("scratch"), &["push", "-q", "origin", "main"]);
    let (_, pull) = s.pull_data(7, &head, &base);
    let prgit = s.prgit();
    let prepared = prgit.prepare(&pull).unwrap();
    assert_eq!(prepared.merge_base, base, "not the head itself, which main now holds");
    assert_eq!(prgit.files(&prepared, &base).unwrap().len(), 1);
}

#[test]
fn commits_come_newest_first_with_title_author_and_time() {
    let s = scenario();
    let base = s.main_tip();
    let head = s.pull(8, "main", &[&|r: &Path| put(r, "src/a.rs", "1\n"), &|r: &Path| put(r, "src/b.rs", "2\n"), &|r: &Path| put(r, "src/c.rs", "3\n")]);
    let (_, pull) = s.pull_data(8, &head, &base);
    let prgit = s.prgit();
    let prepared = prgit.prepare(&pull).unwrap();
    let commits = prgit.commits(&prepared, &prepared.merge_base).unwrap();
    assert_eq!(commits.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(), ["Change 3 of #8", "Change 2 of #8", "Change 1 of #8"]);
    assert_eq!(commits[0].sha, head);
    assert_eq!((commits[0].author.as_str(), commits[0].at), ("Rui", 1_788_256_800));
    // Since a commit: only the ones after it.
    assert_eq!(prgit.commits(&prepared, &commits[2].sha).unwrap().len(), 2);
    assert!(prgit.commits(&prepared, "not-a-sha").is_err());
}

#[test]
fn files_list_added_modified_deleted_and_renamed_with_their_counts_and_blobs() {
    let s = scenario();
    let base = s.main_tip();
    let head = s.pull(9, "main", &[&|r: &Path| {
        put(r, "src/a.rs", "fn one() {}\nfn two() { 2 }\nfn three() {}\nfn four() {}\n");
        put(r, "src/new file.rs", "pub fn new() {}\n");
        remove(r, "src/b.rs");
        std::fs::create_dir_all(r.join("moved")).unwrap();
        std::fs::rename(r.join("old/name.rs"), r.join("moved/name.rs")).unwrap();
        std::fs::write(r.join("logo.bin"), [0u8, 159, 146, 150, 0, 1]).unwrap();
    }]);
    let (_, pull) = s.pull_data(9, &head, &base);
    let prgit = s.prgit();
    let prepared = prgit.prepare(&pull).unwrap();
    let files = prgit.files(&prepared, &base).unwrap();
    let by = |path: &str| -> &FileEntry { files.iter().find(|f| f.path == path).unwrap_or_else(|| panic!("no {path} in {files:#?}")) };
    let a = by("src/a.rs");
    assert_eq!((a.change, a.additions, a.deletions, a.binary), (Change::Modified, 2, 1, false));
    assert!(a.old_blob.is_some() && a.new_blob.is_some() && a.old_blob != a.new_blob);
    let added = by("src/new file.rs");
    assert_eq!((added.change, added.old_blob.clone(), added.additions), (Change::Added, None, 1));
    assert_eq!(by("src/b.rs").change, Change::Deleted);
    assert_eq!(by("src/b.rs").new_blob, None);
    assert_eq!(by("src/b.rs").version(), by("src/b.rs").old_blob.as_deref().unwrap(), "a deleted file's version is its old blob");
    let moved = by("moved/name.rs");
    assert_eq!((moved.change, moved.old_path.as_deref(), moved.additions, moved.deletions), (Change::Renamed, Some("old/name.rs"), 0, 0));
    assert!(by("logo.bin").binary);
    assert_eq!(files.len(), 5);
}

#[test]
fn blobs_read_in_one_process_with_text_binary_and_missing_told_apart() {
    let s = scenario();
    let base = s.main_tip();
    let head = s.pull(10, "main", &[&|r: &Path| {
        put(r, "src/a.rs", "fn one() {}\r\nfn two() { 2 }\n");
        std::fs::write(r.join("blob.bin"), [1u8, 0, 2]).unwrap();
        put(r, "big.txt", &"x".repeat(5 * 1024 * 1024));
        put(r, "src/héllo wörld.rs", "// ünï\n");
    }]);
    let (_, pull) = s.pull_data(10, &head, &base);
    let prgit = s.prgit();
    let prepared = prgit.prepare(&pull).unwrap();
    let files = prgit.files(&prepared, &base).unwrap();
    let blob = |path: &str| files.iter().find(|f| f.path == path).unwrap().new_blob.clone().unwrap();
    let (a, bin, big, odd) = (blob("src/a.rs"), blob("blob.bin"), blob("big.txt"), blob("src/héllo wörld.rs"));
    let read = prgit.blobs(&prepared, &[Some(&a), None, Some(&bin), Some(&big), Some(&odd), Some("0123456789abcdef0123456789abcdef01234567")]).unwrap();
    assert_eq!(read[0], Blob::Text("fn one() {}\r\nfn two() { 2 }\n".into()), "line ends survive");
    assert_eq!(read[1], Blob::Missing, "no sha, no file");
    assert_eq!(read[2], Blob::Binary);
    assert_eq!(read[3], Blob::TooLarge(5 * 1024 * 1024));
    assert_eq!(read[4], Blob::Text("// ünï\n".into()));
    assert_eq!(read[5], Blob::Missing, "a sha the cache lacks");
    assert!(prgit.blobs(&prepared, &[Some("--upload-pack=x")]).is_err(), "a value that is no sha never reaches git");
}

#[test]
fn a_file_the_pull_request_did_not_change_is_read_from_the_head() {
    let s = scenario();
    let base = s.main_tip();
    let head = s.pull(11, "main", &[&|r: &Path| put(r, "dir/with space/x.rs", "let brought = 1;\n")]);
    let (_, pull) = s.pull_data(11, &head, &base);
    let prgit = s.prgit();
    let prepared = prgit.prepare(&pull).unwrap();
    assert_eq!(prgit.head_file(&prepared, "README.md").unwrap(), Blob::Text("readme\n".into()));
    assert_eq!(prgit.head_file(&prepared, "dir/with space/x.rs").unwrap(), Blob::Text("let brought = 1;\n".into()));
    assert_eq!(prgit.head_file(&prepared, "nothing.rs").unwrap(), Blob::Missing);
    assert!(prgit.head_file(&prepared, "-x").is_err());
    let all = prgit.head_files(&prepared).unwrap();
    assert!(all.contains(&"README.md".to_string()) && all.contains(&"dir/with space/x.rs".to_string()));
}

#[test]
fn since_a_commit_the_diff_is_only_what_came_after_it() {
    let s = scenario();
    let base = s.main_tip();
    let head = s.pull(12, "main", &[&|r: &Path| put(r, "src/a.rs", "first\n"), &|r: &Path| put(r, "src/b.rs", "second\n")]);
    let (_, pull) = s.pull_data(12, &head, &base);
    let prgit = s.prgit();
    let prepared = prgit.prepare(&pull).unwrap();
    let commits = prgit.commits(&prepared, &base).unwrap();
    let after_first = prgit.files(&prepared, &commits[1].sha).unwrap();
    assert_eq!(after_first.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["src/b.rs"]);
    assert!(prgit.is_ancestor(&prepared, &commits[1].sha, &head));
    assert!(!prgit.is_ancestor(&prepared, &head, &commits[1].sha));
    assert!(prgit.knows(&prepared, &commits[1].sha));
    assert!(!prgit.knows(&prepared, "0123456789abcdef0123456789abcdef01234567"));
    assert!(!prgit.knows(&prepared, "zzzz"));
}

#[test]
fn a_force_push_makes_the_old_review_point_a_stranger() {
    let s = scenario();
    let base = s.main_tip();
    let old_head = s.pull(13, "main", &[&|r: &Path| put(r, "src/a.rs", "old way\n")]);
    let (_, old_pull) = s.pull_data(13, &old_head, &base);
    let prgit = s.prgit();
    prgit.prepare(&old_pull).unwrap();
    let new_head = s.force_push(13, "main", &|r: &Path| put(r, "src/a.rs", "new way\n"));
    let (_, pull) = s.pull_data(13, &new_head, &base);
    let prepared = prgit.prepare(&pull).unwrap();
    assert_eq!(prepared.head, new_head);
    assert!(prgit.knows(&prepared, &old_head), "the cache still has it");
    assert!(!prgit.is_ancestor(&prepared, &old_head, &new_head), "but it is no part of this branch");
}

#[test]
fn a_head_the_forge_no_longer_has_is_said_so() {
    let s = scenario();
    let (_, mut pull) = s.pull_data(14, "0123456789abcdef0123456789abcdef01234567", &s.main_tip());
    pull.head_sha = "0123456789abcdef0123456789abcdef01234567".into();
    let error = s.prgit().prepare(&pull).unwrap_err();
    assert!(matches!(error, GitError::Failed { .. } | GitError::HeadGone(_)), "{error:?}");
}

#[test]
fn a_hostile_branch_name_or_sha_never_becomes_an_option() {
    let s = scenario();
    let head = s.pull(15, "main", &[&|r: &Path| put(r, "x", "x\n")]);
    let prgit = s.prgit();
    for bad in ["--upload-pack=touch /tmp/pwned", "-x", "a b", "a..b", "x\ny", "", "a:b"] {
        let (_, mut pull) = s.pull_data(15, &head, "");
        pull.base = bad.into();
        assert!(matches!(prgit.prepare(&pull), Err(GitError::Invalid(_))), "{bad:?}");
    }
    let (_, mut pull) = s.pull_data(15, &head, "");
    pull.head_sha = "--exec=x".into();
    assert!(matches!(prgit.prepare(&pull), Err(GitError::Invalid(_))));
    assert!(!Path::new("/tmp/pwned").exists());
}

#[test]
fn the_head_is_checked_out_once_reused_and_made_again_when_it_moves() {
    let s = scenario();
    let base = s.main_tip();
    let head = s.pull(16, "main", &[&|r: &Path| put(r, "src/a.rs", "checked out\n")]);
    let (_, pull) = s.pull_data(16, &head, &base);
    let prgit = s.prgit();
    let prepared = prgit.prepare(&pull).unwrap();
    let dir = prgit.checkout(&prepared).unwrap();
    assert_eq!(std::fs::read_to_string(format!("{dir}/src/a.rs")).unwrap(), "checked out\n");
    assert!(Path::new(&format!("{dir}/README.md")).exists(), "the whole head, not only the changes");
    assert!(!Path::new(&format!("{dir}/.git")).exists(), "files only: no repository, no worktree");
    // A file the user made in it stays while the head is the same.
    std::fs::write(format!("{dir}/marker.txt"), "still here").unwrap();
    assert_eq!(prgit.checkout(&prepared).unwrap(), dir);
    assert!(Path::new(&format!("{dir}/marker.txt")).exists(), "reused, not rebuilt");
    // The head moves: made again.
    let new_head = s.force_push(16, "main", &|r: &Path| put(r, "src/a.rs", "moved\n"));
    let (_, moved) = s.pull_data(16, &new_head, &base);
    let prepared = prgit.prepare(&moved).unwrap();
    assert_eq!(prgit.checkout(&prepared).unwrap(), dir, "same folder, so a server root stays put");
    assert_eq!(std::fs::read_to_string(format!("{dir}/src/a.rs")).unwrap(), "moved\n");
    assert!(!Path::new(&format!("{dir}/marker.txt")).exists());
    // The project itself has no worktree.
    assert_eq!(git(&s.work, &["worktree", "list"]).lines().count(), 1);
}

#[test]
fn a_closed_pull_requests_checkout_goes_and_the_others_stay() {
    let s = scenario();
    let base = s.main_tip();
    let prgit = s.prgit();
    let mut dirs = Vec::new();
    for n in [21, 22, 23] {
        let head = s.pull(n, "main", &[&|r: &Path| put(r, "src/a.rs", &format!("pr {n}\n"))]);
        let (_, pull) = s.pull_data(n, &head, &base);
        let prepared = prgit.prepare(&pull).unwrap();
        dirs.push(prgit.checkout(&prepared).unwrap());
    }
    let repo = s.pull_data(21, "abcd", "").0.repo;
    assert_eq!(prgit.sweep(&repo, &[22]).unwrap(), vec![21, 23]);
    assert!(!Path::new(&dirs[0]).exists() && Path::new(&dirs[1]).exists() && !Path::new(&dirs[2]).exists());
    assert!(!Path::new(&format!("{}.sha", dirs[0])).exists());
    prgit.remove(&s.pull_data(22, "abcd", "").0).unwrap();
    assert!(!Path::new(&dirs[1]).exists());
    assert!(prgit.sweep(&repo, &[]).unwrap().is_empty(), "nothing left to sweep");
    let cache = format!("{}/{}/cache.git", s.data.display(), PrGit::key(&repo));
    assert!(Path::new(&cache).exists(), "the cache stays for the next pull request");
}

#[test]
fn the_remote_that_names_the_repository_is_the_one_fetched_from() {
    let repo = lathe_forge::RepoRef::new("github.com", "flazouh", "relay");
    let listing = "upstream\tgit@github.com:other/thing.git (fetch)\nupstream\tgit@github.com:other/thing.git (push)\nfork\thttps://github.com/flazouh/relay.git (fetch)\nfork\thttps://github.com/flazouh/relay.git (push)\norigin\tgit@github.com:flazouh/relay.git (fetch)\norigin\tgit@github.com:flazouh/relay.git (push)\n";
    assert_eq!(remote_for(listing, &repo).as_deref(), Some("git@github.com:flazouh/relay.git"), "origin wins");
    assert_eq!(remote_for("fork\thttps://github.com/flazouh/relay.git (fetch)\n", &repo).as_deref(), Some("https://github.com/flazouh/relay.git"));
    assert_eq!(remote_for(listing, &lathe_forge::RepoRef::new("github.com", "nobody", "none")), None);
}

#[test]
fn the_data_folder_is_resolved_on_the_host() {
    let s = scenario();
    let home = PrGit::new(s.project(), "~/lathe-test-data").data().unwrap();
    assert!(home.starts_with('/') && home.ends_with("/lathe-test-data") && !home.contains('~'));
    assert!(PrGit::new(s.project(), "relative/folder").data().is_err());
    assert_eq!(PrGit::new(s.project(), "/abs/folder").data().unwrap(), "/abs/folder");
}

#[test]
fn the_parsers_read_git_output() {
    assert!(is_sha("abcd") && is_sha(&"a".repeat(40)) && !is_sha("abc") && !is_sha("xyz1") && !is_sha(&"a".repeat(65)));
    let commits = parse_commits("aaaa111\u{1f}Ada Lovelace\u{1f}1700000000\u{1f}Fix the thing\0bbbb222\u{1f}Rui\u{1f}1699999999\u{1f}Title with \u{1f} inside? no\0");
    assert_eq!(commits.len(), 2);
    assert_eq!((commits[0].author.as_str(), commits[0].at, commits[0].title.as_str()), ("Ada Lovelace", 1_700_000_000, "Fix the thing"));
    let raw = b":100644 100644 1111111111111111111111111111111111111111 2222222222222222222222222222222222222222 M\0a.rs\0:000000 100644 0000000000000000000000000000000000000000 3333333333333333333333333333333333333333 A\0b c.rs\0:100644 100644 4444444444444444444444444444444444444444 4444444444444444444444444444444444444444 R100\0old.rs\0new.rs\0";
    let stat = b"3\t1\ta.rs\0-\t-\tb c.rs\0\x30\t0\t\0old.rs\0new.rs\0";
    let files = parse_files(raw, stat);
    assert_eq!(files.len(), 3);
    assert_eq!((files[0].additions, files[0].deletions, files[0].change), (3, 1, Change::Modified));
    assert!(files[1].binary && files[1].old_blob.is_none() && files[1].path == "b c.rs");
    assert_eq!((files[2].old_path.as_deref(), files[2].path.as_str(), files[2].change), (Some("old.rs"), "new.rs", Change::Renamed));
    let batch = b"abc blob 5\nhello\nfoo bar missing\nabc blob 3\nl\0x\ndir/with space blob 2\nhi\n";
    assert_eq!(parse_batch(batch), vec![Blob::Text("hello".into()), Blob::Missing, Blob::Binary, Blob::Text("hi".into())]);
    assert!(parse_batch(b"abc blob 500\nshort").is_empty(), "a body cut short is dropped, not read past");
}
