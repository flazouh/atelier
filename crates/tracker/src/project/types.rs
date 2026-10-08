use std::path::{Path, PathBuf};

use super::helpers::{fnv1a, trim_slash};

/// Which project. A local path, or a host and a path over SSH. The same project always gives the same
/// database, wherever the repository is cloned from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProjectKey {
    Local { path: String },
    Ssh { host: String, path: String },
}

impl ProjectKey {
    pub(super) fn identity(&self) -> String {
        match self {
            Self::Local { path } => format!("local:{}", trim_slash(path)),
            Self::Ssh { host, path } => format!("ssh:{}:{}", host.to_lowercase(), trim_slash(path)),
        }
    }

    /// The last part of the path: "atelier" for "/Users/user/code/atelier".
    pub fn folder(&self) -> &str {
        let (Self::Local { path } | Self::Ssh { path, .. }) = self;
        trim_slash(path).rsplit('/').find(|part| !part.is_empty()).unwrap_or("project")
    }

    /// The database file's name: the folder, for a person to recognize it, and a hash of the identity, so
    /// two projects in folders of one name do not share a file.
    pub fn file_name(&self) -> String {
        let readable: String =
            self.folder().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
        format!("{readable}-{:016x}.sqlite", fnv1a(self.identity().as_bytes()))
    }

    /// The database's place in the app's data folder: `<data>/tracker/<file name>`. Never inside the
    /// repository.
    pub fn path_in(&self, data_dir: &Path) -> PathBuf {
        data_dir.join("tracker").join(self.file_name())
    }
}
