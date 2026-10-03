use gpui_kit::Task;

use super::super::enums::SignIn;
use super::Run;

/// What a session keeps of its agent's sign-in.
#[derive(Default)]
pub struct Signer {
    pub(in super::super) state: SignIn,
    pub(in super::super) run: Option<Run>,
    /// The named account that holds a session that was resumed without a provider, once it is found.
    pub(in super::super) holder: Option<String>,
    /// Looking for the holder, started once.
    pub(in super::super) looking: Option<Task<()>>,
}
