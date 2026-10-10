use crate::structs::{Bot, Playbook};

impl Playbook {
    /// Every rule the playbook breaks against the bots that are kept. Empty when it is good.
    pub fn problems(&self, bots: &[Bot]) -> Vec<String> {
        let mut out = Vec::new();
        if self.name.trim().is_empty() {
            out.push(format!("{}: the name is empty", self.id));
        }
        if self.steps.is_empty() {
            out.push(format!("{}: a playbook needs at least one step", self.id));
        }
        for (i, step) in self.steps.iter().enumerate() {
            if !bots.iter().any(|b| b.id == step.bot) {
                out.push(format!(
                    "{}: step {} names `{}`, and no such bot is kept",
                    self.id,
                    i + 1,
                    step.bot
                ));
            }
        }
        out
    }
}
