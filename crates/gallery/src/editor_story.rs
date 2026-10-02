//! The Editor story: a real file per language, each in a small project on disk with its language
//! server. The same keys work in every tab, because nothing below the tab picks a language except the
//! file's own path. A tab's server starts the first time the tab is shown.

mod helpers;
mod structs;
mod types;

pub use helpers::editor_story;
pub use structs::EditorTabs;
