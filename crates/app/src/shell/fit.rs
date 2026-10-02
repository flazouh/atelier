//! How the window's width divides between the sidebar, the session column and the right pane. The
//! rules, with their reasons, are in docs/app.md under "Window widths".

mod helpers;
mod structs;
mod types;

pub use helpers::widths;
pub use structs::Wants;
#[cfg(test)]
pub use structs::Widths;
pub use types::{
    Fit, Pane, RIGHT_DEFAULT, RIGHT_LEAST, RIGHT_MOST, SIDEBAR_DEFAULT, SIDEBAR_LEAST,
    SIDEBAR_MOST,
};
#[cfg(test)]
pub use types::AGENT_LEAST;

#[cfg(test)]
mod tests;
