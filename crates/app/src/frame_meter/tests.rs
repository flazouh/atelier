use std::time::Duration;

use super::{Mode, each_line, mode};

#[test]
fn lathe_frames_is_off_reports_or_each() {
    assert_eq!(mode(None), Mode::Off);
    assert_eq!(mode(Some("0")), Mode::Off);
    assert_eq!(mode(Some("1")), Mode::Reports);
    assert_eq!(mode(Some("each")), Mode::Each);
}

/// Each frame's line gives the whole frame, then each named part in the order it was drawn.
#[test]
fn a_frame_line_names_each_part() {
    let parts = [("sidebar", Duration::from_micros(1_250)), ("panels", Duration::from_micros(8_000))];
    assert_eq!(each_line(1000, Duration::from_micros(11_600), &parts), "frame 1000 11.60 sidebar=1.25 panels=8.00");
    assert_eq!(each_line(7, Duration::from_micros(500), &[]), "frame 7 0.50");
}
