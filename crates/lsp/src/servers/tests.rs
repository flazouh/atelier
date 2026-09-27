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
fn every_language_id_has_exactly_one_server() {
    for id in ["rust", "typescript", "typescriptreact", "javascript", "javascriptreact", "python", "go", "java"] {
        let servers = SERVERS.iter().filter(|s| s.language_ids.contains(&id)).count();
        assert_eq!(servers, 1, "{id}");
        assert!(server_for(id).is_some());
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

#[cfg(unix)]
#[test]
fn typescript_falls_back_to_the_typescript_beside_the_server() {
    let base = std::env::temp_dir().join(format!("lathe-ts-{}", std::process::id()));
    let modules = base.join("prefix/lib/node_modules");
    let cli = modules.join("typescript-language-server/lib/cli.mjs");
    std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
    std::fs::write(&cli, "").unwrap();
    std::fs::create_dir_all(modules.join("typescript/lib")).unwrap();
    std::fs::write(modules.join("typescript/lib/tsserver.js"), "").unwrap();
    let program = base.join("prefix/bin/typescript-language-server");
    std::fs::create_dir_all(program.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&cli, &program).unwrap();
    let options = server_for("typescript").unwrap().initialization_options;

    let bare = base.join("bare");
    std::fs::create_dir_all(&bare).unwrap();
    let lib = std::fs::canonicalize(modules.join("typescript/lib")).unwrap();
    assert_eq!(options(&program, &bare), Some(json!({ "tsserver": { "path": lib } })), "no TypeScript of its own");

    let project = base.join("project");
    std::fs::create_dir_all(project.join("node_modules/typescript/lib")).unwrap();
    std::fs::write(project.join("node_modules/typescript/lib/tsserver.js"), "").unwrap();
    assert_eq!(options(&program, &project), None, "a project's own TypeScript wins");
    std::fs::remove_dir_all(&base).ok();
}
