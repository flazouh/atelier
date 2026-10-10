//! The named places where a module adds something to the app. The base app draws the places; each module that has
//! something to put in one registers it at startup, and the app asks the registry what to draw. Delete a module's
//! registration and what it added is gone, and the base app does not change.
//!
//! Two slots so far:
//! - [`StatusBarCard`]: what a module puts in one of the three columns of the bar at the foot of the window.
//! - A plugin's view (`atelier_plugin::PluginView`): an entry on the left rail, and the page it opens. A plugin
//!   registers it with [`Slots::plug`]; `docs/plugins.md` is the guide.
//!
//! `docs/capabilities/slots-v1.md` lists the slots, the ones still to come, and how a capability or plugin registers one.
mod helpers;
mod structs;
mod types;

pub use helpers::builtin;
pub use structs::{BarEnv, Host, Slots, StatusBarCard};
pub use types::{BAR_ID, Column};

#[cfg(test)]
mod tests;
