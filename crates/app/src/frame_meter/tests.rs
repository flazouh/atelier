use super::{Mode, mode};

#[test]
fn lathe_frames_is_off_reports_or_each() {
    assert_eq!(mode(None), Mode::Off);
    assert_eq!(mode(Some("0")), Mode::Off);
    assert_eq!(mode(Some("1")), Mode::Reports);
    assert_eq!(mode(Some("each")), Mode::Each);
}
