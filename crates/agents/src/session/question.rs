//! A question the agent asks the reader, with the choices it offers, read from a tool call's input. The input may be whole, or
//! cut where the stream stopped: the card shows what has come.
mod helpers;
mod types;
pub use helpers::{answers_input, questions_of};
pub use types::{Question, QuestionOption};
#[cfg(test)]
mod tests;
