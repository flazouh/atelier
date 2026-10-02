use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

use super::helpers::strip_bytes;

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
