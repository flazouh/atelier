use super::*;

#[test]
fn each_extension_gets_its_standard_language_id() {
    let cases = [
        ("a.rs", Some("rust")),
        ("a.ts", Some("typescript")),
        ("a.tsx", Some("typescriptreact")),
        ("a.js", Some("javascript")),
        ("a.jsx", Some("javascriptreact")),
        ("a.py", Some("python")),
        ("a.go", Some("go")),
        ("A.java", Some("java")),
        ("README.md", None),
        ("Makefile", None),
    ];
    for (path, want) in cases {
        assert_eq!(language_id(Path::new(path)), want, "{path}");
    }
}

#[test]
fn every_language_has_exactly_one_server() {
    for (id, _) in LANGUAGES {
        let servers = SERVERS.iter().filter(|s| s.language_ids.contains(id)).count();
        assert_eq!(servers, 1, "{id}");
    }
    assert!(server_for("cobol").is_none());
}

#[test]
fn a_program_is_found_in_the_first_directory_that_has_it() {
    let base = std::env::temp_dir().join(format!("lathe-find-{}", std::process::id()));
    let (first, second) = (base.join("a"), base.join("b"));
    for dir in [&first, &second] {
        std::fs::create_dir_all(dir).unwrap();
    }
    let make = |dir: &Path, executable: bool| {
        let path = dir.join("server");
        std::fs::write(&path, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = if executable { 0o755 } else { 0o644 };
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        }
    };
    make(&first, false);
    make(&second, true);
    let dirs = [first.clone(), second.clone()];
    assert_eq!(find_program_in("server", &dirs), Some(second.join("server")), "a file that cannot run is skipped");
    assert_eq!(find_program_in("missing", &dirs), None);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn the_root_is_the_nearest_directory_with_a_marker() {
    let base = std::env::temp_dir().join(format!("lathe-root-{}", std::process::id()));
    let file = base.join("app/src/deep/main.go");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(base.join("go.mod"), "").unwrap();
    std::fs::write(base.join("app/go.mod"), "").unwrap();
    assert_eq!(find_root(&file, &["go.mod"]), base.join("app"), "the nearest one wins");
    assert_eq!(find_root(&file, &["pom.xml"]), base.join("app/src/deep"), "no marker: the file's directory");
    std::fs::remove_dir_all(&base).ok();
}

/// A pin that misses a platform leaves that platform's users with no server and no way to get one.
#[test]
fn every_platform_pin_covers_every_platform_lathe_downloads_for() {
    let platforms = ["aarch64-apple-darwin", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"];
    let pins = SERVERS.iter().filter_map(|spec| spec.download.as_ref().map(|d| (spec.name, d))).chain([("node", &NODE)]);
    for (name, download) in pins {
        let Kind::Platform { files, .. } = &download.kind else { continue };
        let mut covered: Vec<&str> = files.iter().map(|file| file.platform).collect();
        covered.sort();
        let mut expected = platforms.to_vec();
        expected.sort();
        assert_eq!(covered, expected, "{name}");
    }
}

#[test]
fn no_extension_belongs_to_two_languages() {
    let all: Vec<&str> = LANGUAGES.iter().flat_map(|(_, extensions)| extensions.iter().copied()).collect();
    let mut unique = all.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(all.len(), unique.len(), "an extension listed twice would pick a language by table order");
}
