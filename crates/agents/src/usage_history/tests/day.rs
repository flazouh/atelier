use crate::usage_history::Day;

#[test]
fn epoch_days_round_trip() {
    for (y, m, d, days) in [(1970, 1, 1, 0), (2000, 2, 29, 11_016), (2026, 9, 25, 20_721), (1969, 12, 31, -1)] {
        let day = Day::new(y, m, d);
        assert_eq!(day.epoch_days(), days);
        assert_eq!(Day::from_epoch_days(days), day);
    }
}

#[test]
fn plus_and_minus_cross_month_and_year() {
    assert_eq!(Day::new(2026, 3, 1).minus_days(1), Day::new(2026, 2, 28));
    assert_eq!(Day::new(2026, 12, 31).plus_days(1), Day::new(2027, 1, 1));
    assert!(Day::new(2026, 9, 30) < Day::new(2026, 10, 1));
}

#[test]
fn offset_moves_the_day_boundary() {
    let secs = 20_721 * 86_400 + 23 * 3600 + 30 * 60; // 2026-09-25 23:30 UTC
    assert_eq!(Day::from_epoch_secs(secs, 0), Day::new(2026, 9, 25));
    assert_eq!(Day::from_epoch_secs(secs, 2 * 3600), Day::new(2026, 9, 26));
    assert_eq!(Day::from_epoch_secs(secs, -5 * 3600), Day::new(2026, 9, 25));
    assert_eq!(Day::new(2026, 9, 26).start_secs(2 * 3600), 20_722 * 86_400 - 7200);
}
