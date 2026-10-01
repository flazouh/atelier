use gpui_kit::SharedString;
use atelier_forge::PullRef;

/// The context the form's fields sit in, for ⌘↵.
pub const CONTEXT: &str = "PullForm";

pub enum FormEvent {
    Opened(PullRef),
    /// The branch already has this open pull request; the form offers no create.
    Existing(PullRef),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormStage {
    /// Reading the branch, the repository and the bases.
    Reading,
    Open,
    Opening,
    Opened(PullRef),
    /// The branch already has this open pull request, with its title.
    Existing(PullRef, SharedString),
    /// The form cannot open a pull request here: the words say why.
    Failed(SharedString),
}
