use std::fmt;

/// What can go wrong in dictation, in words fit for the person using it.
#[derive(Debug)]
pub enum Error {
    /// No microphone, or the system will not let atelier use it.
    Microphone(String),
    /// The person refused the microphone, or a policy does.
    Access,
    /// The model could not be fetched.
    Network(String),
    /// A fetched file is not the file that was pinned.
    Checksum(&'static str),
    /// The folder for the model cannot be written.
    Disk(std::io::Error),
    /// The model would not load or run.
    Model(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Microphone(why) => write!(f, "Microphone unavailable: {why}"),
            Self::Access => write!(f, "Microphone access is off. Allow Atelier in System Settings > Privacy."),
            Self::Network(why) => write!(f, "Could not download the speech model: {why}"),
            Self::Checksum(file) => write!(f, "The downloaded {file} is damaged. Try again."),
            Self::Disk(why) => write!(f, "Could not save the speech model: {why}"),
            Self::Model(why) => write!(f, "The speech model failed: {why}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(why: std::io::Error) -> Self {
        Self::Disk(why)
    }
}
