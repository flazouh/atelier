use super::*;

#[test]
fn each_changed_file_shows_its_removed_rows_above_its_added_ones() {
    let request = shown(&CHANGES[0]);
    let rows: Vec<&str> = request.text.split('\n').collect();
    let hunk = &request.hunks[0];
    assert_eq!(rows[hunk.removed.start].trim(), "if let Some(response) = self.response.as_mut() {");
    assert_eq!(rows[hunk.removed.start + 1].trim(), "response.write_status(200);");
    assert_eq!(rows[hunk.added.start].trim(), "if let Some(response) = self.response.as_mut() {", "the added rows follow");
    assert_eq!(request.rows.head_text(&request.text), REQUEST.trim_end_matches('\n'), "the file is the head");
}

#[test]
fn a_file_added_whole_is_one_hunk_of_added_rows() {
    let test = shown(&CHANGES[3]);
    assert_eq!((test.added(), test.removed()), (12, 0));
    assert_eq!(test.text, ABORT_TEST.trim_end_matches('\n'));
}

#[test]
fn go_to_file_lists_what_gitignore_leaves() {
    let fixture = Fixture::write();
    std::fs::create_dir_all(fixture.root.join("target/debug")).unwrap();
    std::fs::write(fixture.root.join("target/debug/build.log"), "").unwrap();
    let files = list_files(&fixture.root);
    assert!(files.contains(&"src/config.rs".to_string()));
    assert!(files.contains(&"tests/abort.rs".to_string()));
    assert!(!files.iter().any(|f| f.starts_with("target/")), "the build output is ignored: {files:?}");
    assert!(!files.iter().any(|f| f.starts_with('.')), "hidden files are left out: {files:?}");
}
