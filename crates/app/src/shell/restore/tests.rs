use atelier_settings::{Location, OpenSession};
use super::*;
fn open(place: &str, id: &str) -> OpenSession {
    OpenSession { location: Location::Local { path: place.into() }, id: id.into(), title: format!("Session {id}"), agent: None, provider: None }
}
/// Each project opens once, in the order its first session was open.
#[test]
fn each_project_opens_once_in_order() {
    let saved = [open("/b", "1"), open("/a", "2"), open("/b", "3")];
    assert_eq!(locations(&saved), [Location::Local { path: "/b".into() }, Location::Local { path: "/a".into() }]);
}
/// A project's sessions, in the order they were open.
#[test]
fn a_projects_sessions_keep_their_order() {
    let saved = [open("/b", "1"), open("/a", "2"), open("/b", "3")];
    let ids: Vec<&str> = of(&saved, &Location::Local { path: "/b".into() }).iter().map(|o| o.id.as_str()).collect();
    assert_eq!(ids, ["1", "3"]);
}
