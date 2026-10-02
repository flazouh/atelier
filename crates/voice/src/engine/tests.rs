use super::helpers::verdict;

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
