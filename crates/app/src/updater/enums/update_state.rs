/// Where an update stands, for the window to show. See [`UpdateState::apply`] for how an event moves it.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum UpdateState {
    #[default]
    Idle,
    /// Looking for an update. `asked` is whether the reader asked for the look.
    Checking { asked: bool },
    /// Downloading and unpacking, `fraction` done (0 to 1).
    Downloading { version: String, notes: String, fraction: f64, asked: bool },
    /// Downloaded; waits for the reader.
    Ready { version: String, notes: String },
    Installing,
}
