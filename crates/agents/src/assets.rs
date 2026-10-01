//! Serves every agent's strips to GPUI, then everything atelier-ui serves.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

use crate::claude;

/// Hand this to the application in place of [`atelier_ui::Assets`], so agents' marks load.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = strip_bytes(path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        atelier_ui::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        atelier_ui::Assets.list(path)
    }
}

/// The embedded strip or mark served at `path`, if an agent or a lab ships one there.
pub(crate) fn strip_bytes(path: &str) -> Option<&'static [u8]> {
    claude::SparkState::ALL.iter().map(|state| state.strip()).find(|strip| strip.path == path).map(|strip| strip.bytes)
        .or_else(|| crate::labs::bytes(path))
        .or_else(|| crate::coding_agents::bytes(path))
}

#[cfg(test)]
mod tests;
