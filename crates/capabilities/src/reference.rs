use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

/// The stable name of a thing, the same text everywhere: `tasks:linear:acme:ENG-123`, `plugin:sentry:acme:WEB-1A`.
/// The id part may hold colons; the other three parts may not.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Ref {
    /// The capability (`tasks`, `git`, `messaging`, `mail`), or `plugin` for a plugin that has none.
    pub capability: String,
    pub provider: String,
    pub account: String,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefError {
    /// Fewer than four parts.
    Shape(String),
    /// A part is empty or holds a character it may not.
    Part { part: &'static str, text: String },
}

impl fmt::Display for RefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape(text) => {
                write!(f, "`{text}` is not <capability>:<provider>:<account>:<id>")
            }
            Self::Part { part, text } => write!(f, "`{text}` is not a valid {part}"),
        }
    }
}

impl std::error::Error for RefError {}

fn word(text: &str, extra: &[char]) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || extra.contains(&c))
}

impl Ref {
    pub fn new(
        capability: &str,
        provider: &str,
        account: &str,
        id: &str,
    ) -> Result<Self, RefError> {
        let check = |part: &'static str, text: &str, ok: bool| {
            if ok {
                Ok(())
            } else {
                Err(RefError::Part {
                    part,
                    text: text.to_string(),
                })
            }
        };
        check(
            "capability",
            capability,
            word(capability, &['_', '-']) && !capability.chars().any(|c| c.is_ascii_uppercase()),
        )?;
        check(
            "provider",
            provider,
            word(provider, &['_', '-']) && !provider.chars().any(|c| c.is_ascii_uppercase()),
        )?;
        check(
            "account",
            account,
            word(account, &['_', '-', '.', '@', '+']),
        )?;
        check("id", id, !id.is_empty())?;
        Ok(Self {
            capability: capability.into(),
            provider: provider.into(),
            account: account.into(),
            id: id.into(),
        })
    }
}

impl FromStr for Ref {
    type Err = RefError;

    fn from_str(text: &str) -> Result<Self, RefError> {
        let mut parts = text.splitn(4, ':');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(capability), Some(provider), Some(account), Some(id)) => {
                Self::new(capability, provider, account, id)
            }
            _ => Err(RefError::Shape(text.to_string())),
        }
    }
}

impl fmt::Display for Ref {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}",
            self.capability, self.provider, self.account, self.id
        )
    }
}

impl TryFrom<String> for Ref {
    type Error = RefError;
    fn try_from(text: String) -> Result<Self, RefError> {
        text.parse()
    }
}

impl From<Ref> for String {
    fn from(r: Ref) -> String {
        r.to_string()
    }
}

#[cfg(test)]
mod tests;
