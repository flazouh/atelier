use super::*;

#[test]
fn the_default_agent_comes_first_and_is_found_by_its_backend() {
    let all = agents();
    assert!(!all.is_empty());
    let first = &all[0];
    let found = by_backend(first.backend.name()).expect("found by its backend's name");
    assert_eq!(found.name, first.name);
    assert!(by_backend("no-such-backend").is_none());
    assert!(first.lab.mark().is_some(), "its models wear a lab mark");
}
