use atelier_capabilities::{Ref, messaging::ChannelKind};
use gpui_kit::SharedString;

use super::types::{Body, Group};

/// A reaction as a chip: the emoji's short name and how many reacted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chip {
    pub name: SharedString,
    pub count: u32,
    /// The signed-in person reacted too.
    pub mine: bool,
}

/// A file of a message, as its chip shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub name: SharedString,
    /// Its size, or what kind of thing it is when the provider gives no size.
    pub detail: SharedString,
}

/// One message as the screen draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub reference: Ref,
    pub parent: Option<Ref>,
    pub author: SharedString,
    /// The letter of the avatar.
    pub letter: SharedString,
    /// The avatar's colour: an index of the project palette, kept by the author's id.
    pub color: usize,
    pub agent: bool,
    /// `Sam's agent`, for a message an agent sent.
    pub origin: Option<SharedString>,
    pub time: SharedString,
    pub created_at: i64,
    pub edited: bool,
    pub body: Body,
    /// The replies under a root message. The screen shows them only where the provider has threads.
    pub replies: u32,
    pub reactions: Vec<Chip>,
    pub files: Vec<File>,
}

/// A channel as the sidebar lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub reference: Ref,
    pub name: SharedString,
    pub kind: ChannelKind,
    pub group: Group,
    /// The provider says there is something the person has not read.
    pub unread: bool,
}
