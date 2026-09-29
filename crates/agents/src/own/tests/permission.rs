use crate::{
    own::{
        permission::{Rules, Verdict, decide},
        tools::Access,
    },
    session::PermissionMode::{self, *},
};

fn v(mode: PermissionMode, access: Access) -> Verdict {
    decide(mode, access, "tool", &Rules::default())
}

#[test]
fn every_mode_decides_every_kind_of_call() {
    use Access::*;
    let denied = |mode, access| matches!(v(mode, access), Verdict::Deny(_));
    for (mode, read, edit, execute) in [
        (Ask, Verdict::Allow, Verdict::Ask, Verdict::Ask),
        (AcceptEdits, Verdict::Allow, Verdict::Allow, Verdict::Ask),
        (Auto, Verdict::Allow, Verdict::Allow, Verdict::Ask),
        (Bypass, Verdict::Allow, Verdict::Allow, Verdict::Allow),
    ] {
        assert_eq!((v(mode, Read), v(mode, Edit), v(mode, Execute)), (read, edit, execute), "{mode:?}");
    }
    assert_eq!(v(Plan, Read), Verdict::Allow);
    assert!(denied(Plan, Edit) && denied(Plan, Execute), "plan mode changes nothing");
}

#[test]
fn a_rule_allows_a_tool_in_every_mode_but_plan() {
    let mut rules = Rules::default();
    rules.allow_always("write");
    assert_eq!(decide(Ask, Access::Edit, "write", &rules), Verdict::Allow);
    assert_eq!(decide(Ask, Access::Edit, "edit", &rules), Verdict::Ask, "only the tool that was allowed");
    assert_eq!(decide(Ask, Access::Execute, "shell", &rules), Verdict::Ask);
    assert!(matches!(decide(Plan, Access::Edit, "write", &rules), Verdict::Deny(_)), "a rule does not lift plan mode");
    assert_eq!(rules.names(), vec!["write"]);
    assert_eq!(Rules::new(["shell".to_string()]).names(), vec!["shell"]);
}

#[test]
fn the_reason_a_denial_gives_is_words_the_model_can_act_on() {
    let Verdict::Deny(why) = v(Plan, Access::Edit) else { panic!() };
    assert!(why.contains("Plan mode") && why.contains("instead"));
}
