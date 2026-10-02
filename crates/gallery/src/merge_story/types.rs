pub(super) const REPO: &str = "flazouh/atelier";

pub(super) const BODY: &str = "Before this, a client that aborted between two chunks left the relay writing into a closed sink.\n\nThe stream detaches on abort, and the second write is a no-op.";
