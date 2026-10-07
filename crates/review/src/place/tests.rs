use std::sync::Arc;

use atelier_project::LocalProject;

use super::{is_outside, read, remove, write};

#[test]
fn a_leading_slash_names_a_file_beyond_the_project() {
    assert!(is_outside("/tmp/a.txt"));
    assert!(!is_outside("src/a.txt") && !is_outside("a.txt") && !is_outside(""));
}

#[test]
fn read_write_and_remove_go_to_the_place_the_path_names() {
    let folder = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let project = Arc::new(LocalProject::open(folder.path()).unwrap());
    let far = elsewhere.path().join("z.txt").display().to_string();
    write(project.as_ref(), "in.txt", b"in").unwrap();
    write(project.as_ref(), &far, b"far").unwrap();
    assert_eq!(std::fs::read(folder.path().join("in.txt")).unwrap(), b"in", "a relative path is in the folder");
    assert_eq!(std::fs::read(&far).unwrap(), b"far", "an absolute path is where it says");
    assert_eq!((read(project.as_ref(), "in.txt").unwrap(), read(project.as_ref(), &far).unwrap()), (b"in".to_vec(), b"far".to_vec()));
    remove(project.as_ref(), &far).unwrap();
    assert!(!std::path::Path::new(&far).exists() && folder.path().join("in.txt").exists());
}
