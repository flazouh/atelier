use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use gpui_kit::{App, Window};
use atelier_lsp::{LspWorker, Navigation};
use lsp_types::ShowDocumentParams;

use super::structs::Jump;

/// How long one question may take. Servers answer in milliseconds once they have indexed.
pub const ASK: Duration = Duration::from_secs(20);

/// How long a server may take to start and shake hands.
pub const READY: Duration = Duration::from_secs(60);

/// How long typing must pause before the buffer is checked again. docs/code-editor.md sets 150ms.
pub(super) const RECHECK_AFTER: Duration = Duration::from_millis(150);

/// The key a Zed user holds to follow a symbol.
pub(super) const SECONDARY: &str = if cfg!(target_os = "macos") { "⌘" } else { "ctrl" };

/// The newest answer to a Cmd-hover, Cmd-click or F12, kept so a click can tell one place to jump to
/// from several to list. The provider writes it off the main thread.
pub(super) type LastNavigation = Arc<Mutex<Option<Navigation>>>;

/// What the owner does with a jump out of the file.
pub type Elsewhere = Rc<dyn Fn(Jump, &mut Window, &mut App)>;

/// What the thread that starts a server says, in order: any downloads, then the result.
pub(super) enum Starting {
    Downloading(String),
    Done(Result<LspWorker, String>),
}

/// What to do when gpui-base is about to jump: `true` means it was handled here.
pub(super) type ShowDocument = Rc<dyn Fn(&ShowDocumentParams, &mut Window, &mut App) -> bool>;
