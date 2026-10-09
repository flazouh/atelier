use atelier_capabilities::{
    Actor, CapError,
    messaging::{MessagingProvider, Operation},
};

use super::support::{Fixtures, M1, general, message, provider};

#[test]
fn what_discordcli_cannot_do_is_not_listed_and_runs_no_command() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let caps = p.capabilities();
    for op in [
        Operation::Edit,
        Operation::Delete,
        Operation::React,
        Operation::MarkRead,
        Operation::Person,
        Operation::Export,
        Operation::Import,
    ] {
        assert!(!caps.can(op), "{op:?} is not listed");
    }
    let by = Actor::person("1400000000000000001", "Alex");
    let target = message(M1);
    let results = [
        p.edit(&target, "x", &by).map(drop),
        p.delete(&target, &by),
        p.react(&target, "eyes", true, &by).map(drop),
        p.mark_read(&general(), None),
        p.person(&target).map(drop),
        p.export(None).map(drop),
        p.import(&[]).map(drop),
    ];
    for result in results {
        assert!(
            matches!(result, Err(CapError::Unsupported { .. })),
            "{result:?}"
        );
    }
    assert!(fixtures.calls().is_empty(), "no command ran");
}

#[test]
fn the_provider_says_what_it_is() {
    let p = provider(&Fixtures::the_usual());
    let caps = p.capabilities();
    assert_eq!(
        (p.provider(), p.account()),
        ("discord", super::support::GUILD)
    );
    for op in [
        Operation::Channels,
        Operation::History,
        Operation::Thread,
        Operation::Send,
        Operation::Subscribe,
        Operation::Search,
    ] {
        assert!(caps.can(op), "{op:?}");
    }
    assert_eq!(caps.limits.page_max, Some(100));
}
