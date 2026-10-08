use super::super::{Remembered, UpdateState, remember_on_ready, remembered};

#[test]
fn the_changelog_is_shown_once_when_its_version_starts() {
    assert_eq!(remembered("0.1.4", "0.1.4"), Remembered::Show);
}

#[test]
fn a_changelog_for_a_version_not_installed_yet_is_kept_and_an_older_one_is_dropped() {
    assert_eq!(remembered("0.1.4", "0.1.3"), Remembered::Wait, "Later: it installs at quit, and the next start shows it");
    assert_eq!(remembered("0.1.4", "0.1.10"), Remembered::Forget, "numbers, not letters: 0.1.10 is after 0.1.4");
    assert_eq!(remembered("0.1.3", "0.2.0"), Remembered::Forget);
    assert_eq!(remembered("soon", "0.1.3"), Remembered::Forget);
    assert_eq!(remembered("0.1.4", "dev"), Remembered::Forget);
}

#[test]
fn only_the_move_to_ready_keeps_a_changelog() {
    let downloading = UpdateState::Downloading { version: "0.1.4".into(), notes: "n".into(), fraction: 0.5, asked: false };
    let ready = UpdateState::Ready { version: "0.1.4".into(), notes: "- **A:** b".into() };
    let kept = remember_on_ready(&downloading, &ready).expect("it is ready");
    assert_eq!((kept.version.as_str(), kept.notes.as_str()), ("0.1.4", "- **A:** b"));
    assert!(remember_on_ready(&ready, &ready).is_none(), "again ready: nothing new");
    assert!(remember_on_ready(&UpdateState::Idle, &downloading).is_none());
    assert!(remember_on_ready(&UpdateState::Idle, &UpdateState::Ready { version: String::new(), notes: String::new() }).is_none(), "no version, no record");
}
