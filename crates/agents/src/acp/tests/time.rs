use crate::acp::time::epoch_seconds;

#[test]
fn an_iso_time_reads_as_seconds_since_the_epoch() {
    assert_eq!(epoch_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(epoch_seconds("2026-10-01T18:51:00Z"), Some(1_790_880_660));
    assert_eq!(epoch_seconds("2026-10-01T18:51:00.734Z"), Some(1_790_880_660));
    assert_eq!(epoch_seconds("2026-10-01T20:51:00+02:00"), Some(1_790_880_660));
    assert_eq!(epoch_seconds("2026-10-01T13:51:00-0500"), Some(1_790_880_660));
    assert_eq!(epoch_seconds("2024-02-29T00:00:00Z"), Some(1_709_164_800));
}

#[test]
fn a_bare_number_reads_as_seconds_or_milliseconds() {
    assert_eq!(epoch_seconds("1790880660"), Some(1_790_880_660));
    assert_eq!(epoch_seconds("1790880660734"), Some(1_790_880_660));
}

#[test]
fn a_time_that_does_not_read_is_none() {
    for text in ["", "yesterday", "2026-13-01T00:00:00Z", "2026-10-01", "2026-10-01T25:00:00Z", "1960-01-01T00:00:00Z"] {
        assert_eq!(epoch_seconds(text), None, "{text}");
    }
}

#[test]
fn seconds_that_are_not_a_number_do_not_read() {
    for text in ["2026-10-01T00:00:NaNZ", "2026-10-01T00:00:infZ", "2026-10-01T00:00:-1Z"] {
        assert_eq!(epoch_seconds(text), None, "{text}");
    }
}

#[test]
fn a_day_or_an_offset_that_does_not_exist_does_not_read() {
    for text in [
        "2026-02-30T00:00:00Z",
        "2026-04-31T00:00:00Z",
        "2025-02-29T00:00:00Z",
        "2026-10-01T00:00:00+99:99",
        "2026-10-01T00:00:00+02:60",
        "2026-10-01T00:00:00+2",
    ] {
        assert_eq!(epoch_seconds(text), None, "{text}");
    }
}

#[test]
fn a_lowercase_separator_reads() {
    assert_eq!(epoch_seconds("2026-10-01t18:51:00z"), Some(1_790_880_660));
}
