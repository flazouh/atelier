use crate::layout::{DIFF_MIN, Fit, NARROW, Part, RAIL_MAX, RAIL_SHARE, fit};

#[test]
fn a_wide_pane_shows_the_rail_at_full_width_and_the_tree() {
    assert_eq!(fit(1600., true, None, Part::Details), Fit { rail: Some(RAIL_MAX), tree: true, single: None });
}

#[test]
fn the_rail_is_at_most_two_fifths_of_the_pane_and_never_over_its_widest() {
    for width in [700., 800., 1000., 1400., 2400.] {
        let f = fit(width, true, None, Part::Details);
        let rail = f.rail.unwrap();
        assert!(rail <= RAIL_SHARE * width + 0.01 && rail <= RAIL_MAX, "{width}: the rail is {rail}");
        assert_eq!(f.single, None);
    }
}

#[test]
fn a_narrower_pane_folds_the_tree_first() {
    let f = fit(900., true, None, Part::Details);
    assert!(!f.tree, "{f:?}");
    assert!(fit(1600., true, None, Part::Details).tree);
}

#[test]
fn the_reader_can_force_the_tree_and_the_rail_gives_way() {
    let f = fit(1100., true, Some(true), Part::Details);
    assert!(f.tree);
    assert!(f.rail.unwrap() <= RAIL_MAX);
    assert!(!fit(2000., true, Some(false), Part::Details).tree, "and turn it off in a wide pane");
}

#[test]
fn a_folded_rail_leaves_no_rail_and_the_diff_keeps_its_least() {
    let f = fit(1000., false, None, Part::Details);
    assert_eq!(f.rail, None);
    assert!(1000. - 16. - if f.tree { 248. } else { 0. } >= DIFF_MIN);
}

/// Below 700 px the view shows one of its two parts at a time: the rail (Details) or the diff (Files).
#[test]
fn under_700_px_one_part_shows_at_a_time() {
    for width in [450., 550., 699.] {
        let details = fit(width, true, None, Part::Details);
        assert_eq!(details.single, Some(Part::Details), "{width}");
        assert_eq!(details.rail, Some(width - 16.), "the rail takes the pane");
        assert!(!details.tree);
        let files = fit(width, true, None, Part::Files);
        assert_eq!(files.single, Some(Part::Files));
        assert_eq!(files.rail, None, "the diff takes the pane");
        assert!(!files.tree);
    }
    assert_eq!(fit(NARROW, true, None, Part::Files).single, None, "from 700 up both show");
}
