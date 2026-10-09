use super::{notes_of, releases};
use crate::updater::release_notes;

fn numbers(version: &str) -> Vec<u32> {
    version.split('.').map(|part| part.parse().unwrap()).collect()
}

#[test]
fn the_notes_of_the_version_that_runs_are_built_in_and_newest_comes_first() {
    let all = releases();
    assert!(!all.is_empty());
    assert!(all.windows(2).all(|pair| numbers(pair[0].0) > numbers(pair[1].0)), "newest first, by number: {:?}", all.iter().map(|r| r.0).collect::<Vec<_>>());
    assert!(notes_of(env!("CARGO_PKG_VERSION")).is_some(), "a release is not made without its notes");
    assert!(notes_of("0.0.0-none").is_none());
}

#[test]
fn every_release_has_notes_the_sheet_can_list() {
    for (version, markdown) in releases() {
        assert!(!release_notes(markdown).is_empty(), "{version} has no bullet with a lead");
    }
}

#[test]
fn every_release_has_a_date_the_sheet_can_show() {
    for (version, markdown) in releases() {
        assert!(crate::updater::release_date(markdown).is_some(), "{version} has no `Released: YYYY-MM-DD` line right under its heading");
    }
}
