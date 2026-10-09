//! Reaching a host over the user's own `ssh`: their config, their keys, their agent. atelier asks for
//! no credentials; `BatchMode` makes a host that wants a password fail at once and say so, instead
//! of waiting on a prompt nobody sees.
//!
//! Connecting is three steps:
//!
//! 1. Probe: `uname -sm` names the host's system and architecture, and `$HOME` its home.
//! 2. Deploy: `~/.cache/atelier/remote/<version>-<hash>/atelier-remote` must answer `--version` with
//!    this app's version; the hash is of the copy this app would upload, so a new build of the same
//!    version goes up once instead of an old one staying. If the host has not got it, the copy built
//!    for that platform goes up over the same `ssh`
//!    (`cat` into a temporary file, `chmod +x`, then a rename, so a half copy never runs). This needs
//!    no `scp` on either side.
//! 3. Dial: `ssh <host> <that path> --stdio`, whose stdin and stdout carry the frames.
//!
//! The copy to upload is found with no setup ([`candidates`]): next to the app, by the host's platform
//! (`remote/<system>-<architecture>/atelier-remote`); in a Mac app's resources; the plain `atelier-remote`
//! beside the app for a host of its own kind; and the folder `tools/build-remote.sh` fills. A developer
//! may name another folder with `ATELIER_REMOTE_DIR`, looked at first.

mod helpers;
mod structs;
mod types;

pub use helpers::{
    candidates, connect, connect_at_home, deploy, dial, first_matching, hosts_in_config,
    first_found, known_hosts, local_binary, missing_words, outdated_words, probe, remote_binary, short_hash, speaks,
    speaks_this_protocol, version_line,
};
pub use structs::Platform;
pub use types::VERSION;

#[cfg(test)]
use helpers::upload_command;

#[cfg(test)]
mod tests;
