//! How a request reaches the forge. The rest of the GitHub backend sees only this: a method, a path
//! and a body in, a status, headers and a body out. Nothing here holds a credential.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub method: &'static str,
    /// `graphql`, or a REST path such as `repos/o/r/pulls/1`.
    pub path: String,
    pub body: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    /// Header names in lower case.
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Reply {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
    }
}

/// Why a request produced no reply at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportError {
    ToolMissing,
    NotSignedIn,
    Offline,
    Failed(String),
}

pub trait Transport: Send + Sync {
    fn send(&self, request: &Request) -> Result<Reply, TransportError>;
}
