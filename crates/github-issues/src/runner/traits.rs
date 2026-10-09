use super::{Call, Failure, Reply};

/// The one way the provider talks to GitHub. It is `gh api` in the app and a fake in tests, so the crate never holds
/// a token: the sign-in is `gh`'s own.
pub trait Gh: Send + Sync {
    fn send(&self, call: &Call) -> Result<Reply, Failure>;
}
