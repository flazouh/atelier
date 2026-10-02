use std::time::Duration;

/// How long a list or a history may take before atelier gives up on the agent.
pub(super) const PATIENCE: Duration = Duration::from_secs(60);

/// How long an agent that closed its stdout has to exit before atelier stops it: it says nothing more.
pub(super) const EXIT_GRACE: Duration = Duration::from_secs(2);
