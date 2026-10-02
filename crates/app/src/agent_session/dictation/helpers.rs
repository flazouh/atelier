use std::rc::Rc;

use atelier_voice::{Cue, Engine, Event};
use futures_channel::mpsc::unbounded;
use futures_util::StreamExt;
use gpui_kit::App;

use super::structs::Speech;
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
    cx.set_global(Speech { engine: engine.clone(), owner: owner.clone() });
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
