use super::*;

#[test]
fn an_agent_keeps_the_person_it_works_for_through_json() {
    let agent = Actor::agent("a1", "Claude", "u1");
    let json = serde_json::to_string(&agent).unwrap();
    assert!(json.contains(r#""on_behalf_of":"u1""#) && json.contains(r#""kind":"agent""#));
    assert_eq!(serde_json::from_str::<Actor>(&json).unwrap(), agent);
}

#[test]
fn a_person_writes_no_on_behalf_of() {
    let json = serde_json::to_string(&Actor::person("u1", "Alex")).unwrap();
    assert!(!json.contains("on_behalf_of"));
}
