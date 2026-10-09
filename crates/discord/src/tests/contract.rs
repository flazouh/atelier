use std::{sync::Arc, time::Duration};

use atelier_capabilities::{
    CapError,
    messaging::{
        ChannelKind, ChannelQuery, Filter, MessagingProvider, NewMessage, Operation, SearchQuery,
        contract, contract::Seed,
    },
};

use super::support::GUILD;
use crate::{
    DiscordConfig, DiscordMessaging,
    fake::{DM, FakeDiscord, GENERAL, ME_ID, SAM_ID},
};

fn fake_provider(config: DiscordConfig) -> DiscordMessaging {
    DiscordMessaging::new(FakeDiscord::new(), config)
}

/// A server provider that also holds the direct messages, allowed to write, and quick to poll. The contract needs a
/// public channel and a dm in one provider.
fn contract_config() -> DiscordConfig {
    let mut config = DiscordConfig::new(GUILD);
    config.include_dms = true;
    config.allow_writes = true;
    config.poll_every = Duration::from_millis(20);
    config
}

#[test]
fn discord_passes_the_contract_on_a_fake_discord() {
    contract::run(&|| Seed {
        provider: Box::new(fake_provider(contract_config())),
        public: format!("messaging:discord:{GUILD}:{GENERAL}")
            .parse()
            .unwrap(),
        dm: format!("messaging:discord:dm:{DM}").parse().unwrap(),
    });
}

#[test]
fn a_server_provider_without_dms_lists_search_and_finds_by_word_channel_and_author() {
    let mut config = DiscordConfig::new(GUILD);
    config.allow_writes = true;
    let p = fake_provider(config);
    assert!(p.can(Operation::Search));
    let channel = format!("messaging:discord:{GUILD}:{GENERAL}")
        .parse()
        .unwrap();
    let by = p.whoami().unwrap();
    p.send(&NewMessage::to(&channel, "the zebra crossing"), &by)
        .unwrap();
    p.send(&NewMessage::to(&channel, "a quiet lunch"), &by)
        .unwrap();
    let search = |text: &str, from: Option<&str>| {
        p.search(&SearchQuery {
            text: text.into(),
            from: from.map(str::to_string),
            ..SearchQuery::default()
        })
        .unwrap()
    };
    let hits = search("zebra", None);
    assert_eq!(hits.items.len(), 1);
    assert_eq!(hits.items[0].text, "the zebra crossing");
    assert_eq!(search("zebra", Some(ME_ID)).items.len(), 1, "by its author");
    assert!(search("zebra", Some(SAM_ID)).items.is_empty(), "by another");
    assert!(
        search("zebra", Some("nobody")).items.is_empty(),
        "an author that is no id finds nothing"
    );
    let in_other_channel = p
        .search(&SearchQuery {
            text: "zebra".into(),
            channel: Some(
                format!("messaging:discord:{GUILD}:{}", crate::fake::VOICE)
                    .parse()
                    .unwrap(),
            ),
            ..SearchQuery::default()
        })
        .unwrap();
    assert!(
        in_other_channel.items.is_empty(),
        "a search stays in its channel"
    );
}

#[test]
fn a_server_provider_with_dms_does_not_list_search() {
    let p = fake_provider(contract_config());
    assert!(!p.can(Operation::Search));
    assert!(matches!(
        p.search(&SearchQuery {
            text: "x".into(),
            ..SearchQuery::default()
        }),
        Err(CapError::Unsupported { .. })
    ));
}

#[test]
fn the_dm_account_lists_its_dms_only_and_cannot_search() {
    let p = fake_provider(DiscordConfig::new("dm"));
    assert!(!p.can(Operation::Search));
    let page = p.channels(&ChannelQuery::default()).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].kind, ChannelKind::Dm);
    assert_eq!(
        page.items[0].reference.to_string(),
        format!("messaging:discord:dm:{DM}")
    );
}

#[test]
fn a_message_by_somebody_else_is_in_history_and_in_a_subscription() {
    let fake = Arc::new(FakeDiscord::new());
    let mut config = DiscordConfig::new(GUILD);
    config.poll_every = Duration::from_millis(20);
    let p = DiscordMessaging::new(fake.clone(), config);
    let channel: atelier_capabilities::Ref = format!("messaging:discord:{GUILD}:{GENERAL}")
        .parse()
        .unwrap();
    fake.post_as(GENERAL, "Sam", "already here");
    let events = p.subscribe(&Filter::default()).unwrap();
    fake.post_as(GENERAL, "Sam", "new one");
    let event = events
        .recv_timeout(Duration::from_secs(2))
        .expect("an event");
    let message = event.data.expect("the message");
    assert_eq!(
        (message.text.as_str(), message.author.name.as_str()),
        ("new one", "Sam")
    );
    assert_eq!(message.author.id, SAM_ID);
    assert!(
        events.recv_timeout(Duration::from_millis(100)).is_err(),
        "the old message is not news"
    );
    let history = p.history(&channel, None, None).unwrap();
    assert_eq!(history.items.len(), 2);
}

#[test]
fn a_service_that_fails_fails_every_call_with_the_mapped_error() {
    let fake = Arc::new(FakeDiscord::new());
    fake.fail_with(r#"{"error":"Discord session expired. Run discordcli login."}"#);
    let p = DiscordMessaging::new(fake.clone(), DiscordConfig::new(GUILD));
    assert_eq!(p.whoami().unwrap_err(), CapError::NotSignedIn);
    assert_eq!(
        p.channels(&ChannelQuery::default()).unwrap_err(),
        CapError::NotSignedIn
    );
}

#[test]
fn a_provider_that_may_not_write_sends_nothing_to_the_service() {
    let fake = Arc::new(FakeDiscord::new());
    let p = DiscordMessaging::new(fake.clone(), DiscordConfig::new(GUILD));
    let channel = format!("messaging:discord:{GUILD}:{GENERAL}")
        .parse()
        .unwrap();
    let by = atelier_capabilities::Actor::person(ME_ID, "alex");
    assert!(p.send(&NewMessage::to(&channel, "hello"), &by).is_err());
    assert!(fake.calls().is_empty(), "no command ran");
    assert!(fake.texts(GENERAL).is_empty());
}
