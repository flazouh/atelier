//! A repository on a forge, and the remote URL that names it.
use super::merge::MergeSettings;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RepoRef {
    pub host: String,
    pub owner: String,
    pub name: String,
}

impl RepoRef {
    pub fn new(host: impl Into<String>, owner: impl Into<String>, name: impl Into<String>) -> Self {
        Self { host: host.into(), owner: owner.into(), name: name.into() }
    }

    /// `owner/name`.
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// The repository a git remote URL names: `https://host/o/r(.git)`, `git@host:o/r(.git)`,
    /// `ssh://git@host/o/r(.git)`. Anything with more or fewer parts than an owner and a name is `None`.
    pub fn from_remote(url: &str) -> Option<Self> {
        let url = url.trim();
        let (host, path) = if let Some(rest) = url.split_once("://").map(|(_, rest)| rest) {
            let rest = rest.split_once('@').map_or(rest, |(_, after)| after);
            rest.split_once('/')?
        } else {
            let rest = url.split_once('@').map_or(url, |(_, after)| after);
            rest.split_once(':')?
        };
        let host = host.split(':').next().filter(|host| !host.is_empty())?;
        let path = path.trim_matches('/').trim_end_matches(".git");
        let mut parts = path.split('/');
        let (owner, name) = (parts.next()?, parts.next()?);
        if parts.next().is_some() || owner.is_empty() || name.is_empty() {
            return None;
        }
        Some(Self::new(host, owner, name))
    }
}

/// A repository with what the reader may do in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    /// The forge's own id, which writes need.
    pub id: String,
    pub reference: RepoRef,
    pub url: String,
    pub default_branch: Option<String>,
    pub merge: MergeSettings,
    /// The reader may open a pull request or push here.
    pub can_write: bool,
}

#[cfg(test)]
mod tests;
