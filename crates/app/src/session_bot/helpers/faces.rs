use atelier_ui::ActiveTheme;
use gpui_kit::{AnyElement, App, Entity, Rgba};

use crate::agent_session::AgentSession;

use super::super::consts::{HEADER_FACE, ROW_FACE};
use super::super::structs::Faces;

/// The face of the session's bot for its row in the sidebar, in the mood of what the session does and with the theme's
/// foreground as its ink; `None` for a session with no bot. It moves while the session works and stands still otherwise,
/// and always under reduce motion.
pub fn row_face(session: &Entity<AgentSession>, cx: &mut App) -> Option<AnyElement> {
    face_of_session(session, ROW_FACE, true, cx)
}

/// The face of the session's bot for the head of its panel: the same face in the same mood, standing still. The row is
/// where a reader watches a session work, and one moving face for a session is enough.
pub fn header_face(session: &Entity<AgentSession>, cx: &mut App) -> Option<AnyElement> {
    face_of_session(session, HEADER_FACE, false, cx)
}

fn face_of_session(session: &Entity<AgentSession>, side: f32, may_move: bool, cx: &mut App) -> Option<AnyElement> {
    let mood = session.read(cx).mood()?;
    let faces = cx.default_global::<Faces>().0.clone();
    let ink = Rgba::from(cx.theme().foreground);
    session.read(cx).bot.as_ref()?.face(&faces, mood, side, ink, may_move)
}
