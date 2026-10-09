/// An outside service a person can connect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Linear,
    GithubIssues,
}

impl Kind {
    /// As the Settings page names it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Kind::Linear => "Linear",
            Kind::GithubIssues => "GitHub Issues",
        }
    }

    /// The word that tells a person what to do about a refusal.
    pub(crate) fn refusal(self) -> &'static str {
        match self {
            Kind::Linear => {
                "Linear does not accept this key. Make a new one in Linear under Settings, Security and access."
            }
            Kind::GithubIssues => {
                "GitHub does not accept the gh login. Run gh auth login in a terminal."
            }
        }
    }
}

/// How one kind stands: the state of its row in Settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Row {
    /// Not connected: nothing is saved for it.
    #[default]
    Off,
    /// Saved, and the app is asking the service.
    Checking,
    /// Working. Holds the name of the person it signed in as.
    Connected(String),
    NotSignedIn,
    Offline,
    /// Anything else, in plain words.
    Failed(String),
}
