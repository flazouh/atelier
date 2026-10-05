use serde::Deserialize;

/// One line the socket takes.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    State,
    NewSession { agent: Option<String> },
    Send { text: String },
    /// Pretends the front session's account reached its weekly limit, resetting in 30 h 39 min.
    Limit,
    Find { name: String },
    /// A press on a marked element's centre, or on a point (`x`, `y`) of the window.
    Click { name: Option<String>, x: Option<f32>, y: Option<f32> },
    /// Brings a view to the front by the name the settings keep it by: sessions, tasks, pulls, files or git.
    View { name: String },
    /// Opens the folder at `path`, on `host` over SSH when one is named, as the add menu does.
    Open { path: String, host: Option<String> },
    /// Press Check for Updates, as the app menu does.
    CheckUpdates,
    /// Types `text` over the file `path` of the open project, which leaves a tab with an edit that is not saved.
    Edit { path: String, text: String },
}

/// The longest text a row's description keeps.
pub const TEXT_KEPT: usize = 160;
