use crate::claude;

/// The embedded strip or mark served at `path`, if an agent or a lab ships one there.
pub(crate) fn strip_bytes(path: &str) -> Option<&'static [u8]> {
    claude::SparkState::ALL.iter().map(|state| state.strip()).find(|strip| strip.path == path).map(|strip| strip.bytes)
        .or_else(|| crate::labs::bytes(path))
        .or_else(|| crate::coding_agents::bytes(path))
}
