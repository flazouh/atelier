use crate::enums::{BotsError, MemoryScope};
use crate::structs::{Bot, BotId, MemoryNote, Playbook};

/// Where bots, playbooks and notes are kept. The app talks to this, and a test hands in a fake.
pub trait BotStore {
    /// Every bot, in the order of their ids.
    fn bots(&self) -> Result<Vec<Bot>, BotsError>;
    fn bot(&self, id: &BotId) -> Result<Bot, BotsError>;
    /// Keeps a bot. A bot that is new keeps its version. A bot that differs from the kept one gets the next
    /// version. A bot that is the same changes nothing. Returns the bot as kept.
    fn save_bot(&self, bot: Bot) -> Result<Bot, BotsError>;
    /// Removes a bot. A bot that a playbook names is refused, so no playbook is left with a step that has no bot.
    fn remove_bot(&self, id: &BotId) -> Result<(), BotsError>;
    fn playbooks(&self) -> Result<Vec<Playbook>, BotsError>;
    /// Keeps a playbook after it checks that every step names a bot that is kept.
    fn save_playbook(&self, playbook: Playbook) -> Result<(), BotsError>;
    fn remove_playbook(&self, id: &BotId) -> Result<(), BotsError>;
    /// The notes of one layer, oldest first.
    fn notes(&self, scope: &MemoryScope) -> Result<Vec<MemoryNote>, BotsError>;
    /// Adds a note to a layer. `now_ms` is the time to write on it.
    fn add_note(
        &self,
        scope: &MemoryScope,
        text: &str,
        now_ms: i64,
    ) -> Result<MemoryNote, BotsError>;
    fn remove_note(&self, scope: &MemoryScope, id: u64) -> Result<(), BotsError>;
}
