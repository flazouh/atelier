use std::{cell::RefCell, process::Command};

use super::*;

/// A fresh folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lathe-install-{name}-{}", std::process::id()));
    _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn leak(text: String) -> &'static str {
    text.leak()
}

/// A gzipped shell script that prints `hello`, served from a `file://` URL.
fn gzipped_script(dir: &Path) -> (&'static str, &'static str) {
    let script = dir.join("server");
    fs::write(&script, "#!/bin/sh\necho hello\n").unwrap();
    assert!(Command::new("gzip").arg("-f").arg(&script).status().unwrap().success());
    let archive = dir.join("server.gz");
    (leak(format!("file://{}", archive.display())), leak(sha256_of(&archive).unwrap()))
}

/// An npm-style tarball whose one file is `package/index.js`.
fn npm_tarball(dir: &Path) -> (&'static str, &'static str) {
    fs::create_dir_all(dir.join("package")).unwrap();
    fs::write(dir.join("package/index.js"), "console.log('hello')\n").unwrap();
    let archive = dir.join("package.tgz");
    assert!(Command::new("tar").arg("-czf").arg(&archive).arg("-C").arg(dir).arg("package").status().unwrap().success());
    (leak(format!("file://{}", archive.display())), leak(sha256_of(&archive).unwrap()))
}

/// A server spec named `server` that downloads `download`.
fn spec(download: Option<Download>) -> ServerSpec {
    ServerSpec {
        name: "server",
        program: "server",
        args: &["--stdio"],
        language_ids: &[],
        root_markers: &[],
        install: "install it",
        download,
        initialization_options: |_, _| None,
    }
}

/// The same file for every platform lathe downloads for.
fn binary(url: &'static str, sha256: &'static str) -> Option<Download> {
    let files = ["aarch64-apple-darwin", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"]
        .map(|platform| PlatformFile { platform, url, sha256 });
    Some(Download {
        version: "1.0",
        kind: Kind::Platform { files: Vec::from(files).leak(), unpack: Unpack::Gunzip, program: "server" },
    })
}

/// A log of what `report` heard.
type Log = RefCell<Vec<String>>;

/// Everything `report` heard.
fn heard() -> (Log, impl Fn(&Log, String)) {
    (RefCell::default(), |log: &RefCell<Vec<String>>, line| log.borrow_mut().push(line))
}

/// The names in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn a_binary_is_downloaded_checked_and_run() {
    let (fixtures, store_dir) = (scratch("binary-fixtures"), scratch("binary-store"));
    let (url, sha256) = gzipped_script(&fixtures);
    let store = Store::new(store_dir.clone(), vec![], false);
    let (log, push) = heard();

    let launch = store.launch(&spec(binary(url, sha256)), &|line| push(&log, line)).expect("it downloads");

    assert_eq!(launch, Launch::direct(store_dir.join("server/1.0/server")));
    assert_eq!(launch.args(&spec(None)), ["--stdio"]);
    let output = Command::new(&launch.program).output().expect("the download runs");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello\n");
    assert_eq!(*log.borrow(), ["downloading server 1.0"]);
    assert_eq!(names(&store_dir), ["server"], "no staging folder is left behind");
    assert_eq!(names(&store_dir.join("server/1.0")), ["server"], "the archive is gone");
}

#[test]
fn a_finished_copy_is_used_without_a_download() {
    let (fixtures, store_dir) = (scratch("cached-fixtures"), scratch("cached-store"));
    let (url, sha256) = gzipped_script(&fixtures);
    let store = Store::new(store_dir.clone(), vec![], false);
    store.launch(&spec(binary(url, sha256)), &|_| {}).expect("the first launch downloads");
    fs::remove_dir_all(&fixtures).unwrap();
    let (log, push) = heard();

    let launch = store.launch(&spec(binary(url, sha256)), &|line| push(&log, line)).expect("the copy is kept");

    assert_eq!(launch.program, store_dir.join("server/1.0/server"));
    assert!(log.borrow().is_empty(), "nothing was downloaded");
}

#[test]
fn a_wrong_checksum_leaves_nothing_behind() {
    let (fixtures, store_dir) = (scratch("checksum-fixtures"), scratch("checksum-store"));
    let (url, _) = gzipped_script(&fixtures);
    let store = Store::new(store_dir.clone(), vec![], false);

    let error = store.launch(&spec(binary(url, "00")), &|_| {}).expect_err("the file is not the pinned one");

    assert!(matches!(&error, Unavailable::Failed(reason) if reason.contains("is not the pinned file")), "{error}");
    assert!(names(&store_dir).is_empty(), "no folder, no staging, no archive: {:?}", names(&store_dir));
}

#[test]
fn a_failed_fetch_says_why() {
    let store_dir = scratch("missing-store");
    let store = Store::new(store_dir.clone(), vec![], false);

    let error = store.launch(&spec(binary("file:///nowhere/lathe-server.gz", "00")), &|_| {}).expect_err("no file");

    assert!(matches!(&error, Unavailable::Failed(reason) if reason.starts_with("downloading file:///nowhere")), "{error}");
    assert!(names(&store_dir).is_empty());
}

#[test]
fn offline_stops_before_the_network() {
    let store = Store::new(scratch("offline-store"), vec![], true);
    let (log, push) = heard();

    let error = store.launch(&spec(binary("file:///nowhere/lathe-server.gz", "00")), &|line| push(&log, line));

    assert_eq!(error, Err(Unavailable::NotInstalled));
    assert!(log.borrow().is_empty());
}

#[test]
fn a_server_the_user_installed_wins() {
    let (search, store_dir) = (scratch("search"), scratch("search-store"));
    let installed = search.join("server");
    fs::write(&installed, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&installed, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let store = Store::new(store_dir.clone(), vec![search], false);

    let launch = store.launch(&spec(binary("file:///nowhere/lathe-server.gz", "00")), &|_| {});

    assert_eq!(launch, Ok(Launch::direct(installed)));
    assert!(names(&store_dir).is_empty(), "nothing was downloaded");
}

#[test]
fn a_server_with_no_pin_is_not_installed() {
    let store = Store::new(scratch("no-pin-store"), vec![], false);
    assert_eq!(store.launch(&spec(None), &|_| {}), Err(Unavailable::NotInstalled));
}

#[test]
fn a_platform_with_no_file_is_not_installed() {
    let store_dir = scratch("no-platform-store");
    let store = Store::new(store_dir.clone(), vec![], false);
    let download = Download { version: "1.0", kind: Kind::Platform { files: &[], unpack: Unpack::Gunzip, program: "server" } };

    assert_eq!(store.launch(&spec(Some(download)), &|_| {}), Err(Unavailable::NotInstalled));
    assert!(names(&store_dir).is_empty());
}

#[test]
fn a_node_server_runs_its_script_on_lathes_node() {
    let (fixtures, store_dir) = (scratch("node-fixtures"), scratch("node-store"));
    let (url, sha256) = npm_tarball(&fixtures);
    // lathe's Node.js is already there, so the test downloads only the package.
    let node = store_dir.join("node").join(NODE.version).join("bin/node");
    fs::create_dir_all(node.parent().unwrap()).unwrap();
    fs::write(&node, "").unwrap();
    let store = Store::new(store_dir.clone(), vec![], false);
    let packages = vec![Package { name: "server", url, sha256 }].leak();
    let download = Download { version: "1.0", kind: Kind::Node { packages, script: "node_modules/server/index.js" } };

    let launch = store.launch(&spec(Some(download)), &|_| {}).expect("it downloads");

    let script = store_dir.join("server/1.0/node_modules/server/index.js");
    assert_eq!(launch, Launch { program: node, script: Some(script.clone()) });
    assert!(script.is_file(), "the tarball's top folder is dropped");
    assert_eq!(launch.args(&spec(None)), [script.to_string_lossy().into_owned(), "--stdio".into()]);
    assert_eq!(launch.server_file(), script, "options look beside the script, not beside Node.js");
}
