//! The forge behind a project: where its pull requests, reviews and checks live. [`Forge`] is lathe's
//! own interface to one, in GitQuiet's words (`docs/glossary.md`); nothing above it names GitHub.
//! [`github`] is the first implementation. `docs/forge.md` says how the two fit.
mod court;
mod error;
mod forge;
pub mod github;
mod lookup;
mod model;
pub mod present;
pub mod time;

pub use court::{Court, Filed, Weighing, court_of, file as file_courts};
pub use error::{ForgeError, ForgeResult};
pub use forge::Forge;
pub use lookup::Lookup;
pub use model::*;
