use super::{ago, parse};

#[test]
fn rfc3339_in_utc_becomes_epoch_seconds() {
    assert_eq!(parse("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse("2000-03-01T00:00:00Z"), Some(951_868_800));
    assert_eq!(parse("2026-09-29T15:14:16Z"), Some(1_790_694_856));
    assert_eq!(parse("2024-02-29T23:59:59Z"), Some(1_709_251_199), "a leap day");
}

#[test]
fn anything_else_is_none() {
    for text in ["", "2026-09-29", "2026-09-29T15:14:16", "2026-13-01T00:00:00Z", "2026-01-32T00:00:00Z", "x-y-zT1:2:3Z", "1969-12-31T00:00:00Z"] {
        assert_eq!(parse(text), None, "{text:?}");
    }
}

#[test]
fn ago_says_one_unit() {
    assert_eq!(ago(1000, 1000), "just now");
    assert_eq!(ago(1000, 2000), "just now", "a time in the future");
    assert_eq!(ago(1000 + 90, 1000), "1m ago");
    assert_eq!(ago(1000 + 7200, 1000), "2h ago");
    assert_eq!(ago(1000 + 3 * 86_400, 1000), "3d ago");
    assert_eq!(ago(1000 + 90 * 86_400, 1000), "3mo ago");
    assert_eq!(ago(1000 + 800 * 86_400, 1000), "2y ago");
}
