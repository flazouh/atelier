//! The box for an agent that has no sign-in: one button that signs in, and the handoff as another way on. It takes
//! the place of the agent's own "Not logged in · Please run /login", which a headless agent cannot follow.

mod helpers;

pub use helpers::sign_in_notice;
