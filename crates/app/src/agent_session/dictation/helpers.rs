use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use atelier_ui::{PromptInput, SetupPhase, VoiceDevice};
use atelier_voice::{Cue, Device, Engine, Event, Press, files};
use futures_channel::mpsc::unbounded;
use futures_util::StreamExt;
use gpui_kit::{App, AsyncApp, Context};

use atelier_voice::hotkey::{Action, Input, Key, Tracker};
use gpui_kit::{AnyWindowHandle, Entity, Focusable, Window};

use super::structs::{KeyRoute, Prefs, Recovery, Speech};
use super::types::{Owner, Presses};

/// The engine, started on first use.
pub(super) fn speech(cx: &mut App) -> (Rc<Engine>, Presses) {
    if let Some(speech) = cx.try_global::<Speech>() {
        return (speech.engine.clone(), speech.presses.clone());
    }
    let (tx, mut events) = unbounded::<Event>();
    // A test keeps no recording on this machine's disk, and takes up none.
    let kept = if cfg!(test) { None } else { atelier_voice::kept::dir() };
    let engine = Rc::new(Engine::spawn_keeping(
        move |event| {
            tx.unbounded_send(event).ok();
        },
        kept,
    ));
    let presses = Presses::default();
    let installed = files::dir().is_some_and(|dir| files::installed(&dir, &files::FILES));
    let phase = Rc::new(Cell::new((!engine.ready()).then_some(if installed { SetupPhase::Prepare } else { SetupPhase::Download(0.) })));
    let total_mb = Rc::new(Cell::new(files::total_bytes(&files::FILES) as f32 / 1e6));
    let recovery = Rc::new(RefCell::new(Recovery::default()));
    let (serving, at, size, back) = (presses.clone(), phase.clone(), total_mb.clone(), recovery.clone());
    App::spawn(cx, async move |cx| {
        while let Some(event) = events.next().await {
            dispatch(event, &serving, &back, &at, &size, cx);
        }
    })
    .detach();
    let saved = atelier_settings::path().map(|p| atelier_settings::load(&p)).unwrap_or_default();
    let prefs = Rc::new(RefCell::new(Prefs {
        device: saved.dictation_device,
        hold: saved.dictation_hold.unwrap_or(false),
        key: key_from(saved.dictation_key.as_deref()),
    }));
    cx.set_global(Speech { engine: engine.clone(), presses: presses.clone(), phase, total_mb, prefs, recovery });
    (engine, presses)
}

/// Hands one event to the session it is about: a press's to its owner, the setup's to every session whose words wait for it.
fn dispatch(event: Event, presses: &Presses, recovery: &Rc<RefCell<Recovery>>, phase: &Cell<Option<SetupPhase>>, total_mb: &Cell<f32>, cx: &mut AsyncApp) {
    if let Event::Recovered(press, tag) = event {
        recovery.borrow_mut().orphans.insert(press, tag);
        return;
    }
    if let Some(step) = setup_step(&event, total_mb) {
        phase.set(step.filter(|p| *p != SetupPhase::Ready));
        let owners: Vec<Owner> = presses.borrow().values().cloned().collect();
        for (session, window) in owners {
            let event = event.clone();
            window
                .update(cx, |_, window, cx| {
                    session.update(cx, |s, cx| s.dictation_setup(step, event, window, cx)).ok();
                })
                .ok();
        }
        return;
    }
    let Some(press) = press_of(&event) else { return };
    let owner = presses.borrow().get(&press).cloned();
    let Some((session, window)) = owner else {
        // A press from an earlier run: its words go to a session once they are out, and anything else about it is dropped.
        if ends(&event) {
            let tag = recovery.borrow_mut().orphans.remove(&press);
            if let (Some(tag), Event::Transcript(_, words)) = (tag, event) {
                recovery.borrow_mut().unclaimed.push((tag, words));
                let recovery = recovery.clone();
                cx.update(|cx| deliver(&recovery, cx));
            }
        }
        return;
    };
    if ends(&event) {
        presses.borrow_mut().remove(&press);
    }
    window
        .update(cx, |_, window, cx| {
            session.update(cx, |s, cx| s.dictation_event(event, window, cx)).ok();
        })
        .ok();
}

/// Puts words kept from an earlier run into a session: the one they were spoken in if it is open again, or else the first
/// open. They wait in its box, unsent. With no session open, they wait for one.
fn deliver(recovery: &Rc<RefCell<Recovery>>, cx: &mut App) {
    let (sessions, words) = {
        let mut r = recovery.borrow_mut();
        r.sessions.retain(|(_, s, _)| s.upgrade().is_some());
        if r.sessions.is_empty() {
            return;
        }
        (r.sessions.clone(), std::mem::take(&mut r.unclaimed))
    };
    for (tag, words) in words {
        let keys: Vec<&str> = sessions.iter().map(|(k, _, _)| k.as_ref()).collect();
        let Some(at) = recovered_home(&keys, &tag) else { continue };
        let (_, session, window) = &sessions[at];
        window
            .update(cx, |_, window, cx| {
                session.update(cx, |s, cx| s.composer.update(cx, |c, cx| c.insert_transcript(words.trim(), window, cx))).ok();
            })
            .ok();
    }
}

/// Which of the open sessions, by key, takes words spoken in the session `tag`: that one, or else the first.
pub(super) fn recovered_home(keys: &[&str], tag: &str) -> Option<usize> {
    keys.iter().position(|k| *k == tag).or((!keys.is_empty()).then_some(0))
}

/// Lets words kept from an earlier run reach the session `key` while it is open, and hands it any that are waiting.
pub fn take_recovered(key: gpui_kit::SharedString, session: gpui_kit::WeakEntity<super::super::AgentSession>, window: AnyWindowHandle, cx: &mut App) {
    speech(cx);
    let recovery = cx.global::<Speech>().recovery.clone();
    let waiting = {
        let mut r = recovery.borrow_mut();
        r.sessions.push((key, session, window));
        !r.unclaimed.is_empty()
    };
    // Deferred, since the session is still being built; and only when there are words, as a deferred update on every new
    // session shifts when its rows first draw.
    if waiting {
        cx.defer(move |cx| deliver(&recovery, cx));
    }
}

/// For an event about the model's setup, the step it shows (`None` inside: the setup failed); for any other, `None`. Keeps the
/// download's size up to date in `total_mb`.
pub(super) fn setup_step(event: &Event, total_mb: &Cell<f32>) -> Option<Option<SetupPhase>> {
    match event {
        Event::Download { done, total } => {
            total_mb.set(*total as f32 / 1e6);
            Some(Some(SetupPhase::Download(if *total == 0 { 0. } else { *done as f32 / *total as f32 })))
        }
        Event::Prepare => Some(Some(SetupPhase::Prepare)),
        Event::Ready => Some(Some(SetupPhase::Ready)),
        Event::SetupFailed(_) => Some(None),
        _ => None,
    }
}

pub(super) fn press_of(event: &Event) -> Option<Press> {
    match event {
        Event::Listening(p)
        | Event::Level(p, _)
        | Event::Partial(p, _)
        | Event::Waiting(p)
        | Event::Transcribing(p)
        | Event::Transcript(p, _)
        | Event::Failed(p, _)
        | Event::Cancelled(p) => Some(*p),
        _ => None,
    }
}

/// Whether the press is over after this event: nothing more will come about it.
pub(super) fn ends(event: &Event) -> bool {
    matches!(event, Event::Transcript(..) | Event::Failed(..) | Event::Cancelled(_))
}

/// Loads the model in the background if it is on this machine, so its first words come at once.
pub fn warm(cx: &mut App) {
    // A test never loads a model this machine happens to have: the load reports from a thread the test does not drive.
    if cfg!(test) {
        return;
    }
    speech(cx).0.warm();
}

/// Fetches the model if it is missing, then loads it: the person opened the microphone's menu, so they mean to dictate.
pub(super) fn fetch(cx: &mut App) {
    speech(cx).0.fetch();
}

/// The setup's step now and the download's size, for a press that ends before the model is ready.
pub(super) fn setup_now(cx: &mut App) -> (Option<SetupPhase>, f32) {
    speech(cx);
    let speech = cx.global::<Speech>();
    (speech.phase.get(), speech.total_mb.get())
}

pub(super) fn play(cue: &Option<Cue>) {
    if let Some(cue) = cue {
        cue.play();
    }
}

/// Gives a new composer what the person chose before: whether the microphone records while held.
pub fn start_up(input: &mut PromptInput, cx: &mut Context<PromptInput>) {
    let hold = prefs(cx).hold;
    input.set_voice_hold(hold, cx);
}

/// Lets the dictation key reach `composer`, in `window`, for as long as it lives. The first composer starts listening for the
/// key the person chose, whichever it is at the moment.
pub fn hear_key(composer: &Entity<PromptInput>, window: &mut Window, cx: &mut App) -> gpui_kit::Subscription {
    if cx.try_global::<KeyRoute>().is_none() {
        let mut route = KeyRoute::default();
        speech(cx);
        let prefs = cx.global::<Speech>().prefs.clone();
        let (tx, mut inputs) = unbounded::<(Input, std::time::Instant)>();
        let heard = tx.clone();
        if atelier_voice::hotkey::listen(move || prefs.borrow().key, move |input| {
            heard.unbounded_send((input, std::time::Instant::now())).ok();
        }) {
            route.away = Some(tx);
            App::spawn(cx, async move |cx| {
                let mut tracker = Tracker::default();
                while let Some((input, at)) = inputs.next().await {
                    if let Some(action) = tracker.feed(input, at) {
                        cx.update(|cx| act(action, cx));
                    }
                }
            })
            .detach();
        }
        cx.set_global(route);
    }
    let handle = window.window_handle();
    let route = cx.global_mut::<KeyRoute>();
    route.composers.retain(|(c, _)| c.upgrade().is_some());
    route.composers.push((composer.downgrade(), handle));
    let focus = composer.read(cx).focus_handle(cx);
    let weak = composer.downgrade();
    window.on_focus_in(&focus, cx, move |_, cx| cx.global_mut::<KeyRoute>().last = Some(weak.clone()))
}

/// The window lost focus: a press of the key may never see its release.
pub fn key_away(cx: &mut App) {
    if let Some(tx) = cx.try_global::<KeyRoute>().and_then(|r| r.away.clone()) {
        tx.unbounded_send((Input::Away, std::time::Instant::now())).ok();
    }
}

/// Does what the key asked: a press goes to the focused composer in the front window, or the one last focused there; its end
/// goes wherever the press went.
fn act(action: Action, cx: &mut App) {
    if action == Action::Press {
        let target = press_target(cx);
        cx.global_mut::<KeyRoute>().target = target.clone();
        let Some((composer, window)) = target.and_then(|c| c.upgrade()).zip(cx.active_window()) else { return };
        window.update(cx, |_, _, cx| composer.update(cx, |c, cx| c.press_mic(cx))).ok();
        return;
    }
    let Some(composer) = cx.global_mut::<KeyRoute>().target.take().and_then(|c| c.upgrade()) else { return };
    composer.update(cx, |c, cx| match action {
        Action::Cancel => c.cancel_mic(cx),
        _ => c.release_mic(cx),
    });
}

fn press_target(cx: &mut App) -> Option<gpui_kit::WeakEntity<PromptInput>> {
    let active: AnyWindowHandle = cx.active_window()?;
    let route = cx.global::<KeyRoute>();
    let here: Vec<_> = route.composers.iter().filter(|(_, w)| *w == active).map(|(c, _)| c.clone()).collect();
    let last = route.last.clone();
    let focused = active
        .update(cx, |_, window, cx| {
            here.iter().find(|c| c.upgrade().is_some_and(|c| c.read(cx).focus_handle(cx).contains_focused(window, cx))).cloned()
        })
        .ok()
        .flatten();
    focused.or_else(|| last.filter(|l| here.iter().any(|c| c == l) && l.upgrade().is_some()))
}

/// What the person chose, as of now.
pub fn prefs(cx: &mut App) -> Prefs {
    speech(cx);
    cx.global::<Speech>().prefs.borrow().clone()
}

/// Changes what the person chose, puts it in force in every composer, and writes it to the settings file, off the UI thread.
pub fn choose(cx: &mut App, change: impl FnOnce(&mut Prefs)) {
    speech(cx);
    let now = {
        let mut prefs = cx.global::<Speech>().prefs.borrow_mut();
        change(&mut prefs);
        prefs.clone()
    };
    let composers: Vec<_> = cx.try_global::<KeyRoute>().map(|r| r.composers.iter().filter_map(|(c, _)| c.upgrade()).collect()).unwrap_or_default();
    for composer in composers {
        composer.update(cx, |c, cx| c.set_voice_hold(now.hold, cx));
    }
    // A test writes only the file it names, never this machine's settings.
    if cfg!(test) && std::env::var_os("ATELIER_SETTINGS").is_none() {
        return;
    }
    let Some(path) = atelier_settings::path() else { return };
    cx.background_executor()
        .spawn(async move {
            if let Err(error) = atelier_settings::update(&path, |s| {
                s.dictation_device = now.device;
                s.dictation_hold = Some(now.hold).filter(|hold| *hold);
                s.dictation_key = key_name(now.key);
            }) {
                eprintln!("could not save the dictation settings: {error}");
            }
        })
        .detach();
}

/// The dictation key the settings file names: Fn when it names none, no key for `off`.
pub(super) fn key_from(saved: Option<&str>) -> Option<Key> {
    match saved {
        Some("off") => None,
        Some(name) => Some(name.parse().unwrap_or_default()),
        None => Some(Key::default()),
    }
}

/// How the settings file names the key: left out for the default.
pub(super) fn key_name(key: Option<Key>) -> Option<String> {
    match key {
        None => Some("off".into()),
        Some(Key::Fn) => None,
        Some(key) => Some(key.name().into()),
    }
}

/// The row that means "whichever microphone the system uses".
pub const DEFAULT_ID: &str = "default";

/// The menu's rows for the microphones found: the system's default first, named after the one it is now, then each microphone;
/// and which row is chosen. A choice that is no longer plugged in falls back to the default row.
pub fn device_rows(found: &[Device], chosen: Option<&str>) -> (Vec<VoiceDevice>, String) {
    let default = match found.iter().find(|d| d.is_default) {
        Some(d) => format!("Default - {}", d.label),
        None => "Default".to_string(),
    };
    let mut rows = vec![VoiceDevice::new(DEFAULT_ID, default)];
    rows.extend(found.iter().map(|d| VoiceDevice::new(d.id.clone(), d.label.clone())));
    let selected = chosen.filter(|id| found.iter().any(|d| d.id == *id)).unwrap_or(DEFAULT_ID);
    (rows, selected.to_string())
}

#[cfg(test)]
mod tests;
