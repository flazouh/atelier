use crate::providers::Choice;

use super::super::consts::SEPARATOR;
use super::super::structs::Target;

impl Target {
    /// The id a menu row carries for it: the backend, then the provider's key when it has one.
    pub fn id(&self) -> String {
        match &self.provider {
            Some(choice) => format!("{}{SEPARATOR}{}", self.backend, choice.key()),
            None => self.backend.clone(),
        }
    }

    /// The target an id names, or `None` when it names a provider this build does not know.
    pub fn parse(id: &str) -> Option<Self> {
        match id.split_once(SEPARATOR) {
            None => Some(Self { backend: id.into(), provider: None }),
            Some((backend, key)) => Some(Self { backend: backend.into(), provider: Some(Choice::from_key(key)?) }),
        }
    }
}
