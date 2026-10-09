use super::structs::{
    Channel, ChannelQuery, Envelope, Event, Filter, Message, MessagingCapabilities, NewMessage,
    Page, Person, SearchQuery, Workspace,
};
use super::types::Operation;
use crate::{Actor, CapError, CapResult, Ref, Subscription};

/// A provider of messaging: Slack, Discord, Telegram, or Atelier's own. The one messaging screen and the agent tools talk
/// to this and to nothing else. Every call blocks and may be slow, so none is made on the UI thread.
///
/// A provider lists what it can do in [`capabilities`](Self::capabilities). A call it does not list returns
/// [`CapError::Unsupported`]; the optional calls below do that by default.
pub trait MessagingProvider: Send + Sync {
    /// The provider's name in references: `slack`, `discord`, `memory`.
    fn provider(&self) -> &str;

    /// The account in references: the workspace or the server.
    fn account(&self) -> &str;

    fn capabilities(&self) -> MessagingCapabilities;

    /// The person the credentials belong to.
    fn whoami(&self) -> CapResult<Actor>;

    fn workspace(&self) -> CapResult<Workspace>;

    /// The channels that match, in the provider's order.
    fn channels(&self, query: &ChannelQuery) -> CapResult<Page<Channel>>;

    /// The top-level messages of a channel, newest first. The cursor goes back in time. A reply is not here; it is in
    /// [`thread`](Self::thread).
    fn history(
        &self,
        channel: &Ref,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> CapResult<Page<Message>>;

    /// A thread: the root message first, then the replies, oldest first. `root` may name a reply; its root is used.
    fn thread(&self, root: &Ref, cursor: Option<&str>) -> CapResult<Page<Message>>;

    /// Sends `new` as the signed-in person. `by` is who asked: the person, or an agent that works for the person. An
    /// agent's message has an `origin`.
    fn send(&self, new: &NewMessage, by: &Actor) -> CapResult<Message>;

    /// New activity from now on. Each call gives its own receiver; drop it to stop.
    fn subscribe(&self, filter: &Filter) -> CapResult<Subscription<Event>>;

    fn search(&self, _query: &SearchQuery) -> CapResult<Page<Message>> {
        Err(CapError::unsupported("search messages"))
    }

    fn edit(&self, _message: &Ref, _text: &str, _by: &Actor) -> CapResult<Message> {
        Err(CapError::unsupported("edit a message"))
    }

    fn delete(&self, _message: &Ref, _by: &Actor) -> CapResult<()> {
        Err(CapError::unsupported("delete a message"))
    }

    /// Adds (`on`) or removes a reaction. Idempotent.
    fn react(&self, _message: &Ref, _name: &str, _on: bool, _by: &Actor) -> CapResult<Message> {
        Err(CapError::unsupported("react to a message"))
    }

    /// Moves the read marker of a channel up to `up_to`, or to the newest message when it is `None`.
    fn mark_read(&self, _channel: &Ref, _up_to: Option<&Ref>) -> CapResult<()> {
        Err(CapError::unsupported("mark a channel read"))
    }

    fn person(&self, _person: &Ref) -> CapResult<Person> {
        Err(CapError::unsupported("read a person"))
    }

    fn export(&self, _cursor: Option<&str>) -> CapResult<Page<Envelope>> {
        Err(CapError::unsupported("export"))
    }

    fn import(&self, _batch: &[Envelope]) -> CapResult<usize> {
        Err(CapError::unsupported("import"))
    }

    /// Whether the provider lists `operation`.
    fn can(&self, operation: Operation) -> bool {
        self.capabilities().can(operation)
    }
}
