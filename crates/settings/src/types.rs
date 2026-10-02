use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::helpers::tilde;

/// How many recent projects the list keeps.
pub const RECENT_LIMIT: usize = 10;

/// Where a project lives.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Location {
    Local { path: PathBuf },
    /// A folder on an SSH host, as `ssh` names the host (an alias from ~/.ssh/config works).
    Ssh { host: String, path: PathBuf },
}

impl Location {
    /// The folder's own name, for a list.
    pub fn name(&self) -> String {
        let path = match self {
            Location::Local { path } | Location::Ssh { path, .. } => path,
        };
        path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
    }

    /// Where it is, under the name: the folder's path, and the host for a remote one.
    pub fn place(&self) -> String {
        match self {
            Location::Local { path } => tilde(path),
            Location::Ssh { host, path } => format!("{host}:{}", path.display()),
        }
    }
}
