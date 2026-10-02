use super::*;

fn device(id: &str, label: &str, is_default: bool) -> Device {
    Device { id: id.into(), label: label.into(), is_default }
}

fn found() -> Vec<Device> {
    vec![device("a", "MacBook Pro Microphone", true), device("b", "BlackHole 2ch", false)]
}

#[test]
fn the_first_row_is_the_default_named_after_the_microphone_it_is() {
    let (rows, selected) = device_rows(&found(), None);
    assert_eq!(rows.len(), 3);
    assert_eq!((rows[0].id.as_ref(), rows[0].label.as_ref()), (DEFAULT_ID, "Default - MacBook Pro Microphone"));
    assert_eq!(selected, DEFAULT_ID);
}

#[test]
fn a_microphone_that_is_plugged_in_stays_chosen() {
    assert_eq!(device_rows(&found(), Some("b")).1, "b");
}

#[test]
fn a_microphone_that_is_gone_falls_back_to_the_default_row() {
    assert_eq!(device_rows(&found(), Some("unplugged")).1, DEFAULT_ID);
}

#[test]
fn with_no_microphone_there_is_still_a_default_row() {
    let (rows, selected) = device_rows(&[], None);
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].label.as_ref(), selected.as_str()), ("Default", DEFAULT_ID));
}

#[test]
fn the_settings_file_names_the_key_and_leaves_the_default_out() {
    use atelier_voice::hotkey::Key;
    assert_eq!(key_from(None), Some(Key::Fn));
    assert_eq!(key_from(Some("off")), None);
    assert_eq!(key_from(Some("caps-lock")), Some(Key::Fn), "a name this build does not know is the default");
    for key in [None, Some(Key::Fn), Some(Key::RightOption), Some(Key::LeftOption)] {
        assert_eq!(key_from(key_name(key).as_deref()), key);
    }
    assert_eq!(key_name(Some(Key::Fn)), None);
}

#[test]
fn every_event_about_a_press_reaches_the_session_that_made_it() {
    let about_press = [
        Event::Listening(7),
        Event::Level(7, 0.5),
        Event::Partial(7, "so far".into()),
        Event::Waiting(7),
        Event::Transcribing(7),
        Event::Transcript(7, "done".into()),
        Event::Failed(7, "why".into()),
        Event::Cancelled(7),
    ];
    for event in &about_press {
        assert_eq!(press_of(event), Some(7), "{event:?}");
    }
    assert_eq!(press_of(&Event::Ready), None);
}

#[test]
fn recovered_words_go_back_to_their_session_or_else_the_first() {
    assert_eq!(recovered_home(&["a", "b"], "b"), Some(1));
    assert_eq!(recovered_home(&["a", "b"], "gone"), Some(0));
    assert_eq!(recovered_home(&[], "b"), None);
}
