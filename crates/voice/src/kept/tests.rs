use super::*;

#[test]
fn a_kept_recording_comes_back_as_it_went_in() {
    let dir = tempfile::tempdir().unwrap();
    let samples = vec![0.25, -0.5, 1.0, 0.0];
    let path = save(dir.path(), "session-7", &samples).unwrap();
    assert_eq!(load(&path).unwrap(), ("session-7".to_string(), samples));
}

#[test]
fn recordings_list_oldest_first_and_only_whole_ones() {
    let dir = tempfile::tempdir().unwrap();
    let first = save(dir.path(), "a", &[0.1]).unwrap();
    let second = save(dir.path(), "b", &[0.2]).unwrap();
    std::fs::write(dir.path().join("0000000000000000-0.part"), b"torn").unwrap();
    assert_eq!(list(dir.path()), vec![first, second]);
}

#[test]
fn a_missing_folder_lists_nothing() {
    assert!(list(std::path::Path::new("/nonexistent/atelier/waiting")).is_empty());
}

#[test]
fn a_file_that_is_not_a_recording_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.clip");
    std::fs::write(&path, [200, 0, 0, 0, b'a']).unwrap();
    assert!(load(&path).is_err());
    std::fs::write(&path, [1, 0, 0, 0, b'a', 1, 2]).unwrap();
    assert!(load(&path).is_err(), "samples cut short");
}
