use super::*;

#[test]
fn a_wide_pane_shows_the_tree_beside_a_full_rail() {
    assert_eq!(fit(1400., true, None), Fit { rail: Some(RAIL_MAX), tree: true });
}

#[test]
fn the_tree_folds_before_the_rail_narrows() {
    // Too narrow for the tree beside a full rail and the diff's least.
    let narrow = PADDING + RAIL_MAX + RAIL_GAP + TREE + TREE_GAP + DIFF_MIN - 1.;
    assert_eq!(fit(narrow, true, None), Fit { rail: Some(RAIL_MAX), tree: false });
    // Narrower still: the rail gives up width, down to its narrowest.
    let rail = fit(PADDING + RAIL_GAP + 340. + DIFF_MIN, true, None).rail.unwrap();
    assert!((rail - 340.).abs() < 0.01, "{rail}");
    assert_eq!(fit(600., true, None).rail, Some(RAIL_MIN));
}

#[test]
fn the_shortcut_brings_the_tree_back_and_the_rail_makes_room() {
    let narrow = PADDING + RAIL_MAX + RAIL_GAP + TREE + TREE_GAP + DIFF_MIN - 60.;
    let shown = fit(narrow, true, Some(true));
    assert!(shown.tree);
    assert!(shown.rail.unwrap() < RAIL_MAX, "the rail narrows to make room for the tree");
    assert!(!fit(1500., true, Some(false)).tree, "and a folded tree stays folded on a wide pane");
}

#[test]
fn a_folded_rail_leaves_the_width_to_the_files() {
    assert_eq!(fit(900., false, None), Fit { rail: None, tree: true });
}
