use super::types::Front;

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
