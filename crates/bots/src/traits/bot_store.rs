use crate::enums::{BotsError, MemoryScope};
use crate::structs::{Bot, BotId, MemoryNote, Playbook, Run};

/// Where bots, playbooks, runs and notes are kept. The app talks to this, and a test hands in a fake.
pub trait BotStore {
    /// Every bot, in the order of their ids.
    fn bots(&self) -> Result<Vec<Bot>, BotsError>;
    fn bot(&self, id: &BotId) -> Result<Bot, BotsError>;
    /// Keeps a bot. A bot that is new keeps its version. A bot that differs from the kept one gets the next
    /// version. A bot that is the same changes nothing. Returns the bot as kept.
    fn save_bot(&self, bot: Bot) -> Result<Bot, BotsError>;
    /// Removes a bot. A bot that a playbook names, or that a run still needs, is refused, so no playbook and no run
    /// is left with a step that has no bot.
    fn remove_bot(&self, id: &BotId) -> Result<(), BotsError>;
    fn playbooks(&self) -> Result<Vec<Playbook>, BotsError>;
    /// Keeps a playbook after it checks that every step names a bot that is kept.
    fn save_playbook(&self, playbook: Playbook) -> Result<(), BotsError>;
    fn remove_playbook(&self, id: &BotId) -> Result<(), BotsError>;
    /// Every run, the newest first.
    fn runs(&self) -> Result<Vec<Run>, BotsError>;
    fn run(&self, id: &BotId) -> Result<Run, BotsError>;
    /// Keeps a run after it checks its rules, and that every bot the run still needs is kept. A run that is over
    /// needs none, so it is kept as a record even after its bots are gone.
    fn save_run(&self, run: &Run) -> Result<(), BotsError>;
    fn remove_run(&self, id: &BotId) -> Result<(), BotsError>;
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
