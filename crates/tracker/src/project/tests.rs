use std::path::Path;

use super::{ProjectKey, prefix_for};

fn local(path: &str) -> ProjectKey {
    ProjectKey::Local { path: path.into() }
}

fn ssh(host: &str, path: &str) -> ProjectKey {
    ProjectKey::Ssh { host: host.into(), path: path.into() }
}

#[test]
fn the_prefix_is_the_first_three_letters_in_capitals() {
    assert_eq!(prefix_for("lathe"), "LAT");
    assert_eq!(prefix_for("api-server"), "API");
    assert_eq!(prefix_for("x"), "X");
    assert_eq!(prefix_for("1st app"), "1ST");
    assert_eq!(prefix_for("---"), "TSK", "a name with no letters gets a default");
    assert_eq!(prefix_for(""), "TSK");
}

#[test]
fn one_project_gives_one_file_and_two_projects_give_two() {
    assert_eq!(local("/a/lathe").file_name(), local("/a/lathe/").file_name(), "a trailing slash is the same project");
    assert_ne!(local("/a/lathe").file_name(), local("/b/lathe").file_name(), "same folder name, different projects");
    assert_ne!(local("/a/lathe").file_name(), ssh("hp", "/a/lathe").file_name(), "over SSH is another project");
    assert_ne!(ssh("hp", "/a/lathe").file_name(), ssh("build", "/a/lathe").file_name());
    assert_eq!(ssh("HP", "/a/lathe").file_name(), ssh("hp", "/a/lathe").file_name(), "host names ignore case");
}

#[test]
fn the_file_name_is_stable_and_readable() {
    // A fixed value: the name must not change between runs or versions, or a project loses its tasks.
    assert_eq!(local("/Users/alex/code/lathe").file_name(), "lathe-39f6b39302e287f7.sqlite");
    assert!(local("/x/my project!").file_name().starts_with("my-project--"));
}

#[test]
fn the_database_is_in_the_data_folder() {
    let path = local("/Users/alex/code/lathe").path_in(Path::new("/data/lathe"));
    assert!(path.starts_with("/data/lathe/tracker"));
    assert!(path.extension().is_some_and(|e| e == "sqlite"));
}

#[test]
fn the_folder_is_the_last_part_of_the_path() {
    assert_eq!(local("/a/b/lathe").folder(), "lathe");
    assert_eq!(ssh("hp", "/home/alex/code/lathe-2/").folder(), "lathe-2");
    assert_eq!(local("/").folder(), "project");
}
