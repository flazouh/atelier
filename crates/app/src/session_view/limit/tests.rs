use atelier_agents::session::{Limit, LimitState, LimitWindow};

use super::helpers::{limit_words, resets_in};

const NOW: u64 = 1_790_000_000;

#[test]
fn the_reset_reads_in_the_largest_units_that_matter() {
    assert_eq!(resets_in(NOW - 5, NOW), "in a moment", "a reset already past");
    assert_eq!(resets_in(NOW + 30, NOW), "in a moment");
    assert_eq!(resets_in(NOW + 14 * 60, NOW), "in 14 min");
    assert_eq!(resets_in(NOW + 2 * 3600, NOW), "in 2 h");
    assert_eq!(resets_in(NOW + 2 * 3600 + 14 * 60 + 9, NOW), "in 2 h 14 min");
    assert_eq!(resets_in(NOW + 30 * 3600, NOW), "in a day");
    assert_eq!(resets_in(NOW + 3 * 86_400 + 5, NOW), "in 3 days");
}

#[test]
fn the_box_names_the_limit_and_when_it_resets() {
    let reached = |window, resets_at| Limit { state: LimitState::Reached, resets_at, window };
    assert_eq!(
        limit_words("Claude Code", &reached(Some(LimitWindow::FiveHour), Some(NOW + 45 * 60)), NOW),
        "Claude Code reached its 5-hour limit. It resets in 45 min."
    );
    assert_eq!(limit_words("Claude Code", &reached(Some(LimitWindow::Weekly), None), NOW), "Claude Code reached its weekly limit.");
    assert_eq!(limit_words("Claude Code", &reached(None, None), NOW), "Claude Code reached its usage limit.");
}
