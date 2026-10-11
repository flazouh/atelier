use std::path::PathBuf;

use gpui_kit::Global;

/// The folder the bots of sessions are read from. The app names the reader's own at startup, the one the Bots view
/// keeps them in; a test names its own.
pub struct BotsFolder(pub Option<PathBuf>);

impl Global for BotsFolder {}
