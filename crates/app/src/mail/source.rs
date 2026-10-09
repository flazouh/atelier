//! Where the Mail screen gets its accounts: the mail providers the app holds (`atelier_capabilities::Registry`, through the
//! `CapabilityHub`, so the agent gateway and the screen read the same ones). The screen reads and writes through them and
//! through nothing else.
mod structs;

pub use structs::{Choice, MailSource};

#[cfg(test)]
mod tests;
