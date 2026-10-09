use std::sync::{Arc, Mutex};

use atelier_agents::usage_history::{Cache, Roots, UsageHistory};
use gpui_kit::Global;

/// What the dashboard keeps between openings: what the logs said the last time it read them, the cache that makes the next
/// reading cheap, and where the logs are.
pub struct UsageStore {
    pub seen: Arc<UsageHistory>,
    pub cache: Arc<Mutex<Cache>>,
    pub roots: Roots,
}

impl Default for UsageStore {
    fn default() -> Self {
        Self {
            seen: Arc::default(),
            cache: Arc::default(),
            roots: Roots::from_env(),
        }
    }
}

impl Global for UsageStore {}
