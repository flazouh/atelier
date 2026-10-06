use super::{bad_name, copy_name, joined, moved};

#[test]
fn a_name_has_no_slash_and_is_not_empty() {
    assert_eq!(bad_name(""), Some("Type a name."));
    assert!(bad_name("..").is_some() && bad_name(".").is_some());
    assert!(bad_name("a/b").is_some() && bad_name("a\\b").is_some());
    assert_eq!(bad_name(".gitignore"), None);
    assert_eq!(bad_name("my file.txt"), None);
}

#[test]
fn a_path_under_a_moved_folder_moves_with_it() {
    assert_eq!(moved("src", "src", "lib").as_deref(), Some("lib"));
    assert_eq!(moved("src/a/b.rs", "src", "lib").as_deref(), Some("lib/a/b.rs"));
    assert_eq!(moved("src2/b.rs", "src", "lib"), None, "a name that only starts the same is another folder");
    assert_eq!(moved("a.txt", "src", "lib"), None);
    assert_eq!(moved("src/a.rs", "src", "").as_deref(), Some("a.rs"));
    assert_eq!(joined("", "a"), "a");
    assert_eq!(joined("x/y", "a"), "x/y/a");
}

#[test]
fn a_copy_takes_the_first_free_name_beside_the_original() {
    let none = |_: &str| false;
    assert_eq!(copy_name(none, "a/b.txt"), "a/b copy.txt");
    assert_eq!(copy_name(none, "README"), "README copy");
    assert_eq!(copy_name(none, ".gitignore"), ".gitignore copy");
    assert_eq!(copy_name(none, "a.tar.gz"), "a.tar copy.gz");
    let taken = |p: &str| p == "b copy.txt" || p == "b copy 2.txt";
    assert_eq!(copy_name(taken, "b.txt"), "b copy 3.txt");
}
