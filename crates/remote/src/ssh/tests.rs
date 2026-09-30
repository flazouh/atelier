use super::*;

#[test]
fn a_host_is_named_as_builds_are() {
    assert_eq!(Platform::from_uname("Linux x86_64").unwrap().name(), "linux-x86_64");
    assert_eq!(Platform::from_uname("Darwin arm64\n").unwrap().name(), "darwin-aarch64");
    assert_eq!(Platform::from_uname(""), None);
    assert_eq!(remote_binary("0.1.0", "abc123"), ".cache/lathe/remote/0.1.0-abc123/lathe-remote");
    assert_eq!(short_hash(b"lathe").len(), 12);
    assert_ne!(short_hash(b"one build"), short_hash(b"another"), "a new build gets its own folder");
}

#[test]
fn the_config_offers_named_hosts_not_patterns() {
    let config = "Host *\n  ServerAliveInterval 30\nHost hp-agent hp\n  HostName 10.0.0.2\nhost air\nHost !bad *.corp pro\n# Host commented\n";
    assert_eq!(hosts_in_config(config), ["hp-agent", "hp", "air", "pro"]);
}
/// The helper for a host is looked for, in order: the folder a developer names, next to the app by the
/// host's platform, in the Mac bundle's resources, the plain copy next to the app for a host of its own
/// kind, and the folder tools/build-remote.sh fills.
#[test]
fn the_helper_is_looked_for_without_setup() {
    let linux = Platform { system: "linux".into(), arch: "x86_64".into() };
    let exe = std::path::Path::new("/Applications/lathe.app/Contents/MacOS/lathe");
    let home = std::path::Path::new("/Users/a");
    let found: Vec<String> = candidates(exe, &linux, Some(std::path::Path::new("/dev/remote")), Some(home), false)
        .into_iter()
        .map(|p| p.display().to_string())
        .collect();
    assert_eq!(
        found,
        [
            "/dev/remote/linux-x86_64/lathe-remote",
            "/Applications/lathe.app/Contents/MacOS/remote/linux-x86_64/lathe-remote",
            "/Applications/lathe.app/Contents/Resources/remote/linux-x86_64/lathe-remote",
            "/Users/a/.cache/lathe/remote-builds/linux-x86_64/lathe-remote",
        ]
    );
    let here = candidates(std::path::Path::new("/opt/lathe/lathe"), &linux, None, None, true);
    assert!(here.contains(&std::path::PathBuf::from("/opt/lathe/lathe-remote")), "a host of the app's own kind takes the copy beside it: {here:?}");
}
/// With none found, the words say what is missing and what to do, with no setting to know about.
#[test]
fn no_helper_says_what_to_do() {
    let linux = Platform { system: "linux".into(), arch: "x86_64".into() };
    let words = missing_words("hp-agent", &linux);
    assert!(words.contains("hp-agent") && words.contains("Linux x86_64"), "{words}");
    assert!(!words.contains("LATHE_REMOTE_DIR"), "{words}");
    assert!(words.contains("tools/build-remote.sh"), "{words}");
}
