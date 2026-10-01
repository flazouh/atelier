use std::path::PathBuf;

use lsp_types::{Range, Uri};

use super::types::Found;

/// One place an answer points at. `range` counts characters, whatever the server counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub uri: Uri,
    /// The file `uri` names, canonical, for comparing with other paths. `None` for a server's virtual
    /// document, which has no file.
    pub path: Option<PathBuf>,
    pub range: Range,
    /// That line's text, trimmed, for a list of places. Empty when the file could not be read.
    pub line_text: String,
}

/// Where a Cmd-click or F12 goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Navigation {
    pub found: Found,
    pub targets: Vec<Target>,
}
