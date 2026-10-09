use serde::{Deserialize, Serialize};

/// How a capability call fails. The screen shows each one in its own way: offline and signed out ask the person to act,
/// rate limited waits, unsupported hides the control.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapError {
    NotFound {
        what: String,
    },
    Invalid {
        field: String,
    },
    /// The version sent is old. `current` is the thing as it is now.
    Conflict {
        current: serde_json::Value,
    },
    Offline,
    NotSignedIn,
    RateLimited {
        retry_after_ms: u64,
    },
    Unsupported {
        feature: String,
    },
    Storage {
        message: String,
    },
    Provider {
        code: String,
        message: String,
    },
}

impl CapError {
    pub fn not_found(what: impl Into<String>) -> Self {
        Self::NotFound { what: what.into() }
    }

    pub fn invalid(field: impl Into<String>) -> Self {
        Self::Invalid {
            field: field.into(),
        }
    }

    pub fn unsupported(feature: impl Into<String>) -> Self {
        Self::Unsupported {
            feature: feature.into(),
        }
    }
}

impl std::fmt::Display for CapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { what } => write!(f, "{what} was not found"),
            Self::Invalid { field } => write!(f, "{field} is not valid"),
            Self::Conflict { .. } => write!(f, "it changed since you read it"),
            Self::Offline => write!(f, "there is no connection"),
            Self::NotSignedIn => write!(f, "you are not signed in"),
            Self::RateLimited { retry_after_ms } => write!(
                f,
                "too many requests, try again in {} s",
                retry_after_ms.div_ceil(1000)
            ),
            Self::Unsupported { feature } => write!(f, "this provider cannot {feature}"),
            Self::Storage { message } => write!(f, "the store failed: {message}"),
            Self::Provider { code, message } => write!(f, "{code}: {message}"),
        }
    }
}

impl std::error::Error for CapError {}

pub type CapResult<T> = Result<T, CapError>;

#[cfg(test)]
mod tests;
