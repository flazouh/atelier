use std::{
    sync::{atomic::{AtomicU64}, },
    };

use crate::session::{ChoiceId, RequestId};

/// What the session handle tells the thread.
pub(super) enum Inbox {
    Send(String),
    Answer(RequestId, ChoiceId),
    Close,
}

pub(super) static COUNTER: AtomicU64 = AtomicU64::new(0);

/// How a turn ended, before it is told to the UI.
pub(super) enum Ending {
    Done(Option<String>),
    Interrupted,
    Failed(String),
}

pub(super) enum Answer {
    Allow,
    AllowAlways,
    Deny,
    Interrupted,
}
