//! The list of pull requests without a window: the reader's working set filed into Courts, and what the
//! list says about itself (when it was read, why it could not be). Filing is `atelier_forge::file_courts`;
//! the words for each row are `atelier_forge::present::court_item`.
use std::collections::HashMap;

use atelier_ui::{PrChipData, court::CourtItem};
use atelier_forge::{Court, Filed, ForgeError, Involved, PullRef, file_courts, present};

#[derive(Default)]
pub struct ListModel {
    filed: Vec<(Court, Vec<Filed>)>,
    /// A working set has been read, from disk or from the forge.
    pub loaded: bool,
    /// The last refresh failed, and why.
    pub error: Option<ForgeError>,
    /// When the set was read, in seconds; `0` before any.
    pub updated: u64,
    /// When each pull request was last opened, by `(repo slug, number)`.
    opened: HashMap<(String, u64), u64>,
}

impl ListModel {
    pub fn set(&mut self, items: Vec<Involved>, updated: u64) {
        // No stacks are known here: a pull request never waits on one below it.
        self.filed = file_courts(items, |_| false);
        self.loaded = true;
        self.updated = updated;
    }

    pub fn set_opened(&mut self, opened: HashMap<(String, u64), u64>) {
        self.opened = opened;
    }

    /// A pull request was opened just now, so it is no longer unread.
    pub fn mark_opened(&mut self, reference: &PullRef, at: u64) {
        self.opened.insert((reference.repo.slug(), reference.number), at);
    }

    /// Every pull request as a row, Courts in reading order. A pull request changed since the reader opened
    /// it is unread; one never opened is unread too.
    pub fn rows(&self, now: u64) -> Vec<CourtItem> {
        self.filed
            .iter()
            .flat_map(|(_, rows)| rows)
            .map(|filed| {
                let mut item = present::court_item(filed, now);
                let reference = &filed.involved.summary.brief.reference;
                item.unread = self.opened.get(&(reference.repo.slug(), reference.number)).is_none_or(|at| *at < filed.involved.summary.updated_at);
                item
            })
            .collect()
    }

    /// The pull request a row's chip names.
    pub fn reference_of(&self, chip: &PrChipData) -> Option<PullRef> {
        self.filed.iter().flat_map(|(_, rows)| rows).map(|f| &f.involved.summary.brief.reference).find(|r| r.number == chip.number && r.repo.slug() == chip.repo.as_ref()).cloned()
    }

    pub fn len(&self) -> usize {
        self.filed.iter().map(|(_, rows)| rows.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many are in the first Court: the ones that need the reader.
    pub fn needs_you(&self) -> usize {
        self.filed.iter().find(|(court, _)| *court == Court::NeedsYou).map_or(0, |(_, rows)| rows.len())
    }

    /// The pull requests the list shows as merged or closed: their checkouts can go.
    pub fn closed(&self) -> Vec<PullRef> {
        self.filed
            .iter()
            .flat_map(|(_, rows)| rows)
            .map(|f| &f.involved.summary.brief)
            .filter(|b| matches!(b.state, atelier_forge::PullState::Merged | atelier_forge::PullState::Closed))
            .map(|b| b.reference.clone())
            .collect()
    }

    /// The pull requests that are open, for a checkout cleanup: a closed one's checkout can go.
    pub fn open_numbers(&self, repo: &atelier_forge::RepoRef) -> Vec<u64> {
        self.filed
            .iter()
            .flat_map(|(_, rows)| rows)
            .map(|f| &f.involved.summary.brief)
            .filter(|b| b.reference.repo == *repo && matches!(b.state, atelier_forge::PullState::Open | atelier_forge::PullState::Draft))
            .map(|b| b.reference.number)
            .collect()
    }
}
