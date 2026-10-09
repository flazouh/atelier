use atelier_capabilities::{
    ActorKind,
    messaging::{ChannelKind, ChannelQuery, MessagingProvider},
};
use atelier_capabilities::Actor;

use super::*;
use crate::capability_hub::CapabilityHub;

fn hub() -> CapabilityHub {
    CapabilityHub::new(Actor::person("alex", "Alex"), None, false)
}

#[test]
fn nothing_is_registered_without_the_variable() {
    for value in [None, Some(""), Some("0"), Some("no"), Some("false")] {
        let hub = hub();
        assert!(!register(&hub, value), "{value:?} asks for nothing");
        assert!(hub.messaging_providers().is_empty(), "{value:?} leaves the registry empty");
    }
}

#[test]
fn the_variable_registers_one_seeded_account() {
    for value in ["1", "true", "yes"] {
        let hub = hub();
        assert!(register(&hub, Some(value)));
        let providers = hub.messaging_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!((providers[0].provider(), providers[0].account()), ("memory", "demo"));
    }
}

#[test]
fn the_seed_has_something_of_each_thing_the_screen_draws() {
    let demo = seeded();
    let channels = demo.channels(&ChannelQuery::default()).unwrap().items;
    let kinds: Vec<_> = channels.iter().map(|c| c.kind).collect();
    assert_eq!(kinds, [ChannelKind::Public, ChannelKind::Private, ChannelKind::Dm]);
    assert!(channels.iter().any(|c| c.unread.is_some_and(|n| n > 0)), "one channel has unread");
    let mut messages = Vec::new();
    for channel in &channels {
        let mut cursor = None;
        loop {
            let page = demo.history(&channel.reference, cursor.as_deref(), None).unwrap();
            for root in page.items {
                if root.reply_count > 0 {
                    messages.extend(demo.thread(&root.reference, None).unwrap().items.into_iter().skip(1));
                }
                messages.push(root);
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
    }
    assert!((10..=16).contains(&messages.len()), "about a dozen messages, got {}", messages.len());
    assert!(messages.iter().any(|m| m.parent.is_some()), "a reply");
    assert!(messages.iter().any(|m| !m.reactions.is_empty()), "a reaction");
    assert!(messages.iter().any(|m| !m.attachments.is_empty()), "a file");
    assert!(messages.iter().any(|m| m.author.kind == ActorKind::Agent && m.origin.is_some()), "an agent's message with its origin");
    let general = &channels[0].reference;
    let first_page = demo.history(general, None, None).unwrap();
    assert!(first_page.next_cursor.is_some(), "the first channel pages, so Load older shows");
}
