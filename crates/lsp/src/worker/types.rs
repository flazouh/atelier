use lsp_types::{Diagnostic, Hover, Position};

use crate::{LspError, navigation::{Navigation, Target}, symbols::Symbol};
use super::structs::Doc;
use super::helpers::canonical;

/// Where an answer goes. It runs on the worker's thread.
pub type Reply<T> = Box<dyn FnOnce(Result<T, LspError>) + Send>;

pub(super) enum Job {
    Navigate { doc: Doc, position: Position, reply: Reply<Navigation> },
    References { doc: Doc, position: Position, reply: Reply<Vec<Target>> },
    Hover { doc: Doc, position: Position, reply: Reply<Option<Hover>> },
    Diagnostics { doc: Doc, reply: Reply<Vec<Diagnostic>> },
    Symbols { doc: Doc, reply: Reply<Vec<Symbol>> },
    /// Needs a document only to open the project's server on it.
    ProjectSymbols { doc: Doc, query: String, reply: Reply<Vec<Symbol>> },
}

impl Job {
    /// The same job with its document's path made canonical.
    pub(super) fn with_canonical_path(self) -> Self {
        let fix = |doc: Doc| Doc { path: canonical(&doc.path), ..doc };
        match self {
            Job::Navigate { doc, position, reply } => Job::Navigate { doc: fix(doc), position, reply },
            Job::References { doc, position, reply } => Job::References { doc: fix(doc), position, reply },
            Job::Hover { doc, position, reply } => Job::Hover { doc: fix(doc), position, reply },
            Job::Diagnostics { doc, reply } => Job::Diagnostics { doc: fix(doc), reply },
            Job::Symbols { doc, reply } => Job::Symbols { doc: fix(doc), reply },
            Job::ProjectSymbols { doc, query, reply } => Job::ProjectSymbols { doc: fix(doc), query, reply },
        }
    }

    /// Answers the job with `error` instead of running it.
    pub(super) fn fail(self, error: LspError) {
        match self {
            Job::Navigate { reply, .. } => reply(Err(error)),
            Job::References { reply, .. } => reply(Err(error)),
            Job::Hover { reply, .. } => reply(Err(error)),
            Job::Diagnostics { reply, .. } => reply(Err(error)),
            Job::Symbols { reply, .. } => reply(Err(error)),
            Job::ProjectSymbols { reply, .. } => reply(Err(error)),
        }
    }
}

/// The code a server answers with when the text changed under a request, which LSP 3.17 names
/// `ContentModified`. rust-analyzer sends it for a request that arrives just after a `didChange`.
pub const CONTENT_MODIFIED: i64 = -32801;

/// The code for a request the server dropped and wants asked again, which LSP 3.17 names
/// `ServerCancelled`. A pull for diagnostics can get it while the server is busy.
pub const SERVER_CANCELLED: i64 = -32802;
