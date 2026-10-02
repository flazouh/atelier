/// Why a request produced no reply at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportError {
    ToolMissing,
    NotSignedIn,
    Offline,
    Failed(String),
}
