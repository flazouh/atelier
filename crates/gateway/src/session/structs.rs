/// What one agent session needs to reach the gateway. The token belongs to the session alone: it names the
/// [`Actor`](atelier_capabilities::Actor) the session writes as, and it stops working when the gateway revokes it.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionAccess {
    /// `http://127.0.0.1:<port>/mcp`.
    pub url: String,
    pub token: String,
}

// Not derived: a log line that prints an access must not print the token.
impl std::fmt::Debug for SessionAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionAccess")
            .field("url", &self.url)
            .field("token", &"<hidden>")
            .finish()
    }
}
