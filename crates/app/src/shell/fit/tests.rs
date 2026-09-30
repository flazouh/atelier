use super::*;
/// The side panes as a reader leaves them by default: the sidebar at 260, the right pane at 560.
const DEFAULT: Wants = Wants { sidebar: Some(SIDEBAR_DEFAULT), right: Some(RIGHT_DEFAULT) };
fn sum(w: &Widths) -> f32 {
    w.sidebar.unwrap_or(0.) + w.agent + w.right.unwrap_or(0.)
}
/// Which layout each width gets: one pane below 900, two to 1100, three from there.
#[test]
fn each_width_gets_its_layout() {
    assert_eq!(Fit::of(640.), Fit::Narrow);
    assert_eq!(Fit::of(899.), Fit::Narrow);
    assert_eq!(Fit::of(900.), Fit::Medium);
    assert_eq!(Fit::of(1099.), Fit::Medium);
    assert_eq!(Fit::of(1100.), Fit::Wide);
    assert_eq!(Fit::of(1440.), Fit::Wide);
}
/// At 1440 the panes keep the widths asked for, and the session column takes the rest.
#[test]
fn a_wide_window_keeps_the_widths_asked_for() {
    let w = widths(1440., DEFAULT);
    assert_eq!((w.sidebar, w.right), (Some(260.), Some(560.)));
    assert_eq!(w.agent, 620.);
}
/// At 1100 the right pane gives way first, so the session column keeps its least width.
#[test]
fn the_right_pane_gives_way_first() {
    let w = widths(1100., DEFAULT);
    assert_eq!(w.agent, AGENT_LEAST);
    assert_eq!(w.sidebar, Some(260.));
    assert_eq!(w.right, Some(520.));
    assert_eq!(sum(&w), 1100.);
}
/// At 900 with the sidebar asked for, the right pane stops at its least width and the sidebar gives
/// way next.
#[test]
fn the_sidebar_gives_way_next() {
    let w = widths(900., DEFAULT);
    assert_eq!(w.right, Some(RIGHT_LEAST));
    assert_eq!(w.sidebar, Some(260.));
    assert_eq!(w.agent, AGENT_LEAST);
    let w = widths(850., DEFAULT);
    assert_eq!((w.sidebar, w.agent, w.right), (Some(210.), AGENT_LEAST, Some(RIGHT_LEAST)));
}
/// When the sidebar cannot keep its least width, it hides rather than clips.
#[test]
fn a_pane_that_cannot_keep_its_least_width_hides() {
    let w = widths(760., DEFAULT);
    assert_eq!(w.sidebar, None);
    assert_eq!((w.agent, w.right), (440., Some(RIGHT_LEAST)));
    let w = widths(600., Wants { sidebar: None, right: Some(RIGHT_DEFAULT) });
    assert_eq!(w.right, None, "the session column keeps its least width");
    assert_eq!(w.agent, 600.);
}
/// No width from 640 up leaves a pane under its least width or the sum off the window.
#[test]
fn no_pane_is_ever_under_its_least_width() {
    for total in (640..=2000).step_by(10).map(|t| t as f32) {
        for wants in [DEFAULT, Wants { sidebar: None, ..DEFAULT }, Wants { right: None, ..DEFAULT }, Wants { sidebar: Some(480.), right: Some(2400.) }] {
            let w = widths(total, wants);
            assert!(w.agent >= AGENT_LEAST, "{total}: {w:?}");
            assert!(w.sidebar.is_none_or(|s| s >= SIDEBAR_LEAST), "{total}: {w:?}");
            assert!(w.right.is_none_or(|r| r >= RIGHT_LEAST), "{total}: {w:?}");
            assert!((sum(&w) - total).abs() < 0.01, "{total}: {w:?}");
        }
    }
}
