use super::{
    ChannelKind, MemoryMessaging, Message, MessagingProvider, NewMessage, contract, contract::Seed,
    message_ref, split_message,
};
use crate::Ref;

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
