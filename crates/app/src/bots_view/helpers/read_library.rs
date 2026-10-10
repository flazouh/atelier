use std::path::Path;

use atelier_bots::{BotStore, DiskBotStore, MemoryScope, seed_starters};

use super::super::structs::{Entry, Library};

/// Reads the folder: seeds the starters it lacks, then every bot with the notes of its own layer. An error is kept
/// in words, with the path of the file it names, and the view shows it. Call it off the UI thread.
pub fn read_library(root: &Path) -> Library {
    let store = DiskBotStore::new(root);
    // Seeding reads the folder first, so a broken file is told here, by its path.
    match seed_starters(&store).and_then(|_| store.bots()) {
        Ok(bots) => Library {
            entries: bots
                .into_iter()
                .map(|bot| {
                    let notes = store.notes(&MemoryScope::Bot(bot.id.clone())).unwrap_or_default();
                    Entry { bot, notes }
                })
                .collect(),
            error: None,
        },
        Err(e) => Library { entries: Vec::new(), error: Some(format!("The bots could not be read: {e}")) },
    }
}
