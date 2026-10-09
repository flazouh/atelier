use serde::{Deserialize, Serialize};

/// What a mailbox is for. The screen puts the roles in its sidebar in a fixed order; any other box is `custom`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Inbox,
    Sent,
    Drafts,
    Trash,
    Spam,
    Archive,
    Custom,
}

/// Whether the service made the mailbox or the person did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailboxKind {
    System,
    User,
}

/// A call a mail provider may offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailOperation {
    Mailboxes,
    Search,
    Thread,
    Get,
    MarkRead,
    Star,
    Archive,
    Label,
    Move,
    Trash,
    CreateDraft,
    UpdateDraft,
    Reply,
    Send,
    DownloadAttachment,
    Subscribe,
}

impl MailOperation {
    /// Every operation, in the order of the spec.
    pub const ALL: [MailOperation; 16] = [
        Self::Mailboxes,
        Self::Search,
        Self::Thread,
        Self::Get,
        Self::MarkRead,
        Self::Star,
        Self::Archive,
        Self::Label,
        Self::Move,
        Self::Trash,
        Self::CreateDraft,
        Self::UpdateDraft,
        Self::Reply,
        Self::Send,
        Self::DownloadAttachment,
        Self::Subscribe,
    ];
}

/// Something the service has, which changes what the screen may show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailFeature {
    Threads,
    /// A message may have many labels (Gmail).
    Labels,
    /// A message is in one folder (IMAP).
    Folders,
    Drafts,
    Attachments,
    Push,
    /// A message id stays the same for ever. Without it a provider makes ids that hold only while the thread does not change.
    StableMessageIds,
}

/// How the text of a search is read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSyntax {
    /// Words; a message matches when all of them are in it.
    #[default]
    Plain,
    Gmail,
    Imap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailEventKind {
    NewMessage,
    Changed,
}

/// What a mail reference points at: the letter in front of its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    Mailbox,
    Thread,
    Message,
    Draft,
    Attachment,
}

impl RefKind {
    pub fn letter(self) -> char {
        match self {
            Self::Mailbox => 'b',
            Self::Thread => 't',
            Self::Message => 'm',
            Self::Draft => 'd',
            Self::Attachment => 'a',
        }
    }

    pub fn from_letter(letter: char) -> Option<Self> {
        Some(match letter {
            'b' => Self::Mailbox,
            't' => Self::Thread,
            'm' => Self::Message,
            'd' => Self::Draft,
            'a' => Self::Attachment,
            _ => return None,
        })
    }
}
