//! The sign-in of a session's agent, when it has none. The agent runs headless and cannot run `/login`, so atelier
//! runs the agent's own sign-in command (`Backend::sign_in`) on this machine, and once it is done the agent starts
//! again on the same session and the message it refused goes again. The reader can leave the wait for the browser.
//! A project on another host cannot be signed in from here: the notice says where to do it.
//!
//! A session that was resumed holds no account of its own: the one that holds its file is found when its agent
//! turns it away, so the sign-in, and the notice, are for that account.

mod enums;
mod impls;
mod structs;

pub(super) use structs::Signer;
