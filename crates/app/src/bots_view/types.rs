use atelier_bots::Bot;

/// What the Bots view asks of the window that shows it.
pub enum BotsEvent {
    /// The reader pressed "Start a session" on this bot's profile.
    StartSession(Bot),
}
