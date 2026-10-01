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
