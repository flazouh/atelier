use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::{AtomicU32, Ordering},
    time::Instant,
};

use atelier_bot_face::{BotRuntime, FaceSet, Mood};
use atelier_bots::Bot;
use gpui_kit::{AnyElement, IntoElement, Rgba};

use crate::bots_view::helpers::{face, face_of};

/// The bot a session belongs to: its record as it was read when the session opened, and what moves its face. It reads as
/// the record: `bot.id`, `bot.persona()`.
pub struct SessionBot {
    record: Bot,
    pub(in super::super) runtime: Rc<RefCell<BotRuntime>>,
    started: Instant,
}

impl std::ops::Deref for SessionBot {
    type Target = Bot;

    fn deref(&self) -> &Bot {
        &self.record
    }
}

impl SessionBot {
    /// Each session's face starts at its own phase, so a list of them does not move together.
    pub fn new(record: Bot) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(1);
        let seed = NEXT.fetch_add(1, Ordering::Relaxed);
        Self { record, runtime: Rc::new(RefCell::new(BotRuntime::new(seed, 0.))), started: Instant::now() }
    }

    /// The bot's face, `side` design pixels square, in `mood`, with `ink` for its outline. Where it `may_move`, it moves
    /// while the session works (it thinks or runs a tool); it stands still at the mood's pose otherwise, and `face` itself
    /// stands still under reduce motion. `None` for a bot the face data has no face for.
    pub fn face(&self, faces: &Rc<FaceSet>, mood: Mood, side: f32, ink: Rgba, may_move: bool) -> Option<AnyElement> {
        let model = face_of(faces, &self.record)?;
        let motion = (may_move && moves(mood)).then(|| (self.runtime.clone(), self.started));
        Some(face(faces.clone(), model, mood, motion, side, ink).into_any_element())
    }
}

/// Whether a face in this mood moves: only while the session works.
fn moves(mood: Mood) -> bool {
    matches!(mood, Mood::Thinking | Mood::Working)
}
