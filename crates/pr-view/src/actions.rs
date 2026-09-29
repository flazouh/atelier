//! What a press in the merge box asks of the forge, or why it cannot. Pure: the action and the choice in,
//! a request out.
use beui::merge::{Action, Choice, MergeMethod as UiMethod};
use lathe_forge::{MergeMethod, MergeRequest};

fn method(m: UiMethod) -> MergeMethod {
    match m {
        UiMethod::Merge => MergeMethod::Merge,
        UiMethod::Squash => MergeMethod::Squash,
        UiMethod::Rebase => MergeMethod::Rebase,
    }
}

/// The forge's merge request for a press. The commit's words are sent for a squash and left to the forge
/// otherwise. `expected_head` is the commit the reader saw: the forge refuses if the branch moved.
pub fn merge_request(action: Action, choice: Choice, title: &str, message: &str, head: &str) -> Result<MergeRequest, String> {
    let (chosen, when_ready) = match action {
        Action::Merge(m) | Action::BypassAndMerge(m) => (m, false),
        Action::MergeWhenReady(m) => (m, true),
        Action::AddToQueue => (choice.method, false),
        Action::ReadyForReview => return Err("Marking it ready for review is not built yet.".into()),
        Action::UpdateBranch(_) => return Err("Updating the branch is not built yet.".into()),
        Action::CancelMergeWhenReady | Action::RemoveFromQueue => return Err("Taking it back out is not built yet.".into()),
        Action::DeleteBranch => return Err("Deleting the branch alone is not built yet.".into()),
        Action::Revert => return Err("Revert is not built yet.".into()),
    };
    let squash = chosen == UiMethod::Squash;
    Ok(MergeRequest {
        method: method(chosen),
        title: (squash && !title.trim().is_empty()).then(|| title.trim().to_string()),
        message: (squash && !message.trim().is_empty()).then(|| message.trim().to_string()),
        expected_head: Some(head.to_string()),
        when_ready,
        delete_branch: choice.delete_branch,
    })
}
