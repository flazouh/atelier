//! A project's identity for the tracker: where its database lives, and the prefix of its short ids.
use std::path::{Path, PathBuf};

/// Which project. A local path, or a host and a path over SSH. The same project always gives the same
/// database, wherever the repository is cloned from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProjectKey {
    Local { path: String },
    Ssh { host: String, path: String },
}

impl ProjectKey {
    fn identity(&self) -> String {
        match self {
            Self::Local { path } => format!("local:{}", trim_slash(path)),
            Self::Ssh { host, path } => format!("ssh:{}:{}", host.to_lowercase(), trim_slash(path)),
        }
    }

    /// The last part of the path: "lathe" for "/Users/alex/code/lathe".
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

fn trim_slash(path: &str) -> &str {
    if path.len() > 1 { path.trim_end_matches('/') } else { path }
}

/// FNV-1a, 64 bits: the same on every machine and every run, unlike the standard library's hasher.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
}

/// The prefix of short ids from a project's name: its first three letters or digits, in capitals.
/// "lathe" gives "LAT", "api-server" gives "API", and a name with none gives "TSK".
pub fn prefix_for(name: &str) -> String {
    let prefix: String = name.chars().filter(char::is_ascii_alphanumeric).take(3).collect::<String>().to_uppercase();
    if prefix.is_empty() { "TSK".to_string() } else { prefix }
}

#[cfg(test)]
mod tests;
