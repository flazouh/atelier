//! The agents atelier can start, as data, so the app offers them without naming one: each with its
//! backend, the name and mark its session rows wear, its look while it works, and the lab whose
//! mark its model picker shows.

mod helpers;
mod structs;

pub use helpers::{agents, by_backend, model_lab, model_mark};
pub use structs::Agent;

#[cfg(test)]
use crate::labs::Lab;

#[cfg(test)]
mod tests;
