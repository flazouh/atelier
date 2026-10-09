use std::sync::Arc;

use atelier_capabilities::{
    Actor, CapError,
    messaging::{Feature, MessagingProvider, NewMessage, Operation},
};

use super::support::{
    Fixtures, GENERAL, GUILD, M2, M3, WHOAMI, general, last_call, message, provider, writer,
};
use crate::{DiscordConfig, DiscordMessaging, Output, fake::FakeDiscord};

fn alex() -> Actor {
    Actor::person("1400000000000000001", "Alex")
}

#[test]
fn a_provider_reads_only_unless_it_is_told_it_may_write() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let err = p
        .send(&NewMessage::to(&general(), "hello"), &alex())
        .unwrap_err();
    assert!(
        matches!(&err, CapError::Provider { code, .. } if code == "read_only"),
        "{err:?}"
    );
    let reply = p.send(&NewMessage::reply(&message(M2), &general(), "hi"), &alex());
    assert!(matches!(reply, Err(CapError::Provider { .. })));
    assert!(fixtures.calls().is_empty(), "not even a read ran");
}

#[test]
fn a_provider_lists_read_only_exactly_when_it_may_not_write() {
    let fixtures = Fixtures::the_usual();
    let read = provider(&fixtures).capabilities();
    assert!(read.has(Feature::ReadOnly), "a default block reads only");
    assert!(read.can(Operation::Send), "send is core, so it is listed");
    assert!(!read.offers(Operation::Send), "and it is not offered");
    let write = writer(&fixtures).capabilities();
    assert!(!write.has(Feature::ReadOnly), "allow_writes lifts it");
    assert!(write.offers(Operation::Send));
}

#[test]
fn reading_still_works_in_a_provider_that_may_not_write() {
    let fixtures = Fixtures::the_usual();
    assert!(provider(&fixtures).history(&general(), None, None).is_ok());
}

#[test]
fn send_runs_one_command_with_the_text_after_two_dashes() {
    let fixtures = Fixtures::the_usual();
    let sent = writer(&fixtures)
        .send(&NewMessage::to(&general(), "--hello **there**"), &alex())
        .unwrap();
    assert_eq!(
        last_call(&fixtures),
        format!("send --yes --json -- {GENERAL} --hello **there**")
    );
    assert_eq!(sent.reference, message("1425769339289600005"));
    assert_eq!(sent.created_at, 1_760_000_300_000);
    assert_eq!(sent.text, "--hello **there**");
    assert_eq!(sent.author, alex());
    assert!(sent.origin.is_none());
}

#[test]
fn a_reply_answers_the_message_asked_and_lands_under_the_root() {
    let fixtures = Fixtures::the_usual();
    let sent = writer(&fixtures)
        .send(&NewMessage::reply(&message(M3), &general(), "ok"), &alex())
        .unwrap();
    assert_eq!(
        sent.parent,
        Some(message(M2)),
        "the root, not the message answered"
    );
    assert_eq!(
        last_call(&fixtures),
        format!("reply --yes --json -- https://discord.com/channels/{GUILD}/{GENERAL}/{M3} ok")
    );
}

#[test]
fn a_reply_must_name_a_message_of_the_same_channel() {
    let fixtures = Fixtures::the_usual();
    let other: atelier_capabilities::Ref =
        format!("messaging:discord:{GUILD}:1200000000000000003:{M2}")
            .parse()
            .unwrap();
    let result = writer(&fixtures).send(&NewMessage::reply(&other, &general(), "x"), &alex());
    assert!(matches!(result, Err(CapError::Invalid { .. })));
}

#[test]
fn an_agent_message_carries_the_origin_line_and_the_origin_field() {
    let fake = FakeDiscord::new();
    let mut config = DiscordConfig::new(GUILD);
    config.allow_writes = true;
    let p = DiscordMessaging::new(fake, config);
    let agent = Actor::agent("agent-1", "Claude", "1400000000000000001");
    let sent = p
        .send(&NewMessage::to(&general(), "Build is green"), &agent)
        .unwrap();
    assert_eq!(sent.origin.as_deref(), Some("alex's agent"));
    assert_eq!(sent.author, agent, "the actor is kept");
    let history = p.history(&general(), None, None).unwrap();
    assert_eq!(
        history.items[0].text,
        "Build is green\n\n_sent by alex's agent_"
    );
}

#[test]
fn a_text_cannot_ping_everyone_here_or_a_role_and_the_tool_is_told_so_too() {
    let fake = Arc::new(FakeDiscord::new());
    let mut config = DiscordConfig::new(GUILD);
    config.allow_writes = true;
    let p = DiscordMessaging::new(fake.clone(), config);
    let text = "@everyone @HERE <@&123456789012345678> <@111111111111111111> ok";
    p.send(&NewMessage::to(&general(), text), &alex()).unwrap();
    let held = fake.texts(GENERAL).remove(0);
    for ping in ["@everyone", "@HERE", "@here", "<@&", "<@1"] {
        assert!(!held.contains(ping), "{ping} in {held:?}");
    }
    assert!(held.ends_with("ok"));
}

#[test]
fn a_name_made_of_a_ping_cannot_ping_through_the_origin_line() {
    let whoami = WHOAMI.replace("\"Alex\"", "\"@everyone\"");
    let fixtures = Fixtures::the_usual().with("whoami", Output::ok(whoami));
    let agent = Actor::agent("agent-1", "Claude", "1400000000000000001");
    writer(&fixtures)
        .send(&NewMessage::to(&general(), "hi"), &agent)
        .unwrap();
    let sent = last_call(&fixtures);
    assert!(!sent.contains("@everyone"), "{sent:?}");
    assert!(sent.contains("_sent by @\u{200b}everyone's agent_"));
}

#[test]
fn an_empty_or_too_long_text_is_invalid_and_runs_nothing() {
    let fixtures = Fixtures::the_usual();
    let p = writer(&fixtures);
    for text in ["", "  \n"] {
        assert!(matches!(
            p.send(&NewMessage::to(&general(), text), &alex()),
            Err(CapError::Invalid { .. })
        ));
    }
    let limit = "a".repeat(2000);
    let long = "a".repeat(2001);
    assert!(p.send(&NewMessage::to(&general(), &limit), &alex()).is_ok());
    let before = fixtures.calls().len();
    assert!(matches!(
        p.send(&NewMessage::to(&general(), &long), &alex()),
        Err(CapError::Invalid { .. })
    ));
    // Two units each: an emoji is two UTF-16 units, so 1001 of them are over the limit.
    let emoji = "\u{1F600}".repeat(1001);
    assert!(matches!(
        p.send(&NewMessage::to(&general(), &emoji), &alex()),
        Err(CapError::Invalid { .. })
    ));
    assert_eq!(
        fixtures.calls().len(),
        before,
        "a text over the limit never reached the tool"
    );
}

#[test]
fn an_agent_text_that_fits_only_without_the_origin_line_is_invalid() {
    let fixtures = Fixtures::the_usual();
    let agent = Actor::agent("agent-1", "Claude", "1400000000000000001");
    let result = writer(&fixtures).send(&NewMessage::to(&general(), &"a".repeat(1990)), &agent);
    assert!(matches!(result, Err(CapError::Invalid { .. })));
}
