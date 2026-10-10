use crate::starter_crew;

fn bolt() -> crate::Bot {
    starter_crew()
        .into_iter()
        .find(|b| b.id.as_str() == "bolt")
        .unwrap()
}

#[test]
fn a_starter_bot_has_no_problem() {
    assert!(bolt().problems().is_empty());
}

#[test]
fn every_broken_rule_is_listed_together() {
    let mut bot = bolt();
    bot.name = "  ".into();
    bot.version = 0;
    bot.skills = vec!["eng-tdd".into(), "eng-tdd".into()];
    bot.provider.service.clear();
    let problems = bot.problems();
    assert_eq!(problems.len(), 4, "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("the name")));
    assert!(problems.iter().any(|p| p.contains("`eng-tdd`")));
}

#[test]
fn a_connector_listed_twice_is_a_problem() {
    let mut bot = bolt();
    let grant = crate::ToolGrant {
        connector: "sentry".into(),
        access: crate::Access::Read,
    };
    bot.tools = vec![grant.clone(), grant];
    assert!(bot.problems().iter().any(|p| p.contains("`sentry`")));
}
