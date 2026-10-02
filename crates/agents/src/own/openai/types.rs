pub const OPENAI_BASE: &str = "https://api.openai.com/v1";

pub const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";

pub(super) const ERROR_BODY: usize = 8 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Open {
    None,
    Text,
    Thinking,
}
