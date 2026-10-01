use atelier_agents::session::{SessionId, SessionSummary};
use super::*;
fn past(id: &str, updated: u64) -> SessionSummary {
    SessionSummary { id: SessionId::new(id), title: format!("Session {id}"), updated: Some(updated) }
}
/// A session that is open shows once, as the open one: the past list leaves it out.
#[test]
fn an_open_session_is_not_a_past_one_too() {
    let open = [SessionId::new("b")];
    let left: Vec<String> = not_open(vec![past("a", 3), past("b", 2), past("c", 1)], &open).into_iter().map(|p| p.id.0).collect();
    assert_eq!(left, ["a", "c"]);
}
/// The last activity the list knows of an open session, for its row's place and its stamp.
#[test]
fn an_open_session_takes_its_last_activity_from_the_list() {
    let list = [past("a", 3), past("b", 2)];
    assert_eq!(last_activity(&list, &SessionId::new("b")), Some(2));
    assert_eq!(last_activity(&list, &SessionId::new("z")), None);
}
