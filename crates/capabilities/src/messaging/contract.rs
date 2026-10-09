//! The contract every messaging provider passes. A provider's crate calls [`run`] from one test with a function that makes
//! a fresh, empty provider with one public channel and one dm. Each check panics with a plain message when the provider
//! breaks it.
//!
//! The checks are the ones in `docs/capabilities/messaging-v1.md`, section 9.
use std::{collections::HashSet, time::Duration};

use super::{
    helpers::split_message,
    structs::{ChannelQuery, Filter, NewMessage, SearchQuery},
    traits::MessagingProvider,
    types::{ChannelKind, EventKind, Feature, Operation},
};
use crate::{Actor, ActorKind, CapError, Ref};

/// A fresh provider and the two channels the checks use. Both channels are empty.
pub struct Seed {
    pub provider: Box<dyn MessagingProvider>,
    /// A public channel.
    pub public: Ref,
    /// A direct message.
    pub dm: Ref,
}

/// Makes a fresh seed.
pub type Make<'a> = &'a dyn Fn() -> Seed;

pub fn run(make: Make) {
    send_then_history(make);
    replies_go_to_the_thread(make);
    pages_do_not_repeat_or_skip(make);
    capabilities_are_honest(make);
    edit_and_delete(make);
    reactions_count_once(make);
    the_actor_is_kept(make);
    subscribe_delivers_each_post_once_in_order(make);
    references_parse_back_and_unknown_things_are_not_found(make);
    channels_and_search_filter(make);
    mark_read(make);
}

fn me(p: &dyn MessagingProvider) -> Actor {
    p.whoami().expect("whoami")
}

fn text_of(items: &[super::structs::Message]) -> Vec<&str> {
    items.iter().map(|m| m.text.as_str()).collect()
}

/// 1. `send` then `history` returns what was sent, newest first. An empty text is invalid.
pub fn send_then_history(make: Make) {
    let Seed {
        provider: p,
        public,
        ..
    } = make();
    let by = me(&*p);
    let first = p
        .send(&NewMessage::to(&public, "hello"), &by)
        .expect("send");
    let second = p
        .send(&NewMessage::to(&public, "second"), &by)
        .expect("send");
    assert_eq!(first.text, "hello", "the text");
    assert_eq!(first.channel, public, "the channel");
    assert_eq!(first.author.id, by.id, "the author");
    assert!(first.parent.is_none(), "a top-level message has no parent");
    let page = p.history(&public, None, None).expect("history");
    assert_eq!(text_of(&page.items), ["second", "hello"], "newest first");
    assert_eq!(
        (
            &page.items[1].reference,
            &page.items[1].text,
            &page.items[1].channel
        ),
        (&first.reference, &first.text, &first.channel),
        "history returns what send returned"
    );
    assert_eq!(page.items[0].reference, second.reference);
    for blank in ["", "   "] {
        assert!(
            matches!(
                p.send(&NewMessage::to(&public, blank), &by),
                Err(CapError::Invalid { .. })
            ),
            "an empty text is invalid"
        );
    }
}

/// 2. A reply is in `thread`, not in `history`. The root counts it. A reply to a reply goes under the root.
pub fn replies_go_to_the_thread(make: Make) {
    let Seed {
        provider: p,
        public,
        ..
    } = make();
    let by = me(&*p);
    let root = p.send(&NewMessage::to(&public, "question"), &by).unwrap();
    let reply = p
        .send(&NewMessage::reply(&root.reference, &public, "answer"), &by)
        .expect("reply");
    assert_eq!(reply.parent.as_ref(), Some(&root.reference), "the parent");
    let nested = p
        .send(&NewMessage::reply(&reply.reference, &public, "thanks"), &by)
        .expect("a reply to a reply");
    assert_eq!(
        nested.parent.as_ref(),
        Some(&root.reference),
        "a reply to a reply goes under the root"
    );
    let history = p.history(&public, None, None).unwrap();
    assert_eq!(
        text_of(&history.items),
        ["question"],
        "replies are not in history"
    );
    assert_eq!(
        history.items[0].reply_count, 2,
        "the root counts its replies"
    );
    let thread = p.thread(&root.reference, None).expect("thread");
    assert_eq!(
        text_of(&thread.items),
        ["question", "answer", "thanks"],
        "the root, then the replies oldest first"
    );
    let from_reply = p.thread(&reply.reference, None).expect("thread of a reply");
    assert_eq!(from_reply.items, thread.items, "a reply names its root");
}

/// 3. Pages do not repeat or skip a message while the data does not change.
pub fn pages_do_not_repeat_or_skip(make: Make) {
    let Seed {
        provider: p,
        public,
        ..
    } = make();
    let by = me(&*p);
    for n in 0..7 {
        p.send(&NewMessage::to(&public, &format!("m{n}")), &by)
            .unwrap();
    }
    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..20 {
        let page = p.history(&public, cursor.as_deref(), Some(3)).unwrap();
        assert!(page.items.len() <= 3, "a page keeps to its limit");
        seen.extend(page.items.iter().map(|m| m.text.clone()));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert!(cursor.is_none(), "the pages end");
    assert_eq!(
        seen,
        ["m6", "m5", "m4", "m3", "m2", "m1", "m0"],
        "every message once, newest first"
    );
}

/// 4. `capabilities` is honest: the core is there, and each unlisted operation returns `Unsupported`.
pub fn capabilities_are_honest(make: Make) {
    let Seed {
        provider: p,
        public,
        dm,
    } = make();
    let caps = p.capabilities();
    for op in super::structs::MessagingCapabilities::CORE {
        assert!(caps.can(op), "{op:?} is a core operation");
    }
    let by = me(&*p);
    let message = p.send(&NewMessage::to(&public, "probe"), &by).unwrap();
    let batch: Vec<super::structs::Envelope> = Vec::new();
    for op in super::structs::MessagingCapabilities::ALL {
        if caps.can(op) {
            continue;
        }
        let result = match op {
            Operation::Search => p
                .search(&SearchQuery {
                    text: "probe".into(),
                    ..SearchQuery::default()
                })
                .map(drop),
            Operation::Edit => p.edit(&message.reference, "x", &by).map(drop),
            Operation::Delete => p.delete(&message.reference, &by),
            Operation::React => p.react(&message.reference, "eyes", true, &by).map(drop),
            Operation::MarkRead => p.mark_read(&dm, None),
            Operation::Person => p.person(&by_ref(&public, &by)).map(drop),
            Operation::Export => p.export(None).map(drop),
            Operation::Import => p.import(&batch).map(drop),
            _ => continue,
        };
        assert!(
            matches!(result, Err(CapError::Unsupported { .. })),
            "{op:?} is not listed, so it is unsupported, got {result:?}"
        );
    }
    if caps.has(Feature::Reactions) {
        assert!(caps.can(Operation::React), "reactions need react");
    }
    if caps.has(Feature::Edits) {
        assert!(caps.can(Operation::Edit), "edits need edit");
    }
    if caps.can(Operation::Person) {
        let person = p.person(&by_ref(&public, &by)).expect("person");
        assert_eq!(
            person.reference,
            by_ref(&public, &by),
            "the signed-in person by reference"
        );
    }
}

fn by_ref(like: &Ref, by: &Actor) -> Ref {
    super::helpers::person_ref(like, &by.id)
}

/// 5. `edit` changes the text and sets `edited_at`. `delete` removes the message from history and thread.
pub fn edit_and_delete(make: Make) {
    let Seed {
        provider: p,
        public,
        ..
    } = make();
    let by = me(&*p);
    let root = p.send(&NewMessage::to(&public, "draft"), &by).unwrap();
    let reply = p
        .send(&NewMessage::reply(&root.reference, &public, "reply"), &by)
        .unwrap();
    if p.can(Operation::Edit) {
        let edited = p.edit(&root.reference, "final", &by).expect("edit");
        assert_eq!(edited.text, "final", "the new text");
        assert!(edited.edited_at.is_some(), "edited_at is set");
        assert!(root.edited_at.is_none(), "a new message is not edited");
        let read = p.history(&public, None, None).unwrap();
        assert_eq!(read.items[0].text, "final", "history shows the edit");
        assert!(
            matches!(
                p.edit(&root.reference, " ", &by),
                Err(CapError::Invalid { .. })
            ),
            "an empty edit is invalid"
        );
    }
    if p.can(Operation::Delete) {
        p.delete(&reply.reference, &by).expect("delete");
        let thread = p.thread(&root.reference, None).unwrap();
        assert_eq!(thread.items.len(), 1, "the reply is gone from the thread");
        assert_eq!(thread.items[0].reply_count, 0, "and the root forgets it");
        p.delete(&root.reference, &by).expect("delete");
        assert!(
            p.history(&public, None, None).unwrap().items.is_empty(),
            "the message is gone from history"
        );
        assert!(
            matches!(
                p.delete(&root.reference, &by),
                Err(CapError::NotFound { .. })
            ),
            "a message deleted twice is not found"
        );
        if p.can(Operation::Edit) {
            assert!(
                matches!(
                    p.edit(&root.reference, "back", &by),
                    Err(CapError::NotFound { .. })
                ),
                "a deleted message cannot be edited"
            );
        }
    }
}

/// 6. A reaction by the same actor counts once. Removing it removes it.
pub fn reactions_count_once(make: Make) {
    let Seed {
        provider: p,
        public,
        ..
    } = make();
    if !p.can(Operation::React) {
        return;
    }
    let by = me(&*p);
    let m = p.send(&NewMessage::to(&public, "nice"), &by).unwrap();
    p.react(&m.reference, "thumbsup", true, &by).expect("react");
    let again = p
        .react(&m.reference, "thumbsup", true, &by)
        .expect("react twice");
    assert_eq!(again.reactions.len(), 1, "one emoji");
    assert_eq!(again.reactions[0].name, "thumbsup");
    assert_eq!(
        again.reactions[0].count, 1,
        "twice by one actor counts once"
    );
    assert!(again.reactions[0].me, "the signed-in person reacted");
    let gone = p
        .react(&m.reference, "thumbsup", false, &by)
        .expect("remove");
    assert!(gone.reactions.is_empty(), "the reaction is removed");
    let none = p
        .react(&m.reference, "thumbsup", false, &by)
        .expect("remove again");
    assert!(
        none.reactions.is_empty(),
        "removing it again changes nothing"
    );
}

/// 7. The actor is kept. An agent's message has an origin: `<person>'s agent`.
pub fn the_actor_is_kept(make: Make) {
    let Seed {
        provider: p,
        public,
        ..
    } = make();
    let by = me(&*p);
    assert_eq!(by.kind, ActorKind::Person, "whoami is a person");
    let own = p.send(&NewMessage::to(&public, "mine"), &by).unwrap();
    assert!(own.origin.is_none(), "a person's message has no origin");
    let agent = Actor::agent("agent-1", "Claude", by.id.clone());
    let sent = p
        .send(&NewMessage::to(&public, "from the agent"), &agent)
        .expect("an agent sends");
    assert_eq!(
        sent.origin.as_deref(),
        Some(format!("{}'s agent", by.name).as_str()),
        "the origin"
    );
    let read = p.history(&public, None, None).unwrap();
    assert!(
        read.items[0].text.starts_with("from the agent"),
        "the text is kept as sent"
    );
}

/// 8. `subscribe` delivers each new message once, in order, for the channels asked. A dropped subscription stops.
pub fn subscribe_delivers_each_post_once_in_order(make: Make) {
    let Seed {
        provider: p,
        public,
        dm,
    } = make();
    let by = me(&*p);
    let all = p.subscribe(&Filter::default()).expect("subscribe");
    let only_public = p
        .subscribe(&Filter {
            channels: vec![public.clone()],
        })
        .expect("subscribe to a channel");
    p.send(&NewMessage::to(&public, "one"), &by).unwrap();
    p.send(&NewMessage::to(&dm, "private"), &by).unwrap();
    p.send(&NewMessage::to(&public, "two"), &by).unwrap();
    let wait = Duration::from_secs(2);
    let mut got = Vec::new();
    for _ in 0..3 {
        let e = all.recv_timeout(wait).expect("a post arrives");
        assert_eq!(e.kind, EventKind::Posted);
        got.push(e.data.expect("the message").text);
    }
    assert_eq!(got, ["one", "private", "two"], "in order");
    assert!(
        all.recv_timeout(Duration::from_millis(100)).is_err(),
        "nothing arrives twice"
    );
    let mut mine = Vec::new();
    for _ in 0..2 {
        let e = only_public.recv_timeout(wait).expect("a post arrives");
        mine.push(e.data.expect("the message").text);
    }
    assert_eq!(mine, ["one", "two"], "only the channel asked for");
    assert!(
        only_public
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "the dm did not arrive"
    );
}

/// 9. A reference parses back and has the form of the spec. An unknown channel or message is `NotFound`.
pub fn references_parse_back_and_unknown_things_are_not_found(make: Make) {
    let Seed {
        provider: p,
        public,
        ..
    } = make();
    let by = me(&*p);
    let m = p.send(&NewMessage::to(&public, "ref"), &by).unwrap();
    let text = m.reference.to_string();
    let parsed: Ref = text.parse().expect("a message reference parses");
    assert_eq!(parsed, m.reference, "it prints and parses the same");
    assert_eq!(
        (parsed.capability.as_str(), parsed.provider.as_str()),
        ("messaging", p.provider())
    );
    assert_eq!(parsed.account, p.account(), "the account");
    let (channel, ts) = split_message(&m.reference).expect("channel and message id");
    assert_eq!(channel, public, "the channel is the head of the id");
    assert!(!ts.is_empty());
    let nowhere = Ref {
        id: "NOPE".into(),
        ..public.clone()
    };
    let nothing = Ref {
        id: format!("{}:1000000000.000001", public.id),
        ..public.clone()
    };
    assert!(
        matches!(
            p.history(&nowhere, None, None),
            Err(CapError::NotFound { .. })
        ),
        "an unknown channel"
    );
    assert!(
        matches!(
            p.send(&NewMessage::to(&nowhere, "hi"), &by),
            Err(CapError::NotFound { .. })
        ),
        "sending to an unknown channel"
    );
    assert!(
        matches!(p.thread(&nothing, None), Err(CapError::NotFound { .. })),
        "an unknown message"
    );
    assert!(
        matches!(
            p.send(&NewMessage::reply(&nothing, &public, "hi"), &by),
            Err(CapError::NotFound { .. })
        ),
        "a reply to an unknown message"
    );
}

/// 10. `channels` filters by kind and by text. `search` finds a message by a word of its text.
pub fn channels_and_search_filter(make: Make) {
    let Seed {
        provider: p,
        public,
        dm,
    } = make();
    let all = p.channels(&ChannelQuery::default()).expect("channels");
    let refs: Vec<&Ref> = all.items.iter().map(|c| &c.reference).collect();
    assert!(
        refs.contains(&&public) && refs.contains(&&dm),
        "both channels"
    );
    let publics = p
        .channels(&ChannelQuery {
            kinds: vec![ChannelKind::Public],
            ..ChannelQuery::default()
        })
        .unwrap();
    assert!(
        publics.items.iter().all(|c| c.kind == ChannelKind::Public)
            && publics.items.iter().any(|c| c.reference == public),
        "only public channels"
    );
    let name = &all
        .items
        .iter()
        .find(|c| c.reference == public)
        .expect("the public channel")
        .name;
    let part = name.to_uppercase();
    let found = p
        .channels(&ChannelQuery {
            text: Some(part),
            ..ChannelQuery::default()
        })
        .unwrap();
    assert!(
        found.items.iter().any(|c| c.reference == public),
        "a name matches whatever its case"
    );
    let none = p
        .channels(&ChannelQuery {
            text: Some("zzz-no-such-channel".into()),
            ..ChannelQuery::default()
        })
        .unwrap();
    assert!(none.items.is_empty(), "no match, no channel");
    if p.can(Operation::Search) {
        let by = me(&*p);
        p.send(&NewMessage::to(&public, "the zebra crossing"), &by)
            .unwrap();
        p.send(&NewMessage::to(&dm, "a quiet lunch"), &by).unwrap();
        let hits = p
            .search(&SearchQuery {
                text: "zebra".into(),
                ..SearchQuery::default()
            })
            .expect("search");
        assert_eq!(text_of(&hits.items), ["the zebra crossing"], "the word");
        let elsewhere = p
            .search(&SearchQuery {
                text: "zebra".into(),
                channel: Some(dm.clone()),
                ..SearchQuery::default()
            })
            .unwrap();
        assert!(elsewhere.items.is_empty(), "a search stays in its channel");
        let nobody = p
            .search(&SearchQuery {
                text: "zebra".into(),
                from: Some("nobody".into()),
                ..SearchQuery::default()
            })
            .unwrap();
        assert!(nobody.items.is_empty(), "a search by another author");
    }
}

/// 11. `mark_read` is accepted for a channel and refused with `NotFound` for an unknown one.
pub fn mark_read(make: Make) {
    let Seed {
        provider: p,
        public,
        ..
    } = make();
    if !p.can(Operation::MarkRead) {
        return;
    }
    let by = me(&*p);
    let m = p.send(&NewMessage::to(&public, "read me"), &by).unwrap();
    p.mark_read(&public, None).expect("mark the channel read");
    p.mark_read(&public, Some(&m.reference))
        .expect("mark up to a message");
    let nowhere = Ref {
        id: "NOPE".into(),
        ..public.clone()
    };
    assert!(
        matches!(p.mark_read(&nowhere, None), Err(CapError::NotFound { .. })),
        "an unknown channel"
    );
    let seen: HashSet<_> = p
        .channels(&ChannelQuery::default())
        .unwrap()
        .items
        .into_iter()
        .filter(|c| c.reference == public)
        .map(|c| c.unread.unwrap_or(0))
        .collect();
    assert!(seen.iter().all(|n| *n == 0), "nothing is unread after");
}
