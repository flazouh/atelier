#[test]
fn a_folder_goes_when_its_test_thread_ends() {
    let made = std::thread::spawn(|| {
        let dir = super::path();
        std::fs::write(dir.join("a"), "a").unwrap();
        assert!(dir.exists());
        dir
    })
    .join()
    .unwrap();
    assert!(!made.exists(), "the folder went with the thread that made it");
}
