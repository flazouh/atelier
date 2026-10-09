use super::{
    Attachment, AttachmentKind, Channel, ChannelKind, ChannelQuery, Event, Feature, Filter,
    MemoryMessaging, Message, MessagingCapabilities, MessagingProvider, NewMessage, Operation,
    Page, Workspace, contract, contract::Seed, message_ref, split_message,
};
use crate::{Actor, CapError, CapResult, Ref, Subscription};

fn seed() -> Seed {
    let p = MemoryMessaging::new("test");
    let public = p.add_channel("general", ChannelKind::Public);
    let dm = p.add_channel("sam", ChannelKind::Dm);
    Seed {
        provider: Box::new(p),
        public,
        dm,
    }
}

#[test]
fn the_memory_provider_passes_the_contract() {
    contract::run(&seed);
}

/// A memory provider that may not write: it lists `ReadOnly` and `send` (the core), and refuses the write, as Discord
/// does when it has no `allow_writes`.
struct ReadOnlyMemory(MemoryMessaging);

impl MessagingProvider for ReadOnlyMemory {
    fn provider(&self) -> &str {
        self.0.provider()
    }
    fn account(&self) -> &str {
        self.0.account()
    }
    fn capabilities(&self) -> MessagingCapabilities {
        MessagingCapabilities {
            operations: MessagingCapabilities::CORE.to_vec(),
            features: vec![Feature::ReadOnly],
            ..self.0.capabilities()
        }
    }
    fn whoami(&self) -> CapResult<Actor> {
        self.0.whoami()
    }
    fn workspace(&self) -> CapResult<Workspace> {
        self.0.workspace()
    }
    fn channels(&self, query: &ChannelQuery) -> CapResult<Page<Channel>> {
        self.0.channels(query)
    }
    fn history(&self, c: &Ref, cursor: Option<&str>, limit: Option<u32>) -> CapResult<Page<Message>> {
        self.0.history(c, cursor, limit)
    }
    fn thread(&self, root: &Ref, cursor: Option<&str>) -> CapResult<Page<Message>> {
        self.0.thread(root, cursor)
    }
    fn send(&self, _: &NewMessage, _: &Actor) -> CapResult<Message> {
        Err(CapError::Provider {
            code: "read_only".into(),
            message: "this account may not write".into(),
        })
    }
    fn subscribe(&self, filter: &Filter) -> CapResult<Subscription<Event>> {
        self.0.subscribe(filter)
    }
}

#[test]
fn a_read_only_provider_passes_the_contract_that_asks_it_to_change_nothing() {
    contract::run(&|| {
        let p = MemoryMessaging::new("test");
        let public = p.add_channel("general", ChannelKind::Public);
        let dm = p.add_channel("sam", ChannelKind::Dm);
        Seed {
            provider: Box::new(ReadOnlyMemory(p)),
            public,
            dm,
        }
    });
}

#[test]
fn a_read_only_account_lists_send_but_does_not_offer_a_call_that_writes() {
    let p = ReadOnlyMemory(MemoryMessaging::new("test"));
    assert!(p.can(Operation::Send), "send is core, so it is listed");
    assert!(!p.offers(Operation::Send), "but it is not offered");
    assert!(p.offers(Operation::History), "a read stays");
    let writable = MemoryMessaging::new("test");
    assert!(writable.offers(Operation::Send), "a writable account offers it");
    assert!(
        !writable.capabilities().has(Feature::ReadOnly),
        "the memory provider lists no ReadOnly"
    );
}

#[test]
fn every_write_operation_is_named_and_no_read_is() {
    let writes: Vec<Operation> = MessagingCapabilities::ALL
        .into_iter()
        .filter(|o| o.writes())
        .collect();
    assert_eq!(
        writes,
        [
            Operation::Send,
            Operation::Edit,
            Operation::Delete,
            Operation::React,
            Operation::MarkRead,
            Operation::Import
        ]
    );
}

#[test]
fn a_message_reference_splits_into_its_channel_and_id() {
    let channel: Ref = "messaging:slack:acme:C01".parse().unwrap();
    let message = message_ref(&channel, "1760000000.000100");
    assert_eq!(
        message.to_string(),
        "messaging:slack:acme:C01:1760000000.000100"
    );
    assert_eq!(
        split_message(&message),
        Some((channel.clone(), "1760000000.000100".to_string()))
    );
    assert_eq!(split_message(&channel), None, "a channel is not a message");
}

#[test]
fn a_message_goes_through_json_with_its_ref_as_text() {
    let p = MemoryMessaging::new("test");
    let channel = p.add_channel("general", ChannelKind::Public);
    let me = p.whoami().unwrap();
    let sent = p.send(&NewMessage::to(&channel, "hi"), &me).unwrap();
    let json = serde_json::to_value(&sent).unwrap();
    assert_eq!(json["ref"], sent.reference.to_string());
    assert!(
        json.get("origin").is_none(),
        "an absent origin is not written"
    );
    let back: Message = serde_json::from_value(json).unwrap();
    assert_eq!(back, sent);
}

#[test]
fn deleting_a_root_deletes_its_replies() {
    let p = MemoryMessaging::new("test");
    let channel = p.add_channel("general", ChannelKind::Public);
    let me = p.whoami().unwrap();
    let root = p.send(&NewMessage::to(&channel, "root"), &me).unwrap();
    let reply = p
        .send(&NewMessage::reply(&root.reference, &channel, "reply"), &me)
        .unwrap();
    p.delete(&root.reference, &me).unwrap();
    assert!(p.thread(&reply.reference, None).is_err());
}

#[test]
fn a_reply_must_stay_in_its_channel() {
    let p = MemoryMessaging::new("test");
    let a = p.add_channel("a", ChannelKind::Public);
    let b = p.add_channel("b", ChannelKind::Public);
    let me = p.whoami().unwrap();
    let root = p.send(&NewMessage::to(&a, "root"), &me).unwrap();
    assert!(matches!(
        p.send(&NewMessage::reply(&root.reference, &b, "x"), &me),
        Err(crate::CapError::Invalid { .. })
    ));
}

#[test]
fn the_capabilities_name_the_core() {
    let caps = MemoryMessaging::new("t").capabilities();
    assert!(
        super::MessagingCapabilities::CORE
            .iter()
            .all(|o| caps.can(*o))
    );
    let json = serde_json::to_string(&caps).unwrap();
    assert!(
        json.contains(r#""formatting":"rich""#) && json.contains("mark_read"),
        "{json}"
    );
}

#[test]
fn a_page_holds_no_more_than_the_provider_says() {
    let p = MemoryMessaging::new("test").with_page_max(3);
    let channel = p.add_channel("general", ChannelKind::Public);
    let me = p.whoami().unwrap();
    for n in 0..5 {
        p.send(&NewMessage::to(&channel, &format!("m{n}")), &me)
            .unwrap();
    }
    assert_eq!(p.capabilities().limits.page_max, Some(3));
    let first = p.history(&channel, None, None).unwrap();
    assert_eq!(
        first.items.len(),
        3,
        "the page is as long as the provider allows"
    );
    let cursor = first.next_cursor.expect("two more messages wait");
    let second = p.history(&channel, Some(&cursor), None).unwrap();
    assert_eq!(second.items.len(), 2);
    assert!(second.next_cursor.is_none());
}

#[test]
fn a_person_can_be_named_and_the_origin_of_an_agents_message_carries_the_name() {
    let p = MemoryMessaging::new("test").with_me("Alex");
    let channel = p.add_channel("general", ChannelKind::Public);
    let agent = crate::Actor::agent("a1", "Claude", "me");
    let sent = p.send(&NewMessage::to(&channel, "done"), &agent).unwrap();
    assert_eq!(p.whoami().unwrap().name, "Alex");
    assert_eq!(sent.origin.as_deref(), Some("Alex's agent"));
}

#[test]
fn a_file_and_an_unread_count_can_be_seeded() {
    let p = MemoryMessaging::new("test");
    let channel = p.add_channel("general", ChannelKind::Public);
    let me = p.whoami().unwrap();
    let sent = p.send(&NewMessage::to(&channel, "see file"), &me).unwrap();
    let file = Attachment {
        id: "f1".into(),
        name: "plan.pdf".into(),
        mime: Some("application/pdf".into()),
        size: Some(2048),
        url: "https://example.com/plan.pdf".into(),
        kind: AttachmentKind::File,
    };
    p.attach(&sent.reference, file.clone()).unwrap();
    p.set_unread(&channel, 4).unwrap();
    let held = p.history(&channel, None, None).unwrap().items.remove(0);
    assert_eq!(held.attachments, vec![file]);
    assert_eq!(
        p.channels(&Default::default()).unwrap().items[0].unread,
        Some(4)
    );
    let elsewhere: Ref = "messaging:memory:test:C99".parse().unwrap();
    assert!(
        p.set_unread(&elsewhere, 1).is_err(),
        "a channel the provider lacks"
    );
}
