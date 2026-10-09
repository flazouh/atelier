//! The status bar's numbers: how hard this machine works, and how much of each provider's allowance is used. [`Vitals`]
//! holds the last of them and is drawn as the bar at the foot of the window. It reads nothing itself: the shell hands
//! it a sample of the machine every second, and a reading of each provider every minute. What the bar draws is the cards
//! the slots hold ([`crate::slots`]); [`register`] adds this module's own two, the agents that wait and the machine.

mod consts;
mod helpers;
mod impls;
mod structs;
mod traits;

pub use consts::{LOAD_EVERY, PROVIDERS_EVERY};
pub use helpers::register;
pub use structs::{SysinfoProbe, Vitals};

#[cfg(test)]
mod tests;
