//! What the right pane shows: the review, the pull requests, or the editor. The reader's last ask
//! wins while it is there, so Cmd+Shift+P during a review brings the pull requests up, and hiding
//! them brings the review back.

/// What the right pane holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Front {
    #[default]
    Editor,
    Review,
    Pulls,
    Tasks,
}

/// The pane's content, given what was asked last and whether a review is open and the pull requests
/// are shown.
pub fn front(asked: Front, review: bool, pulls: bool, tasks: bool) -> Front {
    match asked {
        Front::Pulls if pulls => Front::Pulls,
        Front::Tasks if tasks => Front::Tasks,
        Front::Review if review => Front::Review,
        _ if review => Front::Review,
        _ if pulls => Front::Pulls,
        _ if tasks => Front::Tasks,
        _ => Front::Editor,
    }
}

#[cfg(test)]
mod tests;
