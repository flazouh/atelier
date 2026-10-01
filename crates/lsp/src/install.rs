//! Servers that come with the app. A user installs atelier and nothing else: when a file's server is
//! missing, atelier downloads a pinned copy once and runs that copy from then on. The order is the
//! server the user installed, then atelier's own copy, then a download. See `docs/code-editor.md`,
//! Stage E.
//!
//! Every file is pinned by version and SHA-256 in the registry. A download is unpacked in a staging
//! folder and renamed into place only once it checked out, so a folder that exists is complete.

mod helpers;
mod structs;
mod types;

pub use helpers::{platform, sha256_of};
pub use structs::{Download, Launch, Package, PlatformFile, Store};
pub use types::{Kind, Unavailable, Unpack};

#[cfg(test)]
use std::{
    fs,
    path::{Path, PathBuf},
};
#[cfg(test)]
use crate::{ServerSpec, servers::NODE};

#[cfg(test)]
mod tests;
