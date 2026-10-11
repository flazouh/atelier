//! The files a shell command writes by name: the targets of its redirects (`> file`, `>> file`, `2> file`, `&> file`)
//! and the arguments of `tee`. Only an absolute path counts: a relative one is inside the project, where git finds what
//! the command did. A target the shell would expand (`$HOME/x`, `~/x`, a glob, a command substitution) is not known
//! without running it, so it is left out, as is a device (`/dev/null`). A script that opens its own files is out of reach.

mod helpers;
mod types;

pub(super) use helpers::write_targets;

#[cfg(test)]
mod tests;
