//! Which tool calls run at once, which wait for an answer, and which never run. Pure: the mode, what the
//! call does, and the rules the reader added decide. This is lathe's own logic; no backend supplies it.
use std::collections::HashSet;

use super::tools::Access;
use crate::session::PermissionMode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    /// Ask the reader and wait.
    Ask,
    /// Refuse. The reason goes to the model, which reads it and goes on.
    Deny(String),
}

/// Tools the reader allowed for good ("Always allow"), by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rules {
    always: HashSet<String>,
}

impl Rules {
    pub fn new(always: impl IntoIterator<Item = String>) -> Self {
        Self { always: always.into_iter().collect() }
    }

    pub fn allow_always(&mut self, tool: &str) {
        self.always.insert(tool.to_string());
    }

    pub fn allows(&self, tool: &str) -> bool {
        self.always.contains(tool)
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.always.iter().cloned().collect();
        names.sort();
        names
    }
}

/// What to do with a call of `tool`, which does `access`, in `mode`.
///
/// - `Plan` allows looking and nothing else.
/// - `Ask` allows looking and asks about the rest.
/// - `AcceptEdits` also allows changing files, and asks about running processes.
/// - `Auto` is `AcceptEdits` here: this agent has no judge of its own to decide which commands are safe.
/// - `Bypass` allows all.
///
/// A rule allows a tool in every mode but `Plan`.
pub fn decide(mode: PermissionMode, access: Access, tool: &str, rules: &Rules) -> Verdict {
    if mode == PermissionMode::Plan {
        return match access {
            Access::Read => Verdict::Allow,
            _ => Verdict::Deny("Plan mode: this session may look at the project but not change it. Say what you would do instead.".into()),
        };
    }
    if mode == PermissionMode::Bypass || access == Access::Read || rules.allows(tool) {
        return Verdict::Allow;
    }
    match (mode, access) {
        (PermissionMode::AcceptEdits | PermissionMode::Auto, Access::Edit) => Verdict::Allow,
        _ => Verdict::Ask,
    }
}
