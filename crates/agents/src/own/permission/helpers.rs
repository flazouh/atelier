use super::super::tools::Access;
use crate::session::PermissionMode;
use super::structs::Rules;
use super::types::Verdict;

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
