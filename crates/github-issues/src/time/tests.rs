use super::{format, parse};

#[test]
fn a_timestamp_reads_to_milliseconds_and_back() {
    assert_eq!(parse("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse("2000-03-01T00:00:01Z"), Some(951_868_801_000));
    assert_eq!(parse("2026-10-09T01:13:42Z"), Some(1_791_508_422_000));
    for text in [
        "1969-12-31T23:59:59Z",
        "2024-02-29T12:00:00Z",
        "2026-10-09T01:13:42Z",
    ] {
        assert_eq!(format(parse(text).unwrap()), text);
    }
}

#[test]
fn text_that_is_not_a_utc_timestamp_is_none() {
    for text in [
        "",
        "2026-10-09",
        "2026-10-09T01:13:42+02:00",
        "2026-13-09T01:13:42Z",
        "2026-10-09T25:13:42Z",
        "20x6-10-09T01:13:42Z",
    ] {
        assert_eq!(parse(text), None, "{text:?}");
    }
}
