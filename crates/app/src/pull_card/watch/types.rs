use atelier_forge::{Check, ForgeError, Pull};

/// What one read got.
pub(super) type Got = (Result<Pull, ForgeError>, Result<Vec<Check>, ForgeError>);
