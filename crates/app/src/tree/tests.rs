use super::*;

fn entry(path: &str, dir: bool) -> Entry {
    Entry { path: path.into(), dir }
}

fn tree() -> ProjectTree {
    ProjectTree::new(vec![
        entry("Cargo.toml", false),
        entry("README.md", false),
        entry("crates", true),
        entry("crates/app", true),
        entry("crates/app/main.rs", false),
        entry("crates/beui", true),
        entry("docs", true),
        entry("docs/app.md", false),
        entry(".gitignore", false),
        entry("build.rs", false),
    ])
}

fn shown(rows: &[Row]) -> Vec<String> {
    rows.iter().map(|r| format!("{}{}{}", "  ".repeat(r.depth), r.name, if r.dir { "/" } else { "" })).collect()
}

#[test]
fn folders_come_first_then_files_by_name_whatever_the_case() {
    assert_eq!(shown(&tree().rows(&HashSet::new())), ["crates/", "docs/", ".gitignore", "build.rs", "Cargo.toml", "README.md"]);
}

#[test]
fn an_open_folder_shows_its_children_one_level_in() {
    let open: HashSet<String> = ["crates".to_string(), "crates/app".to_string()].into();
    assert_eq!(
        shown(&tree().rows(&open)),
        ["crates/", "  app/", "    main.rs", "  beui/", "docs/", ".gitignore", "build.rs", "Cargo.toml", "README.md"]
    );
    // A folder inside a closed one stays hidden even when it is open itself.
    let open: HashSet<String> = ["crates/app".to_string()].into();
    assert_eq!(tree().rows(&open).len(), 6);
}

#[test]
fn the_ancestors_of_a_path_open_it() {
    assert_eq!(ancestors("crates/app/main.rs"), ["crates/app", "crates"]);
    assert!(ancestors("README.md").is_empty());
}

#[test]
fn an_empty_folder_is_an_empty_tree() {
    let t = ProjectTree::new(Vec::new());
    assert!(t.is_empty());
    assert!(t.rows(&HashSet::new()).is_empty());
    assert_eq!(tree().files(), 6);
}
