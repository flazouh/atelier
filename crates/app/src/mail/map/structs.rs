use atelier_capabilities::{
    Ref,
    mail::{MailboxKind, Role},
};
use gpui_kit::SharedString;

/// A mailbox as the sidebar lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoxRow {
    pub reference: Ref,
    pub name: SharedString,
    pub role: Role,
    pub kind: MailboxKind,
    /// How many unread messages the provider counts in it.
    pub unread: u32,
}

/// A thread as the middle list shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadRow {
    pub reference: Ref,
    /// The people other than the account's own, or the account itself when nobody else is in it.
    pub who: SharedString,
    pub subject: SharedString,
    pub snippet: SharedString,
    pub time: SharedString,
    pub unread: bool,
    pub starred: bool,
    pub attachment: bool,
    pub count: u32,
    /// Changes when the thread does, so a list that read it again draws only the rows that changed.
    pub version: String,
}

/// A file of a message: its name, and its size when the provider gives one. The bytes stay with the provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attached {
    pub name: SharedString,
    pub detail: SharedString,
}

/// One message as the reading pane draws it. All of its text is plain.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageView {
    pub reference: Ref,
    pub from_name: Option<SharedString>,
    pub from_address: SharedString,
    /// `To Ana, ben@example.com`.
    pub to: SharedString,
    pub cc: Option<SharedString>,
    pub time: SharedString,
    pub unread: bool,
    /// The account's own message.
    pub mine: bool,
    /// The whole body.
    pub text: SharedString,
    /// The body cut at [`BODY_LIMIT`](super::BODY_LIMIT), when it is longer; the screen draws it until the reader asks for all.
    pub cut: Option<SharedString>,
    pub attachments: Vec<Attached>,
    /// The addresses in the body, as text. The screen opens one only when the reader asks, and after it shows the address.
    pub links: Vec<SharedString>,
}
