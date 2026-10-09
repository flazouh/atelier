#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Patch,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Patch => "PATCH",
        }
    }
}

/// Why `gh` gave no reply at all. A reply with an error status is not a failure here: it is a [`Reply`](super::Reply).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// `gh` is not on this host.
    ToolMissing,
    /// `gh` is there and the person is signed out, or the sign-in no longer works.
    NotSignedIn,
    /// GitHub could not be reached.
    Offline,
    /// Anything else, with the last line `gh` wrote.
    Other(String),
}

/// `gh` exits with 4 when it needs a sign-in.
pub(super) const NEEDS_SIGN_IN: i32 = 4;
