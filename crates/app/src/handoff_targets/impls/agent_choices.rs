use atelier_ui::menu::Branch;

use crate::providers::label;

use super::super::structs::{AgentChoices, Target};

impl AgentChoices {
    /// A leaf when the reader has nothing to choose, a branch of its providers when it has.
    pub(super) fn branch(&self) -> Branch {
        match self.choices.as_slice() {
            [] => Branch::leaf(self.target(None).id(), self.name.clone()),
            [only] => Branch::leaf(self.target(Some(only.clone())).id(), self.name.clone()),
            choices => Branch::with(self.backend.clone(), self.name.clone(), choices.iter().map(|choice| self.provider_leaf(choice)).collect()),
        }
    }

    fn provider_leaf(&self, choice: &crate::providers::Choice) -> Branch {
        Branch::leaf(self.target(Some(choice.clone())).id(), label(choice, &self.accounts))
    }

    fn target(&self, provider: Option<crate::providers::Choice>) -> Target {
        Target { backend: self.backend.clone(), provider }
    }
}
