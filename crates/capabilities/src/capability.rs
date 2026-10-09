use serde::{Deserialize, Serialize};

/// A call a tasks provider may offer. The core ones every provider has; the rest are optional.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    List,
    Get,
    Create,
    Update,
    Comment,
    Activity,
    Subscribe,
    CreateMany,
    Delete,
    Link,
    Labels,
    Projects,
    Statuses,
    Export,
    Import,
}

/// Something a provider's service has, which changes what the screen may show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    CustomStates,
    Subtasks,
    Relations,
    Estimates,
    DueDates,
    Cycles,
    Attachments,
    Webhooks,
    Reactions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    Oauth,
    Token,
    BrowserSession,
    None,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_max: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_minute: Option<u32>,
}

/// What a provider can do. The screen and the agent tools offer only what is listed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub operations: Vec<Operation>,
    pub features: Vec<Feature>,
    pub limits: Limits,
    pub auth: Vec<AuthKind>,
}

impl Capabilities {
    /// The operations every tasks provider has.
    pub const CORE: [Operation; 9] = [
        Operation::List,
        Operation::Get,
        Operation::Create,
        Operation::Update,
        Operation::Comment,
        Operation::Activity,
        Operation::Subscribe,
        Operation::Labels,
        Operation::Projects,
    ];

    pub fn can(&self, operation: Operation) -> bool {
        self.operations.contains(&operation)
    }

    pub fn has(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_and_has_read_the_lists() {
        let c = Capabilities {
            operations: vec![Operation::List],
            features: vec![Feature::Cycles],
            ..Default::default()
        };
        assert!(c.can(Operation::List) && !c.can(Operation::Delete));
        assert!(c.has(Feature::Cycles) && !c.has(Feature::Estimates));
    }

    #[test]
    fn names_are_snake_case_in_json() {
        let json = serde_json::to_string(&Capabilities {
            operations: vec![Operation::CreateMany],
            auth: vec![AuthKind::BrowserSession],
            ..Default::default()
        })
        .unwrap();
        assert!(
            json.contains("create_many") && json.contains("browser_session"),
            "{json}"
        );
    }
}
