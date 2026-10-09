//! Where the Messages screen gets its accounts: the messaging providers the app holds (`atelier_capabilities::Registry`,
//! through the `CapabilityHub`, so the agent gateway and the screen read the same ones). The screen reads and writes
//! through them and through nothing else.
mod structs;

pub use structs::{Choice, MessagesSource};

#[cfg(test)]
mod tests;
