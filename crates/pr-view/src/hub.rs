//! What the app mounts: the list of pull requests, and one pull request opened from it, from a PR card or
//! from a PR chip. One entity, one pane. Open a pull request with [`PrHub::open`]; the hub shows it with a
//! way back to the list, and tells the app when the reader wants a file in the editor.

mod structs;
mod types;

pub use structs::PrHub;
pub use types::PrEvent;
