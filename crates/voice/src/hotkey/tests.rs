use std::time::{Duration, Instant};

use super::*;

fn ms(start: Instant, n: u64) -> Instant {
    start + Duration::from_millis(n)
}

#[test]
fn held_down_it_talks_and_let_go_it_stops() {
    let (mut t, at) = (Tracker::default(), Instant::now());
    assert_eq!(t.feed(Input::Down, at), Some(Action::Press));
    assert_eq!(t.feed(Input::Up, ms(at, 900)), Some(Action::Release));
}

#[test]
fn a_tap_keeps_it_open_and_the_next_tap_stops_it() {
    let (mut t, at) = (Tracker::default(), Instant::now());
    assert_eq!(t.feed(Input::Down, at), Some(Action::Press));
    assert_eq!(t.feed(Input::Up, ms(at, 80)), None, "a tap: hands-free");
    assert_eq!(t.feed(Input::Other, ms(at, 500)), None, "typing meanwhile does not stop it");
    assert_eq!(t.feed(Input::Down, ms(at, 3000)), Some(Action::Release));
    assert_eq!(t.feed(Input::Up, ms(at, 3080)), None, "the stopping tap's release means nothing");
    assert_eq!(t.feed(Input::Down, ms(at, 5000)), Some(Action::Press), "and the next press starts again");
}

#[test]
fn a_shortcut_takes_the_press_back() {
    let (mut t, at) = (Tracker::default(), Instant::now());
    assert_eq!(t.feed(Input::Down, at), Some(Action::Press));
    assert_eq!(t.feed(Input::Other, ms(at, 60)), Some(Action::Cancel), "Option and an arrow: a word jump");
    assert_eq!(t.feed(Input::Up, ms(at, 120)), None);
    assert_eq!(t.feed(Input::Down, ms(at, 1000)), Some(Action::Press), "it does not lock");
}

#[test]
fn a_key_well_after_the_press_is_part_of_talking() {
    let (mut t, at) = (Tracker::default(), Instant::now());
    t.feed(Input::Down, at);
    assert_eq!(t.feed(Input::Other, ms(at, 700)), None);
    assert_eq!(t.feed(Input::Up, ms(at, 1500)), Some(Action::Release));
}

#[test]
fn losing_focus_while_held_ends_the_press() {
    let (mut t, at) = (Tracker::default(), Instant::now());
    t.feed(Input::Down, at);
    assert_eq!(t.feed(Input::Away, ms(at, 1200)), Some(Action::Release));
    assert_eq!(t.feed(Input::Up, ms(at, 1300)), None, "a release that does come later means nothing");
}

#[test]
fn losing_focus_right_after_the_press_takes_it_back() {
    let (mut t, at) = (Tracker::default(), Instant::now());
    t.feed(Input::Down, at);
    assert_eq!(t.feed(Input::Away, ms(at, 50)), Some(Action::Cancel), "Fn and Tab, say, moved to another app");
}

#[test]
fn losing_focus_hands_free_ends_it() {
    let (mut t, at) = (Tracker::default(), Instant::now());
    t.feed(Input::Down, at);
    t.feed(Input::Up, ms(at, 80));
    assert_eq!(t.feed(Input::Away, ms(at, 4000)), Some(Action::Release));
    assert_eq!(t.feed(Input::Away, ms(at, 4100)), None, "once");
}

#[test]
fn a_repeated_down_is_one_press() {
    let (mut t, at) = (Tracker::default(), Instant::now());
    assert_eq!(t.feed(Input::Down, at), Some(Action::Press));
    assert_eq!(t.feed(Input::Down, ms(at, 300)), None);
}

#[test]
fn the_keys_are_named_as_the_settings_file_names_them() {
    assert_eq!("fn".parse(), Ok(Key::Fn));
    assert_eq!("right-option".parse(), Ok(Key::RightOption));
    assert_eq!("left-option".parse(), Ok(Key::LeftOption));
    assert_eq!("caps-lock".parse::<Key>(), Err(()));
    assert_eq!(Key::default(), Key::Fn);
    for key in Key::ALL {
        assert_eq!(key.name().parse(), Ok(key));
    }
}
