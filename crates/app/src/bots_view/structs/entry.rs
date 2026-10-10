use atelier_bots::{Bot, MemoryNote};

/// One bot, with the notes of its own memory layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub bot: Bot,
    pub notes: Vec<MemoryNote>,
}
