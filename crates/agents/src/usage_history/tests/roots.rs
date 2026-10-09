use std::fs;

use crate::usage_history::{Provider, Roots};

#[test]
fn detect_finds_claude_folders_with_projects_and_codex() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    for dir in [".claude/projects", ".claude-work/projects", ".claude-empty", ".codex/sessions", ".other/projects"] {
        fs::create_dir_all(home.join(dir)).unwrap();
    }
    fs::write(home.join(".claude.json"), "{}").unwrap();
    let env_dir = tmp.path().join("elsewhere");
    fs::create_dir_all(&env_dir).unwrap();

    let roots = Roots::detect(home, Some(&env_dir));
    let found: Vec<_> = roots.accounts().iter().map(|a| (a.provider, a.label.as_str())).collect();
    assert_eq!(
        found,
        [
            (Provider::Claude, "claude"),
            (Provider::Claude, "claude-work"),
            (Provider::Codex, "codex"),
            (Provider::Claude, "elsewhere"),
        ]
    );
}

#[test]
fn a_folder_named_twice_is_kept_once() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join(".claude/projects")).unwrap();
    let roots = Roots::detect(tmp.path(), Some(&tmp.path().join(".claude")));
    assert_eq!(roots.accounts().len(), 1);
    assert!(Roots::detect(&tmp.path().join("nope"), None).accounts().is_empty());
}
