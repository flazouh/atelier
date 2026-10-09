//! The `tasks` capability, v1: the entities, the provider trait, the contract every provider passes, and a provider that
//! lives in memory. The spec is `docs/capabilities/tasks-v1.md`; the schema is `docs/capabilities/tasks.schema.json`.
pub mod contract;
mod memory;
mod structs;
mod traits;
mod types;

pub use memory::MemoryTasks;
pub use structs::{
    Activity, Comment, Envelope, Event, Label, NewTask, Page, Patch, Project, Query, Status, Task,
    TaskLink,
};
pub use traits::TasksProvider;
pub use types::{ActivityKind, Category, Change, EntityKind, EventKind, LinkKind, Priority, Sort};

#[cfg(test)]
mod tests;
