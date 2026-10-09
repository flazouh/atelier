//! The local tracker as a `tasks` provider: [`LocalTasks`] lets the one tasks screen and the agent tools read and change the
//! tasks of a project through `atelier_capabilities`, with no change to the [`Tracker`](crate::Tracker) behind it.
mod helpers;
mod structs;
mod subscribe;

pub use structs::LocalTasks;

#[cfg(test)]
mod tests;
