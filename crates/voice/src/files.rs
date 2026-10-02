//! The model's files: Phonon-2 by Fermion Research (CC-BY-4.0 weights), exported to ONNX by tiyuvta, pinned to one commit and
//! to the hash of every file, so what runs is what was reviewed. The encoder is the `exact4x2` one: the same tokens as the
//! fp32 model, a 662 MB file that needs 1.1 GB of memory instead of 2.2.

mod helpers;
mod structs;
mod types;

pub use helpers::{dir, install, installed, total_bytes};
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
