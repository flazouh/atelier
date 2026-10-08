use super::*;

#[test]
fn a_host_is_named_as_builds_are() {
    assert_eq!(Platform::from_uname("Linux x86_64").unwrap().name(), "linux-x86_64");
    assert_eq!(Platform::from_uname("Darwin arm64\n").unwrap().name(), "darwin-aarch64");
    assert_eq!(Platform::from_uname(""), None);
    assert_eq!(remote_binary("0.1.0", "abc123"), ".cache/atelier/remote/0.1.0-abc123/atelier-remote");
    assert_eq!(short_hash(b"atelier").len(), 12);
    assert_ne!(short_hash(b"one build"), short_hash(b"another"), "a new build gets its own folder");
}

#[test]
fn the_config_offers_named_hosts_not_patterns() {
    let config = "Host *\n  ServerAliveInterval 30\nHost dev-host hp\n  HostName 10.0.0.2\nhost air\nHost !bad *.corp pro\n# Host commented\n";
    assert_eq!(hosts_in_config(config), ["dev-host", "hp", "air", "pro"]);
}
/// The helper for a host is looked for, in order: the folder a developer names, next to the app by the
/// host's platform, in the Mac bundle's resources, the plain copy next to the app for a host of its own
/// kind, and the folder tools/build-remote.sh fills.
#[test]
fn the_helper_is_looked_for_without_setup() {
    let linux = Platform { system: "linux".into(), arch: "x86_64".into() };
    let exe = std::path::Path::new("/Applications/atelier.app/Contents/MacOS/atelier");
    let home = std::path::Path::new("/Users/a");
    let found: Vec<String> = candidates(exe, &linux, Some(std::path::Path::new("/dev/remote")), Some(home), false)
        .into_iter()
        .map(|p| p.display().to_string())
        .collect();
    assert_eq!(
        found,
        [
            "/dev/remote/linux-x86_64/atelier-remote",
            "/Applications/atelier.app/Contents/MacOS/remote/linux-x86_64/atelier-remote",
            "/Applications/atelier.app/Contents/Resources/remote/linux-x86_64/atelier-remote",
            "/Users/a/.cache/atelier/remote-builds/linux-x86_64/atelier-remote",
        ]
    );
    let here = candidates(std::path::Path::new("/opt/atelier/atelier"), &linux, None, None, true);
    assert!(here.contains(&std::path::PathBuf::from("/opt/atelier/atelier-remote")), "a host of the app's own kind takes the copy beside it: {here:?}");
}
/// With none found, the words say what is missing and what to do, with no setting to know about.
#[test]
fn no_helper_says_what_to_do() {
    let linux = Platform { system: "linux".into(), arch: "x86_64".into() };
    let words = missing_words("dev-host", &linux);
    assert!(words.contains("dev-host") && words.contains("Linux x86_64"), "{words}");
    assert!(!words.contains("ATELIER_REMOTE_DIR"), "{words}");
    assert!(words.contains("tools/build-remote.sh"), "{words}");
}
/// A helper's protocol reads from the stamp in its bytes, with no need to run it, since it may be built
/// for another machine.
#[test]
fn a_helper_says_its_protocol_in_its_bytes() {
    assert_eq!(speaks(b"\x7fELF..atelier-remote-protocol:4;..."), Some(4));
    assert_eq!(speaks(b"atelier-remote-protocol:12;"), Some(12));
    assert_eq!(speaks(b"an old helper with no stamp"), None);
    assert_eq!(crate::protocol::STAMP, format!("atelier-remote-protocol:{};", crate::protocol::VERSION).as_bytes(), "the stamp names this protocol");
}
/// The search skips a helper of another protocol and takes the next one that matches.
#[test]
fn the_search_skips_a_helper_of_another_protocol() {
    let dir = tempfile::tempdir().unwrap();
    let (old, new) = (dir.path().join("old"), dir.path().join("new"));
    std::fs::write(&old, b"atelier-remote-protocol:1;").unwrap();
    std::fs::write(&new, crate::protocol::STAMP).unwrap();
    let missing = dir.path().join("missing");
    assert_eq!(first_matching(&[missing, old.clone(), new.clone()]), Some(new));
    assert_eq!(first_matching(&[old]), None);
}
/// What the copy on a host answers to --version, for the check before it is used.
#[test]
fn the_version_names_the_protocol() {
    assert_eq!(version_line(), format!("{VERSION} protocol {}", crate::protocol::VERSION));
}

/// A20: a helper in a real Mac bundle, at Contents/Resources/remote/<platform>/atelier-remote, is found from the
/// app at Contents/MacOS/atelier, and a stale copy earlier in the order is passed over.
#[test]
fn a_helper_in_the_mac_bundle_is_found() {
    let linux = Platform { system: "linux".into(), arch: "x86_64".into() };
    let root = std::env::temp_dir().join(format!("atelier-bundle-{}", std::process::id()));
    let contents = root.join("atelier.app/Contents");
    let put = |path: std::path::PathBuf, bytes: &[u8]| {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    };
    let current = format!("ELF atelier-remote-protocol:{};", crate::protocol::VERSION);
    let bundled = contents.join("Resources/remote/linux-x86_64/atelier-remote");
    put(bundled.clone(), current.as_bytes());
    put(contents.join("MacOS/remote/linux-x86_64/atelier-remote"), b"ELF atelier-remote-protocol:0;");
    let exe = contents.join("MacOS/atelier");
    let found = first_matching(&candidates(&exe, &linux, None, None, false));
    std::fs::remove_dir_all(&root).ok();
    assert_eq!(found, Some(bundled));
}

/// The copy is written under a name of its own, so two connections to one host never share the file they
/// write, and the command leaves the finished helper and nothing else.
#[test]
fn an_upload_writes_a_file_of_its_own_and_leaves_only_the_helper() {
    let command = upload_command("bin/atelier-remote");
    assert!(command.contains(".part.$$"), "{command}");
    let dir = tempfile::tempdir().unwrap();
    let run = |bytes: &[u8]| {
        use std::io::Write;
        let mut child = std::process::Command::new("sh")
            .args(["-c", &command])
            .current_dir(dir.path())
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(bytes).unwrap();
        assert!(child.wait().unwrap().success());
    };
    run(b"first");
    run(b"second");
    let names: Vec<_> = std::fs::read_dir(dir.path().join("bin")).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["atelier-remote"]);
    assert_eq!(std::fs::read(dir.path().join("bin/atelier-remote")).unwrap(), b"second");
}

/// A copy that is found but speaks another protocol is told as that, with both numbers, not as a copy that is missing.
#[test]
fn an_outdated_helper_is_told_as_outdated_not_as_missing() {
    let dir = std::env::temp_dir().join(format!("atelier-helper-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let old = dir.join("atelier-remote");
    std::fs::write(&old, b"junk atelier-remote-protocol:6; more").unwrap();
    let none = dir.join("not-there");
    assert_eq!(first_found(&[none.clone(), old.clone()]), Some((old.clone(), Some(6))));
    assert_eq!(first_found(&[none]), None);
    let linux = Platform { system: "linux".into(), arch: "x86_64".into() };
    let words = outdated_words("dev-host", &linux, &old, Some(6));
    assert!(words.contains("protocol 6") && words.contains(&format!("protocol {}", crate::protocol::VERSION)), "{words}");
    assert!(words.contains(&old.display().to_string()) && words.contains("build-remote.sh"), "{words}");
    assert!(!words.contains("no helper built"), "{words}");
    std::fs::remove_dir_all(&dir).ok();
}
