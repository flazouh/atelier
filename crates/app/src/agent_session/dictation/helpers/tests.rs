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
