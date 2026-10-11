//! Sessions that belong to a bot, as the shell wires them: one starts from a bot's profile or by the bot's id, a session
//! kept at the last quit finds its bot again, and the sidebar's rows show the bot's face. What a bot's session is, is
//! written in `session_bot`.

use atelier_bots::Bot;
use atelier_ui::sidebar::Sidebar;
use gpui_kit::{App, Context, Entity, WeakEntity, Window};

use super::{structs::Shell, view::ShellView};
use crate::{
    agent_session::AgentSession,
    bots_view::{BotsEvent, BotsPage, helpers::harness_words},
    session_bot,
};

/// Lets the sidebar's session rows show the face of a session's bot in the place of the agent's mark. The sidebar asks for
/// a row's mark by the session's key each time it draws the row; a session with no bot gives none, and keeps the agent's.
pub(super) fn show_bot_faces(sidebar: &Entity<Sidebar>, shell: WeakEntity<Shell>, cx: &mut App) {
    sidebar.update(cx, |sidebar, cx| {
        sidebar.set_session_mark(
            move |key, _, cx| {
                let (_, session) = shell.upgrade()?.read(cx).session_by_key(key, cx)?;
                session_bot::row_face(&session, cx)
            },
            cx,
        )
    });
}

impl Shell {
    /// A new session that belongs to `bot`, on the bot's harness, in the project in front. The Sessions view comes in
    /// front with the session open. An error says, in words for the reader, why none started.
    pub(crate) fn new_session_as(&mut self, bot: Bot, window: &mut Window, cx: &mut Context<Self>) -> Result<Entity<AgentSession>, String> {
        let Some(agent) = session_bot::agent_of(bot.harness) else {
            return Err(format!("{} runs on {}, which atelier cannot start yet.", bot.name, harness_words(bot.harness)));
        };
        let at = self.active;
        let Some(project) = self.projects.get(at).cloned() else {
            return Err("Open a project first: a session works in one.".into());
        };
        let session = project.update(cx, |p, cx| p.open_session_as(None, Some(agent), None, Some(bot), window, cx));
        self.show_session(at, &session, window, cx);
        self.show_view(ShellView::Sessions, window, cx);
        Ok(session)
    }

    /// As [`Self::new_session_as`], for the bot the folder keeps under `id`. The folder gets the starters it lacks first,
    /// so a script can name one before the Bots view was ever open. It reads the folder on this thread: it is for the
    /// control socket.
    pub(crate) fn new_session_of_bot(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) -> Result<Entity<AgentSession>, String> {
        let root = self.bots_root.clone().ok_or("There is no folder to keep bots in.")?;
        let bot = session_bot::seeded_bot(&root, id)?;
        self.new_session_as(bot, window, cx)
    }

    /// The bot a session kept at the last quit belongs to, read from the folder by its id: one small file. A bot that is
    /// gone, or that cannot be read, leaves the session with no bot.
    pub(super) fn kept_bot(&self, id: &str) -> Option<Bot> {
        session_bot::kept_bot(self.bots_root.as_deref()?, id).map_err(|why| eprintln!("{why}")).ok()
    }

    /// Hears the Bots view: a press on a profile's "Start a session".
    pub(super) fn bots_event(&mut self, _: &Entity<BotsPage>, event: &BotsEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            BotsEvent::StartSession(bot) => {
                if let Err(why) = self.new_session_as(bot.clone(), window, cx) {
                    self.say(why, cx);
                }
            }
        }
    }
}
