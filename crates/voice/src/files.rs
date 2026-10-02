//! The model's files: NVIDIA's Parakeet TDT 0.6B v2 (English, CC-BY-4.0 weights), exported to ONNX by istupakov with int8
//! weights, pinned to one commit and to the hash of every file, so what runs is what was reviewed. About 660 MB on disk.
//! Chosen over Phonon-2 and Whisper by the eval in `asr-eval`: the lowest error of the fast models, about 100 ms after stop.

mod helpers;
mod structs;
mod types;

pub use helpers::{dir, install, installed, legacy_dir, total_bytes};
pub use structs::ModelFile;
pub use types::{BASE, FILES};

#[cfg(test)]
use helpers::hex;

#[cfg(test)]
use std::fs;
#[cfg(test)]
use sha2::{Digest, Sha256};
#[cfg(test)]
use crate::Error;

#[cfg(test)]
mod tests;
