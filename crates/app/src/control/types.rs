use serde::Deserialize;

/// One line the socket takes.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    State,
    NewSession { agent: Option<String> },
    Send { text: String },
}

/// The longest text a row's description keeps.
pub const TEXT_KEPT: usize = 160;
