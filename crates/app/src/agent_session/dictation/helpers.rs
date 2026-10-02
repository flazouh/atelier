use std::{cell::RefCell, rc::Rc};

use atelier_ui::VoiceDevice;
use atelier_voice::{Cue, Device, Engine, Event};
use futures_channel::mpsc::unbounded;
use futures_util::StreamExt;
use atelier_ui::PromptInput;
use gpui_kit::{App, Context};

use super::structs::{Prefs, Speech};
use super::types::Owner;

/// The engine, started on first use.
pub(super) fn speech(cx: &mut App) -> (Rc<Engine>, Owner) {
    if let Some(speech) = cx.try_global::<Speech>() {
        return (speech.engine.clone(), speech.owner.clone());
    }
    let (tx, mut events) = unbounded::<Event>();
    let engine = Rc::new(Engine::spawn(move |event| {
        tx.unbounded_send(event).ok();
    }));
    let owner = Owner::default();
    let serving = owner.clone();
    App::spawn(cx, async move |cx| {
        while let Some(event) = events.next().await {
            let Some((session, window)) = serving.borrow().clone() else { continue };
            let ended = matches!(event, Event::Transcript(_) | Event::Failed(_));
            window
                .update(cx, |_, window, cx| {
                    session.update(cx, |session, cx| session.dictation_event(event, window, cx)).ok();
                })
                .ok();
            if ended {
                *serving.borrow_mut() = None;
            }
        }
    })
    .detach();
    let saved = atelier_settings::path().map(|p| atelier_settings::load(&p)).unwrap_or_default();
    let prefs = Rc::new(RefCell::new(Prefs { device: saved.dictation_device, hold: saved.dictation_hold.unwrap_or(false) }));
    cx.set_global(Speech { engine: engine.clone(), owner: owner.clone(), prefs });
    (engine, owner)
}

/// Loads the model in the background if it is on this machine, so the first press does not wait for it.
pub fn warm(cx: &mut App) {
    speech(cx).0.warm();
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

/// What the person chose, as of now.
pub(super) fn prefs(cx: &mut App) -> Prefs {
    speech(cx);
    cx.global::<Speech>().prefs.borrow().clone()
}

/// Changes what the person chose and writes it to the settings file, off the UI thread.
pub(super) fn choose(cx: &mut App, change: impl FnOnce(&mut Prefs)) {
    speech(cx);
    let now = {
        let mut prefs = cx.global::<Speech>().prefs.borrow_mut();
        change(&mut prefs);
        prefs.clone()
    };
    let Some(path) = atelier_settings::path() else { return };
    cx.background_executor()
        .spawn(async move {
            if let Err(error) = atelier_settings::update(&path, |s| {
                s.dictation_device = now.device;
                s.dictation_hold = Some(now.hold).filter(|hold| *hold);
            }) {
                eprintln!("could not save the dictation settings: {error}");
            }
        })
        .detach();
}

/// The row that means "whichever microphone the system uses".
pub(super) const DEFAULT_ID: &str = "default";

/// The menu's rows for the microphones found: the system's default first, named after the one it is now, then each microphone;
/// and which row is chosen. A choice that is no longer plugged in falls back to the default row.
pub(super) fn device_rows(found: &[Device], chosen: Option<&str>) -> (Vec<VoiceDevice>, String) {
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
