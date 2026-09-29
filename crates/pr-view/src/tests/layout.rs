use crate::layout::{DIFF_MIN, Fit, RAIL_MAX, RAIL_MIN, fit};

#[test]
fn a_wide_pane_shows_the_rail_at_full_width_and_the_tree() {
    assert_eq!(fit(1600., true, None), Fit { rail: Some(RAIL_MAX), tree: true });
}

#[test]
fn a_narrower_pane_folds_the_tree_first_and_then_narrows_the_rail() {
    let f = fit(900., true, None);
    assert!(!f.tree, "{f:?}");
    let rail = f.rail.unwrap();
    assert!((RAIL_MIN..=RAIL_MAX).contains(&rail));
    let narrow = fit(760., true, None);
    assert_eq!(narrow.rail, Some(RAIL_MIN), "the rail never goes under its least");
}

#[test]
fn the_reader_can_force_the_tree_and_the_rail_gives_way() {
    let f = fit(1100., true, Some(true));
    assert!(f.tree);
    assert!(f.rail.unwrap() < RAIL_MAX);
    assert!(!fit(2000., true, Some(false)).tree, "and turn it off in a wide pane");
}

#[test]
fn a_folded_rail_leaves_no_rail_and_the_diff_keeps_its_least() {
    let f = fit(1000., false, None);
    assert_eq!(f.rail, None);
    assert!(1000. - 16. - if f.tree { 248. } else { 0. } >= DIFF_MIN);
}
