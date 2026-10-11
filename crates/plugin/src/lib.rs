//! What a plugin needs to add a view to atelier, and nothing of the app's own code. `docs/plugins.md` is the guide.
//!
//! A plugin is one Rust crate, compiled into the app. It implements [`Plugin`], whose one job is to register what
//! the plugin adds. Today that is a [`PluginView`]: an entry on the left rail, and the page it opens. The page
//! implements [`PluginPage`]: it draws the sidebar and the main area of its view. The app hands the page a [`Host`],
//! the few things a plugin may ask of the app.
//!
//! The app keeps the [`Registry`], draws what is in it, and implements [`AppHost`]. This crate does not depend on the
//! app, so a plugin builds and tests without it.
//!
//! [`atelier_ui`] (the design system) and [`gpui_kit`] (GPUI) are exported again, so a plugin has this one dependency.

mod structs;
mod traits;
mod types;

pub use atelier_ui;
pub use gpui_kit;

pub use structs::{Host, Page, PluginView, Registry, Vitals};
pub use traits::{AppHost, Plugin, PluginPage};

#[cfg(test)]
mod tests;
