//! A running ACP agent: two threads around the process, so neither reading nor writing ever waits on the
//! other or on the caller. The reader feeds lines to the protocol and its events to the sink; the writer
//! sends what the protocol wrote. Only this file touches a process.

mod helpers;
mod structs;
mod types;

#[cfg(test)]
pub(super) use helpers::deliver;
pub(super) use structs::AcpSession;
#[cfg(test)]
pub(super) use types::Lines;
