/// An outside service a person can connect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Linear,
    GithubIssues,
    Slack,
    Discord,
    Gmail,
}

impl Kind {
    /// As the Settings page names it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Kind::Linear => "Linear",
            Kind::GithubIssues => "GitHub Issues",
            Kind::Slack => "Slack",
            Kind::Discord => "Discord",
            Kind::Gmail => "Gmail",
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
            Kind::Slack => "slackcli is not logged in. Run slackcli login in a terminal.",
            Kind::Discord => "discordcli is not logged in. Run discordcli login in a terminal.",
            Kind::Gmail => {
                "The browser that gmailcli uses is not signed in to Gmail. Sign in to Gmail there."
            }
        }
    }

    /// What a person does when the command line tool of this kind is not there. Empty for a kind with no tool.
    pub(crate) fn missing_tool(self) -> &'static str {
        match self {
            Kind::Slack => {
                "slackcli is not installed here, or not at that path. Get it at github.com/flazouh/slackcli."
            }
            Kind::Discord => {
                "discordcli is not installed here, or not at that path. Get it at github.com/flazouh/discordcli."
            }
            Kind::Gmail => {
                "gmailcli is not installed on that machine, or not at that path. Install it there, or set its path."
            }
            Kind::Linear | Kind::GithubIssues => "",
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
