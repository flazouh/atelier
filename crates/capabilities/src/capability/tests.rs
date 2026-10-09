use super::*;

#[test]
fn can_and_has_read_the_lists() {
    let c = Capabilities {
        operations: vec![Operation::List],
        features: vec![Feature::Cycles],
        ..Default::default()
    };
    assert!(c.can(Operation::List) && !c.can(Operation::Delete));
    assert!(c.has(Feature::Cycles) && !c.has(Feature::Estimates));
}

#[test]
fn names_are_snake_case_in_json() {
    let json = serde_json::to_string(&Capabilities {
        operations: vec![Operation::CreateMany],
        auth: vec![AuthKind::BrowserSession],
        ..Default::default()
    })
    .unwrap();
    assert!(
        json.contains("create_many") && json.contains("browser_session"),
        "{json}"
    );
}
