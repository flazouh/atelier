use crate::{Actor, ActorKind, CapError, CapResult, Ref};

/// The label of a message an agent sent: `Sam's agent`. `owner_name` is the name of the person the agent works for.
/// A person's own message has no origin.
pub fn origin_of(by: &Actor, owner_name: &str) -> Option<String> {
    match by.kind {
        ActorKind::Agent => Some(format!("{owner_name}'s agent")),
        ActorKind::Person => None,
    }
}

/// The reference of a message: the channel's id, a colon, and the message's own id (`ts`).
pub fn message_ref(channel: &Ref, ts: &str) -> Ref {
    Ref {
        id: format!("{}:{ts}", channel.id),
        ..channel.clone()
    }
}

/// Splits a message reference into its channel and its own id. `None` when `message` names a channel.
pub fn split_message(message: &Ref) -> Option<(Ref, String)> {
    let (channel, ts) = message.id.split_once(':')?;
    if channel.is_empty() || ts.is_empty() {
        return None;
    }
    Some((
        Ref {
            id: channel.to_string(),
            ..message.clone()
        },
        ts.to_string(),
    ))
}

/// Like [`split_message`], but a reference that is not a message is an invalid `field`.
pub fn require_message(message: &Ref, field: &str) -> CapResult<(Ref, String)> {
    split_message(message).ok_or_else(|| CapError::invalid(field))
}

/// The reference of a person: `user/<id>`, in the account of `like`.
pub fn person_ref(like: &Ref, user: &str) -> Ref {
    Ref {
        id: format!("user/{user}"),
        ..like.clone()
    }
}

/// The reference of the workspace, in the account of `like`.
pub fn workspace_ref(like: &Ref) -> Ref {
    Ref {
        id: "workspace".into(),
        ..like.clone()
    }
}
