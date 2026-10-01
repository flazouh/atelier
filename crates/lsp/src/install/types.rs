use std::fmt;

use super::structs::{Package, PlatformFile};

#[derive(Debug)]
pub enum Kind {
    /// One archive per platform. `program` is the executable in it, relative to the server's folder.
    Platform { files: &'static [PlatformFile], unpack: Unpack, program: &'static str },
    /// npm tarballs unpacked side by side into `node_modules`, run with atelier's Node.js. `script` is
    /// the server's entry, relative to the server's folder.
    Node { packages: &'static [Package], script: &'static str },
    /// A Go module built by the user's Go into the server's folder, as the server's program. Go
    /// serves Go projects only, and a Go project has Go.
    Go { module: &'static str },
}

/// How a platform file becomes a folder.
#[derive(Clone, Copy, Debug)]
pub enum Unpack {
    /// A gzipped executable, written to `program`.
    Gunzip,
    /// A gzipped tarball with one top folder, which is dropped.
    TarGz,
}

/// Why a server cannot run here.
#[derive(Debug, PartialEq, Eq)]
pub enum Unavailable {
    /// Nothing to run, and nothing atelier may download: no pin for it or this platform, downloads are
    /// off, or a tool it is built with is missing.
    NotInstalled,
    /// A download was tried and failed, for this reason.
    Failed(String),
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled => write!(f, "not installed"),
            Self::Failed(reason) => write!(f, "{reason}"),
        }
    }
}
