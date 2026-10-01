use atelier_forge::{ForgeError, Involved, PullRef};

/// What the list tells its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListEvent {
    Open(PullRef),
}

pub(super) enum Msg {
    Cached(Option<(Vec<Involved>, u64)>, std::collections::HashMap<(String, u64), u64>),
    Read(Result<Vec<Involved>, ForgeError>),
}
