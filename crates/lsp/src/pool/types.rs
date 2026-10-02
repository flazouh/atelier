use std::fmt;

use crate::LspError;

/// Why a file has no server, in words the status line can show as they are.
#[derive(Debug)]
pub enum NoServer {
    /// atelier knows no language for the file's extension.
    UnknownLanguage(String),
    /// The language has no server in the registry.
    NoServerFor(&'static str),
    /// The server is known but its program is not installed, and atelier cannot download it.
    NotInstalled { server: &'static str, install: &'static str },
    /// atelier tried to download the server and could not.
    DownloadFailed { server: &'static str, reason: String },
    /// The program ran but the handshake failed.
    Failed { server: &'static str, error: LspError },
}

impl fmt::Display for NoServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownLanguage(extension) => write!(f, "no language server for .{extension} files"),
            Self::NoServerFor(language) => write!(f, "no language server for {language}"),
            Self::NotInstalled { server, install } => write!(f, "{server} is not installed: {install}"),
            Self::DownloadFailed { server, reason } => write!(f, "could not download {server}: {reason}"),
            Self::Failed { server, error } => write!(f, "{server} did not start: {error}"),
        }
    }
}
