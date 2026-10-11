use std::path::{Path, PathBuf};

use super::disk_bot_store_files::{json_files, memory_file, read, remove, write};
use crate::consts::NOTE_MAX;
use crate::enums::{BotsError, MemoryScope};
use crate::structs::{Bot, BotId, DiskBotStore, MemoryNote, Playbook, Run};
use crate::traits::BotStore;

impl DiskBotStore {
    /// A store in this folder. The folder is made when something is first kept.
    pub fn new(root: impl Into<PathBuf>) -> DiskBotStore {
        DiskBotStore { root: root.into() }
    }

    /// The folder the store keeps its files in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn bot_file(&self, id: &BotId) -> PathBuf {
        self.root.join("bots").join(format!("{id}.json"))
    }

    fn playbook_file(&self, id: &BotId) -> PathBuf {
        self.root.join("playbooks").join(format!("{id}.json"))
    }

    fn run_file(&self, id: &BotId) -> PathBuf {
        self.root.join("runs").join(format!("{id}.json"))
    }
}

impl BotStore for DiskBotStore {
    fn bots(&self) -> Result<Vec<Bot>, BotsError> {
        let mut out = Vec::new();
        for file in json_files(&self.root.join("bots"))? {
            out.extend(read::<Bot>(&file)?);
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    fn bot(&self, id: &BotId) -> Result<Bot, BotsError> {
        read::<Bot>(&self.bot_file(id))?.ok_or_else(|| BotsError::NotFound(id.to_string()))
    }

    fn save_bot(&self, mut bot: Bot) -> Result<Bot, BotsError> {
        let problems = bot.problems();
        if !problems.is_empty() {
            return Err(BotsError::Invalid(problems));
        }
        if let Some(kept) = read::<Bot>(&self.bot_file(&bot.id))? {
            if bot.same_content(&kept) {
                return Ok(kept);
            }
            bot.version = kept.version + 1;
        }
        write(&self.bot_file(&bot.id), &bot)?;
        Ok(bot)
    }

    fn remove_bot(&self, id: &BotId) -> Result<(), BotsError> {
        let playbooks: Vec<String> = self
            .playbooks()?
            .into_iter()
            .filter(|p| p.steps.iter().any(|s| s.bot == *id))
            .map(|p| p.id.to_string())
            .collect();
        if !playbooks.is_empty() {
            return Err(BotsError::Invalid(vec![format!(
                "{id} is a step of the playbook {}: take it out there first",
                playbooks.join(", ")
            )]));
        }
        let runs: Vec<String> = self
            .runs()?
            .into_iter()
            .filter(|r| r.bots_needed().contains(id))
            .map(|r| r.id.to_string())
            .collect();
        if !runs.is_empty() {
            return Err(BotsError::Invalid(vec![format!(
                "{id} still has work in the run {}: stop the run or let it end first",
                runs.join(", ")
            )]));
        }
        remove(&self.bot_file(id), id)
    }

    fn playbooks(&self) -> Result<Vec<Playbook>, BotsError> {
        let mut out = Vec::new();
        for file in json_files(&self.root.join("playbooks"))? {
            out.extend(read::<Playbook>(&file)?);
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    fn save_playbook(&self, playbook: Playbook) -> Result<(), BotsError> {
        let problems = playbook.problems(&self.bots()?);
        if !problems.is_empty() {
            return Err(BotsError::Invalid(problems));
        }
        write(&self.playbook_file(&playbook.id), &playbook)
    }

    fn remove_playbook(&self, id: &BotId) -> Result<(), BotsError> {
        remove(&self.playbook_file(id), id)
    }

    fn runs(&self) -> Result<Vec<Run>, BotsError> {
        let mut out = Vec::new();
        for file in json_files(&self.root.join("runs"))? {
            out.extend(read::<Run>(&file)?);
        }
        out.sort_by(|a, b| b.started_ms.cmp(&a.started_ms).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    fn run(&self, id: &BotId) -> Result<Run, BotsError> {
        read::<Run>(&self.run_file(id))?.ok_or_else(|| BotsError::NotFound(id.to_string()))
    }

    fn save_run(&self, run: &Run) -> Result<(), BotsError> {
        let mut problems = run.problems();
        let bots = self.bots()?;
        for bot in run.bots_needed() {
            if !bots.iter().any(|kept| kept.id == bot) {
                problems.push(format!(
                    "{}: the run still needs `{bot}`, and no such bot is kept",
                    run.id
                ));
            }
        }
        if !problems.is_empty() {
            return Err(BotsError::Invalid(problems));
        }
        write(&self.run_file(&run.id), run)
    }

    fn remove_run(&self, id: &BotId) -> Result<(), BotsError> {
        remove(&self.run_file(id), id)
    }

    fn notes(&self, scope: &MemoryScope) -> Result<Vec<MemoryNote>, BotsError> {
        Ok(read::<Vec<MemoryNote>>(&memory_file(&self.root, scope))?.unwrap_or_default())
    }

    fn add_note(
        &self,
        scope: &MemoryScope,
        text: &str,
        now_ms: i64,
    ) -> Result<MemoryNote, BotsError> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > NOTE_MAX {
            return Err(BotsError::Invalid(vec![format!(
                "a note needs 1 to {NOTE_MAX} characters"
            )]));
        }
        let path = memory_file(&self.root, scope);
        let mut notes = read::<Vec<MemoryNote>>(&path)?.unwrap_or_default();
        let next = self.next_note_id(scope, &notes)?;
        let note = MemoryNote {
            id: next,
            text: text.to_string(),
            created_ms: now_ms,
        };
        notes.push(note.clone());
        write(&path, &notes)?;
        write(&path.with_extension("next"), &(next + 1))?;
        Ok(note)
    }

    fn remove_note(&self, scope: &MemoryScope, id: u64) -> Result<(), BotsError> {
        let path = memory_file(&self.root, scope);
        let mut notes = read::<Vec<MemoryNote>>(&path)?.unwrap_or_default();
        let before = notes.len();
        notes.retain(|n| n.id != id);
        if notes.len() == before {
            return Err(BotsError::NotFound(format!("note {id}")));
        }
        write(&path, &notes)
    }
}

impl DiskBotStore {
    /// The next note number of a layer. It never goes back, even after a note is removed.
    fn next_note_id(&self, scope: &MemoryScope, notes: &[MemoryNote]) -> Result<u64, BotsError> {
        let counter = memory_file(&self.root, scope).with_extension("next");
        let kept = read::<u64>(&counter)?.unwrap_or(0);
        Ok(kept.max(notes.iter().map(|n| n.id + 1).max().unwrap_or(0)))
    }
}
