use std::path::PathBuf;

use gpui_kit::App;

use super::super::structs::BotsFolder;

/// The folder the bots of sessions are kept in, as the app named it when it started; none when it named none.
pub fn bots_folder(cx: &App) -> Option<PathBuf> {
    cx.try_global::<BotsFolder>()?.0.clone()
}
