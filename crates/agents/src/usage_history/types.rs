/// Which agent wrote the logs of an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Provider {
    Claude,
    Codex,
}
