/// Where the sign-in of a session's agent stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SignIn {
    #[default]
    Idle,
    /// The browser is open and the reader signs in there.
    Waiting,
    /// The last try did not finish, with why.
    Failed(gpui_kit::SharedString),
    /// The project is on another host, where a browser here cannot sign the agent in.
    Elsewhere,
}
