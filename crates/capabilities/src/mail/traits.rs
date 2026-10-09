use super::structs::{
    Account, Approval, Draft, DraftPatch, MailCapabilities, MailEvent, Mailbox, Message, NewDraft,
    SearchQuery, Thread, ThreadSummary,
};
use super::types::MailOperation;
use crate::{Actor, CapError, CapResult, Ref, Subscription, tasks::Page};

/// A provider of mail: Gmail, Outlook, IMAP. The one mail screen and the agent tools talk to this and to nothing else.
/// Every call blocks and may be slow, so none is made on the UI thread.
///
/// A provider lists what it can do in [`capabilities`](Self::capabilities). A call it does not list returns
/// [`CapError::Unsupported`]; every call below the first four does that by default.
///
/// Nothing leaves the machine except through [`send`](Self::send), and a draft comes first: see
/// [`check_send_approval`](super::check_send_approval).
pub trait MailProvider: Send + Sync {
    /// The provider's name in references: `gmail`, `outlook`, `imap`.
    fn provider(&self) -> &str;

    /// The account in references: the mail address.
    fn account(&self) -> &str;

    fn capabilities(&self) -> MailCapabilities;

    /// The account the credentials belong to.
    fn whoami(&self) -> CapResult<Account>;

    fn mailboxes(&self) -> CapResult<Vec<Mailbox>> {
        Err(unsupported("list mailboxes"))
    }

    /// The threads that match, newest first. Without a mailbox the search leaves out trash and spam.
    fn search(&self, _query: &SearchQuery) -> CapResult<Page<ThreadSummary>> {
        Err(unsupported("search"))
    }

    /// A thread with its messages, oldest first.
    fn thread(&self, _thread: &Ref) -> CapResult<Thread> {
        Err(unsupported("read a thread"))
    }

    fn get(&self, _message: &Ref) -> CapResult<Message> {
        Err(unsupported("read a message"))
    }

    fn draft(&self, _draft: &Ref) -> CapResult<Draft> {
        Err(unsupported("read a draft"))
    }

    /// `target` is a thread or a message. A thread changes all its messages.
    fn mark_read(&self, _target: &Ref, _read: bool, _by: &Actor) -> CapResult<()> {
        Err(unsupported("mark read"))
    }

    fn star(&self, _target: &Ref, _starred: bool, _by: &Actor) -> CapResult<()> {
        Err(unsupported("star"))
    }

    /// Takes the target out of the inbox. It stays findable.
    fn archive(&self, _target: &Ref, _by: &Actor) -> CapResult<()> {
        Err(unsupported("archive"))
    }

    /// Adds and removes labels. A folder provider does not list it and uses [`move_to`](Self::move_to).
    fn label(&self, _target: &Ref, _add: &[Ref], _remove: &[Ref], _by: &Actor) -> CapResult<()> {
        Err(unsupported("label"))
    }

    fn move_to(&self, _target: &Ref, _mailbox: &Ref, _by: &Actor) -> CapResult<()> {
        Err(unsupported("move"))
    }

    /// Moves the target to the trash. The mail is still there; `move_to` brings it back.
    fn trash(&self, _target: &Ref, _by: &Actor) -> CapResult<()> {
        Err(unsupported("trash"))
    }

    fn create_draft(&self, _new: &NewDraft, _by: &Actor) -> CapResult<Draft> {
        Err(unsupported("create a draft"))
    }

    /// Changes the fields in `patch`. `version` is the one the caller read; a draft that changed since fails with
    /// [`CapError::Conflict`].
    fn update_draft(
        &self,
        _draft: &Ref,
        _patch: &DraftPatch,
        _version: &str,
        _by: &Actor,
    ) -> CapResult<Draft> {
        Err(unsupported("update a draft"))
    }

    /// A draft that answers `message`. It sends nothing.
    fn reply(&self, _message: &Ref, _all: bool, _text: &str, _by: &Actor) -> CapResult<Draft> {
        Err(unsupported("reply"))
    }

    /// Sends a draft. `version` is the one the caller read. An agent needs an `approval` for that version, from the person
    /// it works for; without it the call fails with the `approval_required` provider error.
    fn send(
        &self,
        _draft: &Ref,
        _version: &str,
        _by: &Actor,
        _approval: Option<&Approval>,
    ) -> CapResult<Message> {
        Err(unsupported("send"))
    }

    fn download_attachment(&self, _attachment: &Ref) -> CapResult<Vec<u8>> {
        Err(unsupported("download an attachment"))
    }

    /// Changes from now on. A provider with no push polls. Drop the receiver to stop.
    fn subscribe(&self) -> CapResult<Subscription<MailEvent>> {
        Err(unsupported("subscribe"))
    }

    /// Whether the provider lists `operation`.
    fn can(&self, operation: MailOperation) -> bool {
        self.capabilities().can(operation)
    }
}

fn unsupported(feature: &str) -> CapError {
    CapError::unsupported(feature)
}
