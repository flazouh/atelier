use std::path::Path;

use atelier_bots::{Bot, BotId, BotStore, DiskBotStore, seed_starters};

/// The bot the folder keeps under `id`, or why there is none, in words. It reads one small file.
pub fn kept_bot(root: &Path, id: &str) -> Result<Bot, String> {
    let id: BotId = id.parse()?;
    DiskBotStore::new(root).bot(&id).map_err(|e| format!("the bot `{id}` could not be read: {e}"))
}

/// As [`kept_bot`], after the folder got the starters it lacks, so a folder nobody opened yet knows them.
pub fn seeded_bot(root: &Path, id: &str) -> Result<Bot, String> {
    seed_starters(&DiskBotStore::new(root)).map_err(|e| format!("the bots could not be read: {e}"))?;
    kept_bot(root, id)
}
