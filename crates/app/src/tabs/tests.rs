use super::*;

#[test]
fn a_new_tab_opens_after_the_one_showing_and_an_open_one_is_shown_again() {
    let mut t = Tabs::default();
    assert!(t.open("a"));
    assert!(t.open("b"));
    assert!(t.open("c"));
    assert!(!t.open("a"), "a has a tab already");
    assert_eq!(t.active(), Some("a"));
    t.open("d");
    assert_eq!(t.paths(), ["a", "d", "b", "c"]);
    assert_eq!(t.active(), Some("d"));
}

#[test]
fn closing_the_tab_showing_shows_its_right_neighbour_or_else_its_left() {
    let mut t = Tabs::default();
    for p in ["a", "b", "c"] {
        t.open(p);
    }
    t.open("b");
    t.close("b");
    assert_eq!(t.active(), Some("c"));
    t.close("c");
    assert_eq!(t.active(), Some("a"));
    t.close("a");
    assert_eq!(t.active(), None);
    t.close("missing");
}

#[test]
fn closing_another_tab_keeps_the_one_showing() {
    let mut t = Tabs::default();
    for p in ["a", "b", "c"] {
        t.open(p);
    }
    t.close("a");
    assert_eq!(t.active(), Some("c"));
    assert_eq!(t.paths(), ["b", "c"]);
}
