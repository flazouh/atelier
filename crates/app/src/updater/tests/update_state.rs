use super::super::{Reaction, UP_TO_DATE_NOTICE, UpdateEvent, UpdateState};

fn found(user: bool) -> UpdateEvent {
    UpdateEvent::Found { version: "0.2.0".into(), notes: Some("- new".into()), user }
}

/// Runs `events` from `Idle` and gives the end state and every reaction on the way.
fn run(events: Vec<UpdateEvent>) -> (UpdateState, Vec<Reaction>) {
    events.into_iter().fold((UpdateState::Idle, Vec::new()), |(state, mut said), event| {
        let (next, reaction) = state.apply(event);
        said.push(reaction);
        (next, said)
    })
}

#[test]
fn the_daily_look_downloads_in_silence_and_waits_for_the_reader() {
    let (state, said) = run(vec![
        UpdateEvent::Checking { user: false },
        found(false),
        UpdateEvent::Downloading { fraction: 0.5 },
        UpdateEvent::Extracting { fraction: 1. },
        UpdateEvent::Ready,
    ]);
    assert_eq!(state, UpdateState::Ready { version: "0.2.0".into(), notes: "- new".into() });
    assert!(said.iter().all(|r| *r == Reaction::Nothing), "nothing opens by itself: {said:?}");
}

#[test]
fn a_look_the_reader_asked_for_opens_the_update_when_it_is_ready() {
    let (state, said) = run(vec![UpdateEvent::Checking { user: true }, found(true), UpdateEvent::Ready]);
    assert!(matches!(state, UpdateState::Ready { .. }));
    assert_eq!(said.last(), Some(&Reaction::Show));
}

#[test]
fn a_look_the_reader_asked_for_says_when_there_is_nothing_newer_and_the_daily_look_does_not() {
    let (state, said) = run(vec![UpdateEvent::Checking { user: true }, UpdateEvent::UpToDate]);
    assert_eq!((state, said.last().cloned()), (UpdateState::Idle, Some(Reaction::Say(UP_TO_DATE_NOTICE.into()))));
    let (state, said) = run(vec![UpdateEvent::Checking { user: false }, UpdateEvent::UpToDate]);
    assert_eq!((state, said.last().cloned()), (UpdateState::Idle, Some(Reaction::Nothing)));
}

/// The share of the work done, as the chip shows it.
fn fraction(state: &UpdateState) -> Option<f64> {
    match state {
        UpdateState::Downloading { fraction, .. } => Some(*fraction),
        _ => None,
    }
}

#[test]
fn progress_runs_from_the_download_through_the_unpacking() {
    let (state, _) = run(vec![UpdateEvent::Checking { user: false }, found(false), UpdateEvent::Downloading { fraction: 0.5 }]);
    assert_eq!(fraction(&state), Some(0.45));
    let (state, _) = run(vec![UpdateEvent::Checking { user: false }, found(false), UpdateEvent::Extracting { fraction: 0.5 }]);
    assert_eq!(fraction(&state), Some(0.95));
    let (state, _) = run(vec![UpdateEvent::Checking { user: false }, found(false), UpdateEvent::Downloading { fraction: 7. }]);
    assert_eq!(fraction(&state), Some(0.9), "a fraction past 1 stops at the end of the download");
}

#[test]
fn a_failure_is_told_only_when_the_reader_asked_or_the_install_began() {
    let fail = || UpdateEvent::Failed { message: "no network".into() };
    let (_, said) = run(vec![UpdateEvent::Checking { user: true }, fail()]);
    assert_eq!(said.last(), Some(&Reaction::Say("Could not update: no network".into())));
    let (state, said) = run(vec![UpdateEvent::Checking { user: false }, fail()]);
    assert_eq!((state, said.last().cloned()), (UpdateState::Idle, Some(Reaction::Nothing)), "the daily look fails in silence");
    let (_, said) = run(vec![UpdateEvent::Checking { user: false }, found(false), UpdateEvent::Ready, UpdateEvent::Installing, fail()]);
    assert_eq!(said.last(), Some(&Reaction::Say("Could not update: no network".into())), "an install that fails is told");
}

#[test]
fn a_second_look_or_a_second_find_does_not_restart_what_goes_on() {
    let (state, _) = run(vec![UpdateEvent::Checking { user: false }, found(false), UpdateEvent::Downloading { fraction: 0.2 }]);
    let (again, _) = state.clone().apply(UpdateEvent::Checking { user: true });
    assert_eq!(again, state);
    let (again, _) = state.clone().apply(found(true));
    assert_eq!(again, state);
}

#[test]
fn the_updater_putting_a_ready_update_in_front_opens_it_and_nothing_else_does() {
    let (ready, _) = run(vec![UpdateEvent::Checking { user: false }, found(false), UpdateEvent::Ready]);
    assert_eq!(ready.clone().apply(UpdateEvent::Focus).1, Reaction::Show);
    assert_eq!(UpdateState::Idle.apply(UpdateEvent::Focus).1, Reaction::Nothing);
}

#[test]
fn the_installing_and_the_end_of_an_update_settle_the_state() {
    let (ready, _) = run(vec![UpdateEvent::Checking { user: false }, found(false), UpdateEvent::Ready]);
    assert_eq!(ready.clone().apply(UpdateEvent::Installing).0, UpdateState::Installing);
    assert_eq!(ready.apply(UpdateEvent::Idle).0, UpdateState::Idle);
}

#[test]
fn an_event_line_from_the_native_side_reads() {
    let line = r#"{"kind":"found","version":"0.2.0","notes":"- new","user":true}"#;
    assert_eq!(serde_json::from_str::<UpdateEvent>(line).unwrap(), found(true));
    assert_eq!(serde_json::from_str::<UpdateEvent>(r#"{"kind":"up_to_date"}"#).unwrap(), UpdateEvent::UpToDate);
    assert_eq!(serde_json::from_str::<UpdateEvent>(r#"{"kind":"found","version":"1","user":false}"#).unwrap(), UpdateEvent::Found { version: "1".into(), notes: None, user: false });
}
