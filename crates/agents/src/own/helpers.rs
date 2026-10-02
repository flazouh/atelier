use std::path::Path;

pub use super::message::Secret;
use crate::session::ModelChoice;

pub(super) fn choice(id: &str, label: &str) -> ModelChoice {
    ModelChoice { id: id.into(), label: label.into() }
}

/// The Claude models to offer before the account's own list is known (see [`Anthropic::models`]).
pub fn anthropic_models() -> Vec<ModelChoice> {
    vec![choice("claude-opus-5-5", "Opus 5.5"), choice("claude-fable-5-1", "Fable 5.1"), choice("claude-sonnet-5-5", "Sonnet 5.5"), choice("claude-sonnet-5", "Sonnet 5"), choice("claude-haiku-4-5", "Haiku 4.5")]
}

/// The instructions every session starts with. It holds nothing that changes from call to call (no
/// clock), so the prompt cache keeps it.
pub fn system_prompt(root: &Path, extra: &str) -> String {
    let mut prompt = format!(
        "You are atelier's coding agent. You work in the project \"{}\" (folder: {}). You help the user \
         read, understand and change its code.\n\n\
         Work like this:\n\
         - Look before you change. Use `search` and `list` to find code, and `read` to read it.\n\
         - Change files with `edit` (an exact replacement) for a part of a file and `write` for a whole file. \
           Read a file before you edit it. Keep changes small and in the style of the code around them.\n\
         - Use `shell` to run the project's tests, build and other commands. Its output is cut when long.\n\
         - A tool result that says it failed is information: read it, then fix the cause. Do not repeat a call that failed.\n\
         - If the user does not allow an action, do not try it again. Ask what they want.\n\
         - Say briefly what you did and what you found. Do not paste files back to the user; they can see the changes.\n\
         All paths are relative to the project folder.",
        super::runner::root_name(root),
        root.display()
    );
    if !extra.trim().is_empty() {
        prompt.push_str("\n\nThe project's instructions:\n");
        prompt.push_str(extra.trim());
    }
    prompt
}

pub(super) fn env_key(name: &str) -> Option<Secret> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).map(Secret::new)
}
