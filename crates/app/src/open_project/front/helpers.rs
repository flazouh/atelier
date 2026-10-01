use super::types::Front;

/// The pane's content, given what was asked last and whether the pull requests and the tasks are shown.
pub fn front(asked: Front, pulls: bool, tasks: bool) -> Front {
    match asked {
        Front::Pulls if pulls => Front::Pulls,
        Front::Tasks if tasks => Front::Tasks,
        _ if pulls => Front::Pulls,
        _ if tasks => Front::Tasks,
        _ => Front::Editor,
    }
}
