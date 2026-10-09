use atelier_agents::usage_history::Tokens;
/// The tokens the dashboard counts: what was sent, written and answered. Reading the cache back is left out, because
/// one long session reads its cache over and over and would bury everything else; it shows apart, in a session's split.
pub fn counted(tokens: &Tokens) -> u64 {
    tokens.input + tokens.output + tokens.cache_write
}
