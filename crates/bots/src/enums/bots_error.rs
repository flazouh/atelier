/// Why a bot, a playbook or a note could not be read, kept or made.
#[derive(Debug)]
pub enum BotsError {
    /// The data breaks a rule. Every problem is listed.
    Invalid(Vec<String>),
    /// No bot or playbook has this id.
    NotFound(String),
    /// A file could not be read or written.
    Io { path: String, reason: String },
    /// A file is not valid data.
    Parse { path: String, reason: String },
}
