//! Why a forge call failed, in a form the UI can show without knowing the forge.
use std::fmt;

pub type ForgeResult<T> = Result<T, ForgeError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForgeError {
    /// The forge's command line tool is not on the host.
    ToolMissing { tool: String },
    /// The tool is there and the reader is signed out, or the sign-in no longer works.
    NotSignedIn,
    /// The forge could not be reached.
    Offline,
    /// The forge asked lathe to wait. `retry_after` is in seconds, when it says.
    RateLimited { retry_after: Option<u64> },
    /// The thing is not there, or the reader may not see it.
    NotFound(String),
    /// The reader lacks the right to do it.
    Denied(String),
    /// The forge refused the change, with its own reason: a merge with conflicts, a title left empty.
    Rejected(String),
    /// The remote is not one this forge serves.
    UnknownRemote(String),
    /// An answer lathe could not read: the forge changed shape, or the call broke in transit.
    Unexpected(String),
}

impl fmt::Display for ForgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ToolMissing { tool } => write!(f, "{tool} is not installed on this host"),
            Self::NotSignedIn => f.write_str("You are not signed in to the forge on this host"),
            Self::Offline => f.write_str("The forge could not be reached"),
            Self::RateLimited { retry_after: Some(seconds) } => {
                write!(f, "The forge asked us to wait {seconds} seconds")
            }
            Self::RateLimited { retry_after: None } => f.write_str("The forge asked us to wait"),
            Self::NotFound(what) => write!(f, "{what} was not found"),
            Self::Denied(why) => write!(f, "Not allowed: {why}"),
            Self::Rejected(why) => f.write_str(why),
            Self::UnknownRemote(url) => write!(f, "{url} is not a remote this forge serves"),
            Self::Unexpected(why) => write!(f, "The forge sent something unexpected: {why}"),
        }
    }
}

impl std::error::Error for ForgeError {}
