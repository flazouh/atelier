/// Why a bot, a playbook, a run or a note could not be read, kept, made or moved.
#[derive(Debug)]
pub enum BotsError {
    /// The data breaks a rule. Every problem is listed.
    Invalid(Vec<String>),
    /// The rules of a run do not allow this move. It says why.
    Refused(String),
    /// No bot, playbook or run has this id.
    NotFound(String),
    /// A file could not be read or written.
    Io { path: String, reason: String },
    /// A file is not valid data.
    Parse { path: String, reason: String },
}
