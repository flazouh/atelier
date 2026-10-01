//! A project's tracker, opened on the first ask and kept, so every pane and rule that asks gets the same
//! store and hears the same changes.

use std::sync::{Arc, Mutex};

use atelier_tracker::{Tracker, TrackerResult};

#[derive(Default)]
pub struct TrackerSlot(Mutex<Option<Arc<dyn Tracker>>>);

impl TrackerSlot {
    /// The kept tracker, or the one `open` makes. A failed open keeps nothing, so the next ask tries again.
    pub fn get_or_open(&self, open: impl FnOnce() -> TrackerResult<Arc<dyn Tracker>>) -> TrackerResult<Arc<dyn Tracker>> {
        let mut slot = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(kept) = &*slot {
            return Ok(kept.clone());
        }
        let opened = open()?;
        *slot = Some(opened.clone());
        Ok(opened)
    }
}
