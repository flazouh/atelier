use std::fmt;

use crate::enums::BotsError;

impl fmt::Display for BotsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BotsError::Invalid(problems) => write!(f, "{}", problems.join("; ")),
            BotsError::Refused(why) => f.write_str(why),
            BotsError::NotFound(id) => write!(f, "nothing is kept under `{id}`"),
            BotsError::Io { path, reason } => write!(f, "could not use {path}: {reason}"),
            BotsError::Parse { path, reason } => write!(f, "{path} is not valid data: {reason}"),
        }
    }
}

impl std::error::Error for BotsError {}
