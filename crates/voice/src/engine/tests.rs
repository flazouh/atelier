use super::helpers::{join, pause_end, verdict};

const ONE_SECOND: usize = 16_000;

#[test]
fn words_are_passed_on() {
    assert_eq!(verdict(&[0.1; ONE_SECOND], "Hello.".into()), Ok("Hello.".into()));
}

#[test]
fn a_tap_is_too_short() {
    assert!(verdict(&[0.1; 800], String::new()).unwrap_err().starts_with("Too short"));
}

#[test]
fn exact_silence_says_the_microphone_gave_no_sound() {
    assert!(verdict(&[0.; ONE_SECOND], "  ".into()).unwrap_err().starts_with("No sound"));
}

#[test]
fn a_quiet_room_is_sound_so_it_is_not_blamed_on_the_microphone() {
    assert!(verdict(&[0.001; ONE_SECOND], String::new()).unwrap_err().starts_with("No words"));
}

/// A voice: a tone at speaking level, for `seconds`.
fn voice(seconds: f32) -> Vec<f32> {
    (0..(seconds * ONE_SECOND as f32) as usize).map(|i| 0.1 * (i as f32 * 0.05).sin()).collect()
}

fn room(seconds: f32, level: f32) -> Vec<f32> {
    (0..(seconds * ONE_SECOND as f32) as usize).map(|i| level * if i % 2 == 0 { 1. } else { -1. }).collect()
}

#[test]
fn a_pause_after_a_sentence_is_a_cut() {
    let samples = [voice(1.5), room(0.3, 0.001), voice(1.)].concat();
    let cut = pause_end(&samples).expect("a cut");
    let seconds = cut as f32 / ONE_SECOND as f32;
    assert!((1.55..1.7).contains(&seconds), "inside the pause, quiet on both sides: {seconds}");
}

#[test]
fn a_breath_between_words_is_no_cut() {
    assert_eq!(pause_end(&[voice(1.5), room(0.1, 0.001), voice(1.)].concat()), None);
}

#[test]
fn a_pause_too_soon_waits_for_more_speech() {
    assert_eq!(pause_end(&[voice(0.4), room(0.3, 0.0), voice(0.2)].concat()), None, "under a second is not a stretch yet");
}

#[test]
fn a_noisy_room_still_shows_its_pauses() {
    let samples = [voice(1.5), room(0.4, 0.004), voice(1.)].concat();
    assert!(pause_end(&samples).is_some(), "the line rises with the room");
}

#[test]
fn speech_with_no_pause_is_not_cut() {
    assert_eq!(pause_end(&voice(4.)), None);
}

#[test]
fn stretches_join_into_one_text_and_their_seams_heal() {
    let parts = ["Then add a test for that case.".to_string(), "and one for a blank line.".into(), "".into(), "After that, run it.".into()];
    assert_eq!(join(&parts), "Then add a test for that case, and one for a blank line. After that, run it.");
    assert_eq!(join(&["Is it done?".into(), "and then?".into()]), "Is it done? and then?");
}
