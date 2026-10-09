use serde::{Deserialize, Serialize};

/// Who made a change. A person is a person; an agent may work for a person, whose rights it never exceeds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub kind: ActorKind,
    pub id: String,
    pub name: String,
    /// The id of the person an agent works for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_behalf_of: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    Person,
    Agent,
}

impl Actor {
    pub fn person(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            kind: ActorKind::Person,
            id: id.into(),
            name: name.into(),
            on_behalf_of: None,
        }
    }

    pub fn agent(
        id: impl Into<String>,
        name: impl Into<String>,
        on_behalf_of: impl Into<String>,
    ) -> Self {
        Self {
            kind: ActorKind::Agent,
            id: id.into(),
            name: name.into(),
            on_behalf_of: Some(on_behalf_of.into()),
        }
    }
}

#[cfg(test)]
mod tests;
