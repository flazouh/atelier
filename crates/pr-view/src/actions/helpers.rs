use atelier_ui::merge::{Action, Choice, MergeMethod as UiMethod, UpdateWay};
use atelier_forge::{MergeMethod, MergeRequest, UpdateMethod};

use super::types::Ask;

pub(super) fn method(m: UiMethod) -> MergeMethod {
    match m {
        UiMethod::Merge => MergeMethod::Merge,
        UiMethod::Squash => MergeMethod::Squash,
        UiMethod::Rebase => MergeMethod::Rebase,
    }
}

/// What a press asks of the forge. A merge sends the commit's words for a squash and leaves them to the
/// forge otherwise. `head` is the commit the reader saw: the forge refuses a merge or an update if the
/// branch moved.
pub fn ask(action: Action, choice: Choice, title: &str, message: &str, head: &str) -> Ask {
    let (chosen, when_ready) = match action {
        Action::Merge(m) | Action::BypassAndMerge(m) => (m, false),
        Action::MergeWhenReady(m) => (m, true),
        Action::AddToQueue => (choice.method, false),
        Action::ReadyForReview => return Ask::Ready,
        Action::UpdateBranch(way) => {
            let method = match way {
                UpdateWay::Merge => UpdateMethod::Merge,
                UpdateWay::Rebase => UpdateMethod::Rebase,
            };
            return Ask::UpdateBranch { method, expected_head: head.to_string() };
        }
        Action::CancelMergeWhenReady => return Ask::CancelAutoMerge,
        Action::RemoveFromQueue => return Ask::Dequeue,
        Action::DeleteBranch => return Ask::DeleteBranch,
        Action::Revert => return Ask::Revert,
    };
    let squash = chosen == UiMethod::Squash;
    Ask::Merge(MergeRequest {
        method: method(chosen),
        title: (squash && !title.trim().is_empty()).then(|| title.trim().to_string()),
        message: (squash && !message.trim().is_empty()).then(|| message.trim().to_string()),
        expected_head: Some(head.to_string()),
        when_ready,
        delete_branch: choice.delete_branch,
    })
}
