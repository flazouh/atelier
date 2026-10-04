//! The app's own languages (docs/i18n.md).
//!
//! - [`Locale`] is the list: the languages the Mac App Store lists, with its English variants counted once.
//! - [`Message`] holds one string for every locale, so a key that misses a language does not compile.
//! - [`t`] and [`t_with`] pick the string for the current locale.
//! - [`system_locale`] is the language the reader's system is set to.
//! - [`glossary`] is what never changes between languages, and what is always said one way.
//! - [`translate`] builds the request that has a model write a key in every locale, and checks the answer.
pub mod glossary;
mod locale;
mod message;
mod system;
pub mod translate;

pub use locale::{Locale, current, set_current};
pub use message::{Message, t, t_with};
pub use system::system_locale;

#[cfg(test)]
mod tests;
