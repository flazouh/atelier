use std::path::Path;

use serde_json::Value;

use crate::install::Download;

/// How to run one language server, and for which files.
#[derive(Debug)]
pub struct ServerSpec {
    /// What the status line calls it.
    pub name: &'static str,
    pub program: &'static str,
    pub args: &'static [&'static str],
    /// The LSP language ids it serves, as `language_id` returns them.
    pub language_ids: &'static [&'static str],
    /// Files whose directory is a project root for this server.
    pub root_markers: &'static [&'static str],
    /// How to install it, for the status line when it is missing and atelier cannot download it.
    pub install: &'static str,
    /// The pinned copy atelier downloads when the user has none.
    pub download: Option<Download>,
    /// The `initializationOptions` to send, from where the program is and the project root.
    pub initialization_options: fn(program: &Path, root: &Path) -> Option<Value>,
}
