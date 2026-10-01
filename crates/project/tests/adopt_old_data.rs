//! The data the version named lathe kept becomes atelier's, once, and never over atelier's own.
use std::fs;

use atelier_project::adopt_old_data;

#[test]
fn the_old_data_folder_becomes_atelier_s() {
    let data = tempfile::tempdir().unwrap();
    fs::create_dir_all(data.path().join("lathe/projects/app-1")).unwrap();
    fs::write(data.path().join("lathe/settings.json"), "{}").unwrap();
    assert!(adopt_old_data(Some(data.path())).unwrap());
    assert!(data.path().join("atelier/settings.json").exists());
    assert!(data.path().join("atelier/projects/app-1").is_dir());
    assert!(!data.path().join("lathe").exists());
    assert!(!adopt_old_data(Some(data.path())).unwrap(), "once");
}

#[test]
fn atelier_s_own_data_is_never_replaced() {
    let data = tempfile::tempdir().unwrap();
    fs::create_dir_all(data.path().join("lathe")).unwrap();
    fs::create_dir_all(data.path().join("atelier")).unwrap();
    fs::write(data.path().join("atelier/settings.json"), "mine").unwrap();
    assert!(!adopt_old_data(Some(data.path())).unwrap());
    assert_eq!(fs::read_to_string(data.path().join("atelier/settings.json")).unwrap(), "mine");
    assert!(data.path().join("lathe").exists(), "the old folder is left alone");
}

#[test]
fn nothing_to_adopt_is_no_error() {
    let data = tempfile::tempdir().unwrap();
    assert!(!adopt_old_data(Some(data.path())).unwrap());
    assert!(!data.path().join("atelier").exists());
}
