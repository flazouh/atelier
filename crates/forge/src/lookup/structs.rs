use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{Forge, ForgeResult, PullSummary, RepoRef};
use super::types::{BRIEF_TTL, INVOLVED_TTL};

#[derive(Default)]
pub(super) struct Kept {
    /// `None` is an answer too: the number is not a pull request there.
    pub(super) briefs: HashMap<(RepoRef, u64), (u64, Option<PullSummary>)>,
    pub(super) involved: Option<(u64, Vec<PullSummary>)>,
}

pub struct Lookup {
    pub(super) forge: Arc<dyn Forge>,
    pub(super) current: RepoRef,
    pub(super) clock: Box<dyn Fn() -> u64 + Send + Sync>,
    pub(super) kept: Mutex<Kept>,
}

impl Lookup {
    /// `current` is the repository of the open project.
    pub fn new(forge: Arc<dyn Forge>, current: RepoRef) -> Self {
        let clock = || SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        Self { forge, current, clock: Box::new(clock), kept: Mutex::default() }
    }

    #[cfg(test)]
    pub(super) fn with_clock(mut self, clock: impl Fn() -> u64 + Send + Sync + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    /// One entry per number, in order. A number is a pull request of the current repository, or else of
    /// one the reader is involved in, or `None`. Asks the forge only for what is not kept.
    pub fn resolve(&self, numbers: &[u64]) -> ForgeResult<Vec<Option<PullSummary>>> {
        let now = (self.clock)();
        let stale: Vec<u64> = {
            let kept = self.kept();
            let mut missing: Vec<u64> = numbers
                .iter()
                .copied()
                .filter(|n| kept.briefs.get(&(self.current.clone(), *n)).is_none_or(|(at, _)| now.saturating_sub(*at) >= BRIEF_TTL))
                .collect();
            missing.sort_unstable();
            missing.dedup();
            missing
        };
        if !stale.is_empty() {
            let found = self.forge.briefs(&self.current, &stale)?;
            let mut kept = self.kept();
            for (number, brief) in stale.into_iter().zip(found) {
                kept.briefs.insert((self.current.clone(), number), (now, brief));
            }
        }
        let here: Vec<Option<PullSummary>> = {
            let kept = self.kept();
            numbers.iter().map(|n| kept.briefs.get(&(self.current.clone(), *n)).and_then(|(_, b)| b.clone())).collect()
        };
        if here.iter().all(Option::is_some) {
            return Ok(here);
        }
        let involved = self.involved(now)?;
        Ok(numbers
            .iter()
            .zip(here)
            .map(|(number, brief)| brief.or_else(|| involved.iter().find(|s| s.brief.reference.number == *number).cloned()))
            .collect())
    }

    pub(super) fn involved(&self, now: u64) -> ForgeResult<Vec<PullSummary>> {
        if let Some((at, list)) = &self.kept().involved
            && now.saturating_sub(*at) < INVOLVED_TTL
        {
            return Ok(list.clone());
        }
        let list: Vec<PullSummary> = self.forge.involved()?.into_iter().map(|i| i.summary).collect();
        self.kept().involved = Some((now, list.clone()));
        Ok(list)
    }

    pub(super) fn kept(&self) -> std::sync::MutexGuard<'_, Kept> {
        self.kept.lock().unwrap_or_else(|e| e.into_inner())
    }
}
