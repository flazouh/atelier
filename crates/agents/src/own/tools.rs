//! The agent's tools. Each is a function of the project and a JSON input, and touches the project only
//! through the `Project` interface, so the same tool works on a folder here and on an SSH host.
//! A tool never panics on bad input: it returns an error the model can read and fix.

mod edit;
mod helpers;
mod list;
mod read;
mod search;
mod shell;
mod structs;
mod traits;
mod types;
mod write;

pub use helpers::{all, brief, cap, definitions, describe};
pub(crate) use helpers::{make_parent, object, project_path};
pub use structs::{ToolContext, ToolResult};
pub use traits::Tool;
pub use types::{Access, MAX_RESULT};
