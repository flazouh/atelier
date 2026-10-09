//! Capabilities: an interface and one shared screen, with many providers behind it. The spec is in
//! `docs/capabilities`. This crate holds the parts every capability shares and the first capability, `tasks`:
//!
//! - [`Ref`]: the stable text that names a thing across capabilities, `tasks:linear:acme:ENG-123`.
//! - [`Actor`], [`Capabilities`], [`CapError`]: who acts, what a provider can do, and how a call fails.
//! - [`tasks`]: the tasks entities, the [`tasks::TasksProvider`] trait, the shared contract suite, and a memory provider.
//! - [`Registry`]: the providers the app has, by capability, provider and account.
//! - [`card`]: the tool card schema, and the resolver that turns a card and a tool result into plain values to draw.
//!
//! The crate has no UI and no network. Every call blocks and may be slow, so none is made on the UI thread.
mod actor;
mod capability;
pub mod card;
mod error;
pub mod messaging;
mod reference;
mod registry;
mod subscription;
pub mod tasks;

pub use actor::{Actor, ActorKind};
pub use capability::{AuthKind, Capabilities, Feature, Limits, Operation};
pub use error::{CapError, CapResult};
pub use reference::{Ref, RefError};
pub use registry::Registry;
pub use subscription::{StopFlag, Subscription};
