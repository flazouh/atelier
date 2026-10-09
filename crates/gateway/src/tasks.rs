//! The `tasks` tools: `tasks_list`, `tasks_get`, `tasks_create`, `tasks_update`, `tasks_comment` and `tasks_search`,
//! over the providers in a [`Registry`](atelier_capabilities::Registry). The spec is section 8 of
//! `docs/capabilities/tasks-v1.md`.
mod helpers;
mod render;
mod schemas;
mod structs;

pub use structs::TasksTools;
