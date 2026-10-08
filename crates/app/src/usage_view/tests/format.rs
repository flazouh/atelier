use atelier_agents::usage_history::Day;
use crate::usage_view::helpers::{format_cost, format_tokens, weekday};
#[test]
fn tokens_are_short() {
    assert_eq!(format_tokens(812), "812");
    assert_eq!(format_tokens(12_300), "12.3 K");
    assert_eq!(format_tokens(5_840_000), "5.84 M");
    assert_eq!(format_tokens(1_200_000_000), "1.20 B");
}
#[test]
fn a_cost_is_in_dollars_and_a_missing_price_is_a_dash() {
    assert_eq!(format_cost(Some(14.2)), "$14.20");
    assert_eq!(format_cost(Some(0.004)), "<$0.01");
    assert_eq!(format_cost(None), "–");
}
#[test]
fn the_weekday_follows_the_calendar() {
    assert_eq!(weekday(Day::new(1970, 1, 1)), "Thu");
    assert_eq!(weekday(Day::new(2026, 10, 9)), "Fri");
    assert_eq!(weekday(Day::new(2024, 2, 29)), "Thu");
}
