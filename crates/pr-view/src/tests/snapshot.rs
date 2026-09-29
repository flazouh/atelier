use crate::{
    PartKind,
    fixture::sample,
    snapshot::Snapshots,
};

fn data() -> crate::PullData {
    let mut data = sample::data(44169, "d405137", Vec::new());
    data.threads = vec![sample::thread("T1", "src/a.rs", 2, vec![sample::comment("c1", "Ada", "Why?", 100)])];
    data
}

#[test]
fn a_pull_request_comes_back_from_disk_as_it_was_saved() {
    let dir = tempfile::tempdir().unwrap();
    let snapshots = Snapshots::new(dir.path().join("pr"));
    let saved = data();
    snapshots.save(&saved).unwrap();
    let loaded = snapshots.load(&saved.reference).unwrap();
    assert_eq!(loaded, saved);
    assert!(loaded.complete() && loaded.has(PartKind::Threads));
    assert!(!dir.path().join("pr").read_dir().unwrap().any(|e| e.unwrap().file_name().to_string_lossy().ends_with(".saving")), "no temporary file is left");
}

#[test]
fn a_pull_request_never_saved_is_a_miss() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Snapshots::new(dir.path()).load(&sample::reference(1)).is_none());
}

#[test]
fn a_damaged_or_foreign_file_is_a_miss_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let snapshots = Snapshots::new(dir.path());
    let saved = data();
    snapshots.save(&saved).unwrap();
    let file = std::fs::read_dir(dir.path()).unwrap().next().unwrap().unwrap().path();
    std::fs::write(&file, "{not json").unwrap();
    assert!(snapshots.load(&saved.reference).is_none());
    std::fs::write(&file, "").unwrap();
    assert!(snapshots.load(&saved.reference).is_none());
    // A file from another version of lathe.
    let mut value: serde_json::Value = serde_json::to_value(serde_json::json!({"version": 0, "data": serde_json::to_value(&saved).unwrap()})).unwrap();
    std::fs::write(&file, value.to_string()).unwrap();
    assert!(snapshots.load(&saved.reference).is_none(), "an old shape is not read");
    value["version"] = serde_json::json!(1);
    std::fs::write(&file, value.to_string()).unwrap();
    assert_eq!(snapshots.load(&saved.reference).unwrap(), saved);
}

#[test]
fn each_pull_request_has_its_own_file_and_remove_forgets_one() {
    let dir = tempfile::tempdir().unwrap();
    let snapshots = Snapshots::new(dir.path());
    let (a, b) = (sample::data(1, "a", Vec::new()), sample::data(2, "b", Vec::new()));
    snapshots.save(&a).unwrap();
    snapshots.save(&b).unwrap();
    assert_eq!(snapshots.load(&a.reference).unwrap().pull.unwrap().head_sha, "a");
    snapshots.remove(&a.reference);
    assert!(snapshots.load(&a.reference).is_none());
    assert!(snapshots.load(&b.reference).is_some());
}
