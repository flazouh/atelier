use atelier_ui::DiffLineKind;

use super::*;

#[test]
fn the_log_reads_one_commit_a_record_newest_first() {
    let out = "aaa111\u{1f}aaa\u{1f}Ada\u{1f}1700000100\u{1f}Fix the scroll\u{1e}\nbbb222\u{1f}bbb\u{1f}Grace\u{1f}1700000000\u{1f}A | B: pipes stay\u{1e}\n";
    let log = parse_log(out);
    assert_eq!(log.len(), 2);
    assert_eq!((log[0].short.as_ref(), log[0].author.as_ref(), log[0].at), ("aaa", "Ada", 1_700_000_100));
    assert_eq!(log[1].subject, "A | B: pipes stay");
}

#[test]
fn a_record_that_does_not_read_is_skipped() {
    assert!(parse_log("").is_empty());
    assert!(parse_log("half\u{1f}a record\u{1e}").is_empty());
    assert_eq!(parse_log("x\u{1f}x\u{1f}a\u{1f}not a time\u{1f}s\u{1e}c\u{1f}c\u{1f}a\u{1f}5\u{1f}s\u{1e}").len(), 1);
}

#[test]
fn a_show_splits_into_the_message_and_each_files_lines() {
    let out = "Fix the scroll\n\nThe body.\n\u{1e}\n\
diff --git a/src/a.rs b/src/a.rs\n\
index 1..2 100644\n\
--- a/src/a.rs\n\
+++ b/src/a.rs\n\
@@ -1,2 +1,2 @@\n\
 keep\n\
-old\n\
+new\n\
diff --git a/gone.txt b/gone.txt\n\
deleted file mode 100644\n\
--- a/gone.txt\n\
+++ /dev/null\n\
@@ -1 +0,0 @@\n\
-bye\n";
    let shown = split_show("abc", out);
    assert_eq!(shown.message, "Fix the scroll\n\nThe body.");
    let paths: Vec<&str> = shown.files.iter().map(|f| f.path.as_ref()).collect();
    assert_eq!(paths, ["src/a.rs", "gone.txt"], "a deleted file keeps its old path");
    let kinds: Vec<DiffLineKind> = shown.files[0].lines.iter().map(|l| l.kind).collect();
    assert_eq!(kinds, [DiffLineKind::Hunk, DiffLineKind::Context, DiffLineKind::Removed, DiffLineKind::Added]);
    assert_eq!(shown.files[1].lines.len(), 2);
}

#[test]
fn a_moved_file_is_named_by_its_new_path() {
    let out = "Move\u{1e}\ndiff --git a/old.rs b/new.rs\nsimilarity index 90%\nrename from old.rs\nrename to new.rs\n--- a/old.rs\n+++ b/new.rs\n@@ -1 +1 @@\n-a\n+b\n";
    assert_eq!(split_show("s", out).files[0].path, "new.rs");
}

#[test]
fn a_commit_with_no_changes_has_no_files() {
    let shown = split_show("s", "Empty\n\u{1e}\n");
    assert_eq!(shown.message, "Empty");
    assert!(shown.files.is_empty());
}
