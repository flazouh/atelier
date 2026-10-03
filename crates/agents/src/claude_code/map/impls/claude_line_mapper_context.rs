//! How full the context is, and the window of each model.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::claude_code::wire::{ModelUsage, RawUsage};
use crate::session::{ContextFill, Event};

use super::super::structs::ClaudeLineMapper;

impl ClaudeLineMapper {
    /// The event for a new reading of the context, or nothing when it is the same as the last.
    pub(super) fn context_changed(&mut self, context: ContextFill) -> Option<Event> {
        (std::mem::replace(&mut self.context, context) != context).then_some(Event::Context(context))
    }
}

/// The tokens a request carried, which is what the context holds: the new input, what the cache gave
/// and what it took in, and the reply that joins them.
pub(super) fn context_tokens(usage: &RawUsage) -> u64 {
    usage.input_tokens + usage.cache_read_input_tokens + usage.cache_creation_input_tokens + usage.output_tokens
}

/// The window of the session's model, or the largest the turn used when the model is not among them.
/// Every window told is kept for [`known_window`].
pub(super) fn context_window(models: &HashMap<String, ModelUsage>, model: Option<&str>) -> Option<u64> {
    let mut known = windows().lock().unwrap_or_else(|p| p.into_inner());
    known.extend(models.iter().filter_map(|(name, usage)| Some((name.clone(), usage.context_window?))));
    model
        .and_then(|name| models.get(name))
        .and_then(|usage| usage.context_window)
        .or_else(|| models.values().filter_map(|usage| usage.context_window).max())
}

/// The window of `model` as some session's result told it. Only a result tells a window, so a session
/// resumed from its transcript knows its window before its first turn ends only through another.
pub(super) fn known_window(model: &str) -> Option<u64> {
    windows().lock().unwrap_or_else(|p| p.into_inner()).get(model).copied()
}

fn windows() -> &'static Mutex<HashMap<String, u64>> {
    static WINDOWS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    WINDOWS.get_or_init(Mutex::default)
}
