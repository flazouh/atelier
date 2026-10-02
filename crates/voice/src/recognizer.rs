//! The speech model, running on the CPU through ONNX Runtime.

mod structs;
mod types;

pub use structs::Recognizer;
pub use types::{MIN_SECONDS, SAMPLE_RATE};

#[cfg(test)]
mod tests;
