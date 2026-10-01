#[cfg(unix)]
use std::ffi::c_int;

/// The signals whose default is to end the process, with no word of it.
#[cfg(unix)]
pub(super) const SIGNALS: [c_int; 4] = [libc::SIGTERM, libc::SIGINT, libc::SIGHUP, libc::SIGQUIT];
