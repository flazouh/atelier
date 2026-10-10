use std::collections::HashSet;

use crate::consts::{JOB_MAX, NAME_MAX};
use crate::structs::Bot;

impl Bot {
    /// Every rule the bot breaks, in words. Empty when it is good.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.name.trim().is_empty() || self.name.chars().count() > NAME_MAX {
            out.push(format!(
                "{}: the name needs 1 to {NAME_MAX} characters",
                self.id
            ));
        }
        if self.role.trim().is_empty() {
            out.push(format!("{}: the role is empty", self.id));
        }
        if self.job.chars().count() > JOB_MAX {
            out.push(format!(
                "{}: the job is longer than {JOB_MAX} characters",
                self.id
            ));
        }
        if self.version == 0 {
            out.push(format!("{}: the version starts at 1", self.id));
        }
        if self.provider.service.trim().is_empty() {
            out.push(format!("{}: the provider has no service", self.id));
        }
        let mut seen = HashSet::new();
        for skill in &self.skills {
            if skill.trim().is_empty() || !seen.insert(skill.as_str()) {
                out.push(format!(
                    "{}: the skill `{skill}` is empty or listed twice",
                    self.id
                ));
            }
        }
        let mut seen = HashSet::new();
        for grant in &self.tools {
            if grant.connector.trim().is_empty() || !seen.insert(grant.connector.as_str()) {
                out.push(format!(
                    "{}: the connector `{}` is empty or listed twice",
                    self.id, grant.connector
                ));
            }
        }
        out
    }

    /// The bot with its version left out, to see if an edit changed anything.
    pub(in super::super) fn same_content(&self, other: &Bot) -> bool {
        let mut a = self.clone();
        a.version = other.version;
        a == *other
    }
}
