use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use super::*;

/// A one-file web server on this machine: it answers every request with `body`, then stops after `requests` of them.
fn serve(body: Vec<u8>, requests: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().take(requests) {
            let mut stream = stream.unwrap();
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
        }
    });
    format!("http://{address}")
}

fn file_of(body: &[u8]) -> ModelFile {
    let name: &'static str = "model.bin";
    ModelFile { remote: name, local: name, size: body.len() as u64, sha256: Box::leak(hex(&Sha256::digest(body)).into_boxed_str()) }
}

#[test]
fn a_fetched_file_is_checked_kept_and_counted() {
    let body: Vec<u8> = (0..700_000u32).map(|i| (i % 251) as u8).collect();
    let base = serve(body.clone(), 1);
    let dir = tempfile::tempdir().unwrap();
    let files = [file_of(&body)];
    let mut seen = Vec::new();
    install(&base, dir.path(), &files, &mut |done, total| seen.push((done, total))).unwrap();
    assert_eq!(fs::read(dir.path().join("model.bin")).unwrap(), body);
    assert!(!dir.path().join("model.bin.part").exists());
    assert!(installed(dir.path(), &files));
    assert!(seen.windows(2).all(|w| w[0].0 <= w[1].0), "progress only goes forward");
    assert!(seen.len() >= 3, "several reports, not one: {}", seen.len());
    assert_eq!(*seen.last().unwrap(), (700_000, 700_000));
}

#[test]
fn a_file_with_the_wrong_hash_is_thrown_away() {
    let body = vec![7u8; 1000];
    let base = serve(vec![8u8; 1000], 1);
    let dir = tempfile::tempdir().unwrap();
    let files = [file_of(&body)];
    let why = install(&base, dir.path(), &files, &mut |_, _| {}).unwrap_err();
    assert!(matches!(why, Error::Checksum("model.bin")), "{why}");
    assert!(!dir.path().join("model.bin").exists() && !dir.path().join("model.bin.part").exists());
    assert!(!installed(dir.path(), &files));
}

#[test]
fn a_file_already_in_place_is_not_fetched_again() {
    let body = vec![3u8; 500];
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("model.bin"), &body).unwrap();
    // Nothing listens at this address, so any request would fail.
    let mut last = (0, 0);
    install("http://127.0.0.1:1", dir.path(), &[file_of(&body)], &mut |done, total| last = (done, total)).unwrap();
    assert_eq!(last, (500, 500));
}

#[test]
fn an_unreachable_server_is_a_network_error() {
    let dir = tempfile::tempdir().unwrap();
    let why = install("http://127.0.0.1:1", dir.path(), &[file_of(b"abc")], &mut |_, _| {}).unwrap_err();
    assert!(matches!(why, Error::Network(_)), "{why}");
}

#[test]
fn only_a_complete_set_counts_as_installed() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!installed(dir.path(), &FILES));
    fs::write(dir.path().join("vocab.txt"), vec![0u8; 9_384]).unwrap();
    assert!(!installed(dir.path(), &FILES));
}

#[test]
fn the_pinned_files_add_up_to_the_download_the_words_promise() {
    let mb = total_bytes(&FILES) as f32 / 1e6;
    assert!((660. ..663.).contains(&mb), "{mb}");
}

#[test]
fn the_old_model_has_its_own_folder_so_it_can_be_cleared() {
    assert_ne!(dir(), legacy_dir());
    assert!(legacy_dir().unwrap().ends_with("phonon-2"));
}
