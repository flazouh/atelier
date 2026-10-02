//! How an edit or a write shows in the list: as the diff it makes, written out as the agent's input arrives. Which
//! calls, and whether the diff is open, follow the tool density. Pure.

mod helpers;
mod structs;

pub use helpers::edit_view;
pub use structs::EditView;

#[cfg(test)]
use crate::tool_density::ToolDensity;

#[cfg(test)]
mod tests;
